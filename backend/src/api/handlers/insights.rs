use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use futures::future::join_all;
use tracing::{error, warn};

use crate::api::dtos::insights::*;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::api::{game_type, season};
use crate::error::Result;
use crate::models::fantasy::PlayerStats;

/// Calculate the current NHL "hockey date" in Eastern Time (proper DST handling)
pub fn hockey_today() -> String {
    use chrono_tz::America::New_York;
    Utc::now().with_timezone(&New_York).format("%Y-%m-%d").to_string()
}

/// Generate and cache insights for a given league. Used by both the API handler and the cron job.
pub async fn generate_and_cache_insights(
    state: &Arc<AppState>,
    league_id: &str,
) -> Result<InsightsResponse> {
    let today = hockey_today();
    let cache_key = format!("insights:{}:{}:{}:{}", league_id, season(), game_type(), today);

    // Check cache
    if let Some(cached) = state
        .db
        .cache()
        .get_cached_response::<InsightsResponse>(&cache_key)
        .await?
    {
        return Ok(cached);
    }

    // 1. Compute signals
    let signals = compute_signals(state, league_id, &today).await?;

    // 2. Call Claude for narratives
    let narratives = generate_narratives(&signals).await;

    let response = InsightsResponse {
        generated_at: Utc::now().to_rfc3339(),
        narratives,
        signals,
    };

    // Cache
    let _ = state
        .db
        .cache()
        .store_response(&cache_key, &today, &response)
        .await;

    Ok(response)
}

pub async fn get_insights(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<InsightsResponse>>> {
    let league_id = params.get("league_id").cloned().unwrap_or_default();
    let response = generate_and_cache_insights(&state, &league_id).await?;
    Ok(json_success(response))
}

// ---------------------------------------------------------------------------
// Signal computation
// ---------------------------------------------------------------------------

async fn compute_signals(
    state: &Arc<AppState>,
    league_id: &str,
    hockey_today: &str,
) -> Result<InsightsSignals> {
    // Run independent signal computations concurrently
    let hot_fut = compute_hot_players(state, league_id);
    let contenders_fut = compute_cup_contenders(state);
    let games_fut = compute_todays_games(state, hockey_today);
    let fantasy_fut = compute_fantasy_race(state, league_id, hockey_today);
    let sleepers_fut = compute_sleeper_alerts(state, league_id);
    let news_fut = scrape_headlines();

    let (hot, contenders, games, fantasy, sleepers, news) =
        tokio::join!(hot_fut, contenders_fut, games_fut, fantasy_fut, sleepers_fut, news_fut);

    Ok(InsightsSignals {
        hot_players: hot.unwrap_or_default(),
        cup_contenders: contenders.unwrap_or_default(),
        todays_games: games.unwrap_or_default(),
        fantasy_race: fantasy.unwrap_or_default(),
        sleeper_alerts: sleepers.unwrap_or_default(),
        news_headlines: news.unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// Hot players (top 5 by recent form)
// ---------------------------------------------------------------------------

async fn compute_hot_players(
    state: &Arc<AppState>,
    league_id: &str,
) -> Result<Vec<HotPlayerSignal>> {
    let stats = state
        .nhl_client
        .get_skater_stats(&season(), game_type())
        .await?;

    // Take top 20 by playoff points
    let mut top_players = stats.points.clone();
    top_players.sort_by(|a, b| (b.value as i32).cmp(&(a.value as i32)));
    top_players.truncate(20);

    // Build fantasy ownership mapping if league is provided
    let ownership: HashMap<i64, String> = if !league_id.is_empty() {
        build_ownership_map(state, league_id).await
    } else {
        HashMap::new()
    };

    // Fetch form for each player concurrently (semaphore in NhlClient limits to 5)
    let form_futures: Vec<_> = top_players
        .iter()
        .map(|player| {
            let state = Arc::clone(state);
            let player_id = player.id as i64;
            async move {
                match state
                    .nhl_client
                    .get_player_form(player_id, &season(), game_type(), 5)
                    .await
                {
                    Ok((goals, assists, points)) => Some((player_id, goals, assists, points, 5usize)),
                    Err(e) => {
                        warn!("Failed to get form for player {}: {}", player_id, e);
                        None
                    }
                }
            }
        })
        .collect();

    let form_results = join_all(form_futures).await;

    // Combine player info with form data
    let mut signals: Vec<(i64, HotPlayerSignal)> = Vec::new();

    for (player, form) in top_players.iter().zip(form_results.into_iter()) {
        let name = format!(
            "{} {}",
            player.first_name.get("default").cloned().unwrap_or_default(),
            player.last_name.get("default").cloned().unwrap_or_default()
        );
        let player_id = player.id as i64;
        let (form_goals, form_assists, form_points, form_games) = match form {
            Some((_, g, a, p, n)) => (g, a, p, n),
            None => (0, 0, 0, 0),
        };

        signals.push((
            player_id,
            HotPlayerSignal {
                name,
                nhl_team: player.team_abbrev.clone(),
                position: player.position.clone(),
                form_goals,
                form_assists,
                form_points,
                form_games,
                playoff_points: player.value as i32,
                fantasy_team: ownership.get(&player_id).cloned(),
                image_url: state.nhl_client.get_player_image_url(player_id),
                top_speed: None,
                top_shot_speed: None,
            },
        ));
    }

    // Sort by form points descending, take top 5
    signals.sort_by(|a, b| b.1.form_points.cmp(&a.1.form_points));
    signals.truncate(5);

    // Fetch NHL Edge data for top 5 players concurrently
    let edge_futures: Vec<_> = signals
        .iter()
        .map(|(pid, _)| {
            let state = Arc::clone(state);
            let pid = *pid;
            async move {
                match state.nhl_client.get_skater_edge_detail(pid).await {
                    Ok(edge) => Some((pid, edge)),
                    Err(e) => {
                        warn!("Failed to get edge data for player {}: {}", pid, e);
                        None
                    }
                }
            }
        })
        .collect();

    let edge_results = join_all(edge_futures).await;
    let edge_map: HashMap<i64, serde_json::Value> = edge_results
        .into_iter()
        .flatten()
        .collect();

    // Enrich with edge data
    let signals: Vec<HotPlayerSignal> = signals
        .into_iter()
        .map(|(pid, mut signal)| {
            if let Some(edge) = edge_map.get(&pid) {
                signal.top_speed = edge
                    .get("topSkatingSpeed")
                    .and_then(|v| v.as_f64());
                signal.top_shot_speed = edge
                    .get("topShotSpeed")
                    .and_then(|v| v.as_f64());
            }
            signal
        })
        .collect();

    Ok(signals)
}

// ---------------------------------------------------------------------------
// Cup contenders (teams leading their series or with most wins)
// ---------------------------------------------------------------------------

async fn compute_cup_contenders(state: &Arc<AppState>) -> Result<Vec<ContenderSignal>> {
    let carousel = state
        .nhl_client
        .get_playoff_carousel(format!("{}", season()))
        .await?;

    let carousel = match carousel {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    // Find the latest round with active series
    let current_round = carousel.current_round as u32;
    let active_round = carousel
        .rounds
        .iter()
        .find(|r| r.round_number == current_round as i64);

    let round = match active_round {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };

    let mut contenders: Vec<ContenderSignal> = Vec::new();

    for series in &round.series {
        let top_wins = series.top_seed.wins as u32;
        let bot_wins = series.bottom_seed.wins as u32;

        // Report the team that is leading (or both if tied)
        if top_wins >= bot_wins {
            contenders.push(ContenderSignal {
                team_abbrev: series.top_seed.abbrev.clone(),
                series_title: series.series_label.clone(),
                wins: top_wins,
                opponent_abbrev: series.bottom_seed.abbrev.clone(),
                opponent_wins: bot_wins,
                round: current_round,
            });
        }
        if bot_wins > top_wins {
            contenders.push(ContenderSignal {
                team_abbrev: series.bottom_seed.abbrev.clone(),
                series_title: series.series_label.clone(),
                wins: bot_wins,
                opponent_abbrev: series.top_seed.abbrev.clone(),
                opponent_wins: top_wins,
                round: current_round,
            });
        }
    }

    // Sort by wins descending, take top 3
    contenders.sort_by(|a, b| b.wins.cmp(&a.wins));
    contenders.truncate(3);

    Ok(contenders)
}

// ---------------------------------------------------------------------------
// Today's games
// ---------------------------------------------------------------------------

async fn compute_todays_games(
    state: &Arc<AppState>,
    hockey_today: &str,
) -> Result<Vec<TodaysGameSignal>> {
    let schedule = state
        .nhl_client
        .get_schedule_by_date(hockey_today)
        .await?;

    let games = schedule.games_for_date(hockey_today);

    // Fetch standings and yesterday's scores concurrently
    let yesterday = {
        let date = chrono::NaiveDate::parse_from_str(hockey_today, "%Y-%m-%d")
            .unwrap_or_else(|_| Utc::now().date_naive());
        (date - chrono::Duration::days(1)).format("%Y-%m-%d").to_string()
    };

    let (standings_res, scores_res) = tokio::join!(
        state.nhl_client.get_standings_raw(),
        state.nhl_client.get_scores_by_date(&yesterday)
    );

    // Build standings lookup: team_abbrev -> (streak, l10, conf_rank)
    let standings_map: HashMap<String, (String, String)> = standings_res
        .ok()
        .and_then(|json| json.get("standings")?.as_array().cloned())
        .map(|standings| {
            standings
                .iter()
                .filter_map(|team| {
                    let abbrev = team.get("teamAbbrev")
                        .and_then(|a| a.get("default"))
                        .and_then(|a| a.as_str())?
                        .to_string();
                    let streak_code = team.get("streakCode").and_then(|v| v.as_str()).unwrap_or("");
                    let streak_count = team.get("streakCount").and_then(|v| v.as_i64()).unwrap_or(0);
                    let streak = if !streak_code.is_empty() {
                        format!("{}{}", streak_code, streak_count)
                    } else {
                        String::new()
                    };
                    let l10w = team.get("l10Wins").and_then(|v| v.as_i64()).unwrap_or(0);
                    let l10l = team.get("l10Losses").and_then(|v| v.as_i64()).unwrap_or(0);
                    let l10o = team.get("l10OtLosses").and_then(|v| v.as_i64()).unwrap_or(0);
                    let l10 = format!("{}-{}-{}", l10w, l10l, l10o);
                    Some((abbrev, (streak, l10)))
                })
                .collect()
        })
        .unwrap_or_default();

    // Build last-result lookup from yesterday's scores: team_abbrev -> "W 4-2 vs OPP" / "L 2-4 vs OPP"
    let last_result_map: HashMap<String, String> = scores_res
        .ok()
        .and_then(|json| json.get("games")?.as_array().cloned())
        .map(|games_arr| {
            let mut map = HashMap::new();
            for g in &games_arr {
                let state_str = g.get("gameState").and_then(|v| v.as_str()).unwrap_or("");
                if state_str != "OFF" && state_str != "FINAL" {
                    continue;
                }
                let home_abbrev = g.get("homeTeam").and_then(|t| t.get("abbrev")).and_then(|a| a.as_str()).unwrap_or("");
                let away_abbrev = g.get("awayTeam").and_then(|t| t.get("abbrev")).and_then(|a| a.as_str()).unwrap_or("");
                let home_score = g.get("homeTeam").and_then(|t| t.get("score")).and_then(|s| s.as_i64()).unwrap_or(0);
                let away_score = g.get("awayTeam").and_then(|t| t.get("score")).and_then(|s| s.as_i64()).unwrap_or(0);

                if !home_abbrev.is_empty() && !away_abbrev.is_empty() {
                    let (home_result, away_result) = if home_score > away_score {
                        (
                            format!("W {}-{} vs {}", home_score, away_score, away_abbrev),
                            format!("L {}-{} @ {}", away_score, home_score, home_abbrev),
                        )
                    } else {
                        (
                            format!("L {}-{} vs {}", home_score, away_score, away_abbrev),
                            format!("W {}-{} @ {}", away_score, home_score, home_abbrev),
                        )
                    };
                    map.insert(home_abbrev.to_string(), home_result);
                    map.insert(away_abbrev.to_string(), away_result);
                }
            }
            map
        })
        .unwrap_or_default();

    let mut signals: Vec<TodaysGameSignal> = Vec::new();

    for game in &games {
        let home = &game.home_team.abbrev;
        let away = &game.away_team.abbrev;

        let (series_context, is_elimination) = match &game.series_status {
            Some(ss) => {
                let context = format!(
                    "{} - {} leads {}-{}",
                    ss.series_title,
                    if ss.top_seed_wins >= ss.bottom_seed_wins { &ss.top_seed_team_abbrev } else { &ss.bottom_seed_team_abbrev },
                    ss.top_seed_wins.max(ss.bottom_seed_wins),
                    ss.top_seed_wins.min(ss.bottom_seed_wins)
                );
                let elim = ss.top_seed_wins == 3 || ss.bottom_seed_wins == 3;
                (Some(context), elim)
            }
            None => (None, false),
        };

        // Fetch rich landing data
        let mut home_record = String::new();
        let mut away_record = String::new();
        let mut venue = String::new();
        let mut points_leaders = None;
        let mut goals_leaders = None;
        let mut assists_leaders = None;
        let mut home_goalie = None;
        let mut away_goalie = None;

        if let Ok(landing) = state.nhl_client.get_game_landing_raw(game.id).await {
            venue = landing.get("venue").and_then(|v| v.get("default")).and_then(|v| v.as_str()).unwrap_or("").to_string();

            if let Some(matchup) = landing.get("matchup") {
                if let Some(leaders) = matchup.get("skaterComparison").and_then(|s| s.get("leaders")).and_then(|l| l.as_array()) {
                    for leader in leaders {
                        let cat = leader.get("category").and_then(|c| c.as_str()).unwrap_or("");
                        let al = extract_player_leader(leader, "awayLeader");
                        let hl = extract_player_leader(leader, "homeLeader");
                        if let (Some(a), Some(h)) = (al, hl) {
                            match cat {
                                "points" => points_leaders = Some((a, h)),
                                "goals" => goals_leaders = Some((a, h)),
                                "assists" => assists_leaders = Some((a, h)),
                                _ => {}
                            }
                        }
                    }
                }

                if let Some(gc) = matchup.get("goalieComparison") {
                    home_goalie = extract_goalie_stats(gc, "homeTeam");
                    away_goalie = extract_goalie_stats(gc, "awayTeam");
                    home_record = gc.get("homeTeam").and_then(|t| t.get("teamTotals")).and_then(|t| t.get("record")).and_then(|r| r.as_str()).unwrap_or("").to_string();
                    away_record = gc.get("awayTeam").and_then(|t| t.get("teamTotals")).and_then(|t| t.get("record")).and_then(|r| r.as_str()).unwrap_or("").to_string();
                }
            }
        }

        // Standings context
        let (home_streak, home_l10) = standings_map
            .get(home)
            .map(|(s, l)| (Some(s.clone()), Some(l.clone())))
            .unwrap_or((None, None));
        let (away_streak, away_l10) = standings_map
            .get(away)
            .map(|(s, l)| (Some(s.clone()), Some(l.clone())))
            .unwrap_or((None, None));

        // Last game result
        let home_last_result = last_result_map.get(home).cloned();
        let away_last_result = last_result_map.get(away).cloned();

        signals.push(TodaysGameSignal {
            home_team: home.clone(),
            away_team: away.clone(),
            home_record,
            away_record,
            venue,
            start_time: game.start_time_utc.clone(),
            series_context,
            is_elimination,
            points_leaders,
            goals_leaders,
            assists_leaders,
            home_goalie,
            away_goalie,
            home_streak,
            away_streak,
            home_l10,
            away_l10,
            home_last_result,
            away_last_result,
        });
    }

    Ok(signals)
}

fn extract_player_leader(leader: &serde_json::Value, side: &str) -> Option<PlayerLeader> {
    let p = leader.get(side)?;
    Some(PlayerLeader {
        name: p.get("name").and_then(|n| n.get("default")).and_then(|n| n.as_str()).unwrap_or("").to_string(),
        position: p.get("positionCode").and_then(|p| p.as_str()).unwrap_or("").to_string(),
        value: p.get("value").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        headshot: p.get("headshot").and_then(|h| h.as_str()).unwrap_or("").to_string(),
    })
}

fn extract_goalie_stats(gc: &serde_json::Value, side: &str) -> Option<GoalieStats> {
    let team = gc.get(side)?;
    let leaders = team.get("leaders").and_then(|l| l.as_array())?;
    let best = leaders.iter().max_by_key(|g| g.get("gamesPlayed").and_then(|v| v.as_i64()).unwrap_or(0))?;
    Some(GoalieStats {
        name: best.get("name").and_then(|n| n.get("default")).and_then(|n| n.as_str()).unwrap_or("").to_string(),
        record: best.get("record").and_then(|r| r.as_str()).unwrap_or("").to_string(),
        gaa: best.get("gaa").and_then(|v| v.as_f64()).unwrap_or(0.0),
        save_pctg: best.get("savePctg").and_then(|v| v.as_f64()).unwrap_or(0.0),
        shutouts: best.get("shutouts").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
    })
}

// ---------------------------------------------------------------------------
// Fantasy race (only when league_id present)
// ---------------------------------------------------------------------------

async fn compute_fantasy_race(
    state: &Arc<AppState>,
    league_id: &str,
    hockey_today: &str,
) -> Result<Vec<FantasyRaceSignal>> {
    if league_id.is_empty() {
        return Ok(Vec::new());
    }

    let teams = state.db.get_all_teams(league_id).await?;
    let stats = state
        .nhl_client
        .get_skater_stats(&season(), game_type())
        .await?;

    // Determine which NHL teams play today
    let schedule = state
        .nhl_client
        .get_schedule_by_date(hockey_today)
        .await
        .ok();
    let today_nhl_teams: HashSet<String> = schedule
        .as_ref()
        .map(|s| {
            s.games_for_date(hockey_today)
                .iter()
                .flat_map(|g| {
                    vec![
                        g.home_team.abbrev.clone(),
                        g.away_team.abbrev.clone(),
                    ]
                })
                .collect()
        })
        .unwrap_or_default();

    let mut race: Vec<FantasyRaceSignal> = Vec::new();

    for team in &teams {
        let players = match state.db.get_team_players(team.id).await {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Calculate total points from playoff stats
        let mut total_points = 0i32;
        let mut active_today = 0usize;

        for player in &players {
            // Calculate points
            let mut ps = PlayerStats::default();
            let calculated = ps.calculate_player_points(player.nhl_id, &stats);
            total_points += calculated.total_points;

            // Check if player's NHL team plays today
            if today_nhl_teams.contains(&player.nhl_team) {
                active_today += 1;
            }
        }

        race.push(FantasyRaceSignal {
            team_name: team.name.clone(),
            total_points,
            rank: 0, // assigned after sorting
            players_active_today: active_today,
        });
    }

    // Sort by total_points descending and assign ranks
    race.sort_by(|a, b| b.total_points.cmp(&a.total_points));
    for (i, entry) in race.iter_mut().enumerate() {
        entry.rank = i + 1;
    }

    Ok(race)
}

// ---------------------------------------------------------------------------
// Sleeper alerts (only when league_id present)
// ---------------------------------------------------------------------------

async fn compute_sleeper_alerts(
    state: &Arc<AppState>,
    league_id: &str,
) -> Result<Vec<SleeperAlertSignal>> {
    if league_id.is_empty() {
        return Ok(Vec::new());
    }

    let sleepers = state.db.get_all_sleepers(league_id).await?;
    let stats = state
        .nhl_client
        .get_skater_stats(&season(), game_type())
        .await?;

    // Build fantasy team name map
    let fantasy_teams = state.db.get_all_teams(league_id).await?;
    let team_name_map: HashMap<i64, String> = fantasy_teams
        .into_iter()
        .map(|t| (t.id, t.name))
        .collect();

    let mut alerts: Vec<SleeperAlertSignal> = Vec::new();

    for sleeper in &sleepers {
        let mut goals = 0i32;
        let mut assists = 0i32;

        if let Some(p) = stats.goals.iter().find(|p| p.id as i64 == sleeper.nhl_id) {
            goals = p.value as i32;
        }
        if let Some(p) = stats.assists.iter().find(|p| p.id as i64 == sleeper.nhl_id) {
            assists = p.value as i32;
        }

        let fantasy_team = sleeper
            .team_id
            .and_then(|tid| team_name_map.get(&tid).cloned());

        alerts.push(SleeperAlertSignal {
            name: sleeper.name.clone(),
            nhl_team: sleeper.nhl_team.clone(),
            fantasy_team,
            points: goals + assists,
            goals,
            assists,
        });
    }

    // Sort by points descending
    alerts.sort_by(|a, b| b.points.cmp(&a.points));

    Ok(alerts)
}

// ---------------------------------------------------------------------------
// News headlines from Daily Faceoff
// ---------------------------------------------------------------------------

async fn scrape_headlines() -> Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| crate::Error::Internal(format!("Failed to build HTTP client: {}", e)))?;

    let mut all_headlines: Vec<String> = Vec::new();

    // 1. Scrape player news headlines
    if let Ok(resp) = client
        .get("https://www.dailyfaceoff.com/hockey-player-news")
        .header("User-Agent", "Mozilla/5.0 (compatible; FantasyHockeyBot/1.0)")
        .send()
        .await
    {
        if let Ok(html) = resp.text().await {
            let document = scraper::Html::parse_document(&html);
            let selectors = ["h3 a", ".news-item h3", ".news-item__title", "article h3", ".post-title a", "h2 a"];
            for sel_str in &selectors {
                if let Ok(selector) = scraper::Selector::parse(sel_str) {
                    for element in document.select(&selector) {
                        let text: String = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        if !text.is_empty() && text.len() > 10 && !all_headlines.contains(&text) {
                            all_headlines.push(text);
                        }
                    }
                }
                if all_headlines.len() >= 8 { break; }
            }
        }
    }

    // 2. Scrape injury report
    if let Ok(resp) = client
        .get("https://www.dailyfaceoff.com/nhl-injury-report")
        .header("User-Agent", "Mozilla/5.0 (compatible; FantasyHockeyBot/1.0)")
        .send()
        .await
    {
        if let Ok(html) = resp.text().await {
            let document = scraper::Html::parse_document(&html);
            // Try to get injury entries — format varies but typically has player name + status
            let selectors = [
                ".injury-table tr",
                ".player-injury-item",
                "table.injuries tr",
                ".injury-report-card",
            ];
            for sel_str in &selectors {
                if let Ok(selector) = scraper::Selector::parse(sel_str) {
                    for element in document.select(&selector) {
                        let text: String = element.text().collect::<Vec<_>>().join(" ");
                        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                        if text.len() > 15 && (text.contains("Injured") || text.contains("Day-to-Day") || text.contains("IR") || text.contains("Out") || text.contains("Game Time Decision") || text.contains("GTD")) {
                            let injury_note = format!("[INJURY] {}", text.trim());
                            if !all_headlines.contains(&injury_note) {
                                all_headlines.push(injury_note);
                            }
                        }
                    }
                }
                if all_headlines.len() >= 15 { break; }
            }
        }
    }

    all_headlines.truncate(15);
    if all_headlines.is_empty() {
        warn!("Headline scraper returned 0 results — DailyFaceoff selectors may be broken");
    }
    Ok(all_headlines)
}

// ---------------------------------------------------------------------------
// Ownership mapping helper
// ---------------------------------------------------------------------------

async fn build_ownership_map(state: &Arc<AppState>, league_id: &str) -> HashMap<i64, String> {
    let mut map = HashMap::new();
    if let Ok(groups) = state.db.get_nhl_teams_and_players(league_id).await {
        for group in groups {
            for player in group.players {
                map.insert(player.nhl_id, player.fantasy_team_name);
            }
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Narrative generation via Claude API
// ---------------------------------------------------------------------------

async fn generate_narratives(signals: &InsightsSignals) -> InsightsNarratives {
    match call_claude_api(signals).await {
        Ok(n) => n,
        Err(e) => {
            error!("Failed to generate narratives: {}", e);
            fallback_narratives()
        }
    }
}

fn fallback_narratives() -> InsightsNarratives {
    let msg = "Unable to generate insights at this time.".to_string();
    InsightsNarratives {
        todays_watch: msg.clone(),
        game_narratives: Vec::new(),
        hot_players: msg.clone(),
        cup_contenders: msg.clone(),
        fantasy_race: msg.clone(),
        sleeper_watch: msg,
    }
}

async fn call_claude_api(signals: &InsightsSignals) -> std::result::Result<InsightsNarratives, String> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY not set".to_string())?;

    let signals_json = serde_json::to_string(signals)
        .map_err(|e| format!("Failed to serialize signals: {}", e))?;

    // Build human-readable game summaries for Claude
    let mut game_summaries = String::new();
    for g in &signals.todays_games {
        game_summaries.push_str(&format!("\n--- {} ({}) @ {} ({}) at {} ---\n", g.away_team, g.away_record, g.home_team, g.home_record, g.venue));
        if let Some(ctx) = &g.series_context { game_summaries.push_str(&format!("  Series: {}{}\n", ctx, if g.is_elimination { " [ELIMINATION GAME]" } else { "" })); }
        // Standings context
        if let Some(ref streak) = g.home_streak { game_summaries.push_str(&format!("  {} streak: {}", g.home_team, streak)); }
        if let Some(ref l10) = g.home_l10 { game_summaries.push_str(&format!(", L10: {}", l10)); }
        if g.home_streak.is_some() { game_summaries.push('\n'); }
        if let Some(ref streak) = g.away_streak { game_summaries.push_str(&format!("  {} streak: {}", g.away_team, streak)); }
        if let Some(ref l10) = g.away_l10 { game_summaries.push_str(&format!(", L10: {}", l10)); }
        if g.away_streak.is_some() { game_summaries.push('\n'); }
        // Last game results
        if let Some(ref res) = g.home_last_result { game_summaries.push_str(&format!("  {} last game: {}\n", g.home_team, res)); }
        if let Some(ref res) = g.away_last_result { game_summaries.push_str(&format!("  {} last game: {}\n", g.away_team, res)); }
        if let Some((ref a, ref h)) = g.points_leaders { game_summaries.push_str(&format!("  Points (L5): {} {} ({}) vs {} {} ({})\n", a.name, a.position, a.value, h.name, h.position, h.value)); }
        if let Some((ref a, ref h)) = g.goals_leaders { game_summaries.push_str(&format!("  Goals (L5): {} ({}) vs {} ({})\n", a.name, a.value, h.name, h.value)); }
        if let Some((ref a, ref h)) = g.assists_leaders { game_summaries.push_str(&format!("  Assists (L5): {} ({}) vs {} ({})\n", a.name, a.value, h.name, h.value)); }
        if let (Some(ref ag), Some(ref hg)) = (&g.away_goalie, &g.home_goalie) {
            game_summaries.push_str(&format!("  Goalies: {} ({}, {:.2} GAA, {:.3} SV%) vs {} ({}, {:.2} GAA, {:.3} SV%)\n", ag.name, ag.record, ag.gaa, ag.save_pctg, hg.name, hg.record, hg.gaa, hg.save_pctg));
        }
    }
    // Hot player edge data for Claude context
    let mut edge_summary = String::new();
    for p in &signals.hot_players {
        if p.top_speed.is_some() || p.top_shot_speed.is_some() {
            edge_summary.push_str(&format!("  {} -", p.name));
            if let Some(spd) = p.top_speed { edge_summary.push_str(&format!(" top skating speed: {:.1} mph", spd)); }
            if let Some(shot) = p.top_shot_speed { edge_summary.push_str(&format!(" top shot speed: {:.1} mph", shot)); }
            edge_summary.push('\n');
        }
    }

    let num_games = signals.todays_games.len();

    let request_body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 3072,
        "system": format!(r#"You're a veteran hockey analyst writing for a small friend-group fantasy league newsletter. Your style is knowledgeable, engaging, and opinionated — like a bar conversation with a hockey encyclopedia.

CRITICAL RULES:
- ONLY reference stats, player names, records, and facts from the data provided below
- NEVER make up or hallucinate any statistics, records, or player information
- If data is missing or empty, say "stats aren't available yet" rather than inventing numbers
- Wrap ALL player names in **double asterisks** for bold formatting (e.g. **Connor McDavid**)

Return JSON with these exact fields:

- **todays_watch**: A brief 1-2 sentence overview of today's slate — the big picture.

- **game_narratives**: An array of exactly {num_games} strings, one per game IN THE SAME ORDER as the games listed below. Each string should be 2-3 punchy sentences previewing that specific matchup: key players, goalie edge, series stakes, streaks, and last game results. Do NOT start with the team matchup (e.g. "CBJ @ BUF:") — the matchup is already shown in the UI header. Jump straight into the analysis.

- **hot_players**: 3-4 sentences analyzing the hottest players. Reference actual form stats and NHL Edge data (skating speed, shot speed) when available. Mention fantasy team owners.

- **cup_contenders**: 3-4 sentences on playoff series using actual records and goalie stats.

- **fantasy_race**: 3-4 sentences on the fantasy race using actual point totals.

- **sleeper_watch**: 3-4 sentences on sleeper picks using actual stats."#),
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "Generate insights as JSON.\n\n=== TODAY'S GAMES ({num_games} games — generate exactly {num_games} game_narratives in this order) ==={}\n\n=== NHL EDGE DATA ===\n{}\n\n=== FULL DATA ===\n{}",
                    game_summaries, edge_summary, signals_json
                )
            }
        ]
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", &api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Claude API request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Claude API returned {}: {}", status, body));
    }

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse Claude API response: {}", e))?;

    // Extract text content from the response
    let text = body
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|block| block.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| "No text content in Claude API response".to_string())?;

    // Try to extract JSON from the response (it may be wrapped in markdown code blocks)
    let json_str = extract_json_from_text(text);

    let parsed: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse Claude response as JSON: {} — raw: {}", e, text))?;

    let game_narratives: Vec<String> = parsed
        .get("game_narratives")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();

    Ok(InsightsNarratives {
        todays_watch: parsed
            .get("todays_watch")
            .and_then(|v| v.as_str())
            .unwrap_or("No update available.")
            .to_string(),
        game_narratives,
        hot_players: parsed
            .get("hot_players")
            .and_then(|v| v.as_str())
            .unwrap_or("No update available.")
            .to_string(),
        cup_contenders: parsed
            .get("cup_contenders")
            .and_then(|v| v.as_str())
            .unwrap_or("No update available.")
            .to_string(),
        fantasy_race: parsed
            .get("fantasy_race")
            .and_then(|v| v.as_str())
            .unwrap_or("No update available.")
            .to_string(),
        sleeper_watch: parsed
            .get("sleeper_watch")
            .and_then(|v| v.as_str())
            .unwrap_or("No update available.")
            .to_string(),
    })
}

/// Extract JSON from text that may be wrapped in markdown code blocks.
fn extract_json_from_text(text: &str) -> String {
    let trimmed = text.trim();

    // Try to find JSON in a code block
    if let Some(start) = trimmed.find("```json") {
        let after_marker = &trimmed[start + 7..];
        if let Some(end) = after_marker.find("```") {
            return after_marker[..end].trim().to_string();
        }
    }
    if let Some(start) = trimmed.find("```") {
        let after_marker = &trimmed[start + 3..];
        if let Some(end) = after_marker.find("```") {
            return after_marker[..end].trim().to_string();
        }
    }

    // If it starts with { assume it's raw JSON
    if trimmed.starts_with('{') {
        return trimmed.to_string();
    }

    // Last resort: try to find a JSON object in the text
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
}
