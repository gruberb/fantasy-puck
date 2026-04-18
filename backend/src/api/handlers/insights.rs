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

    // Check cache. Self-heal only on off-day responses: if the cached
    // payload has no games (e.g. 10am UTC prewarm ran before NHL
    // published today's schedule), regenerate. A cached response with
    // games — even one with partial landing data — is served as-is
    // for the rest of the hockey-date. Insights is a daily preview;
    // users don't expect it to change after the page first loads.
    if let Some(cached) = state
        .db
        .cache()
        .get_cached_response::<InsightsResponse>(&cache_key)
        .await?
    {
        if !cached.signals.todays_games.is_empty() {
            return Ok(cached);
        }
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

    // Cache whenever today has games. The previous version also gated
    // on "every game has some landing signal," which meant one rate-
    // limited landing fetch killed caching for the whole day and every
    // visitor regenerated the entire payload (including a Claude call).
    // A partial sidebar for one card is a far smaller problem than
    // re-running this for every request. Off-day responses still fall
    // through uncached so they regenerate once the schedule appears.
    if !response.signals.todays_games.is_empty() {
        let _ = state
            .db
            .cache()
            .store_response(&cache_key, &today, &response)
            .await;
    }

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
    // Run independent signal computations concurrently. Personal surfaces
    // (fantasy race, rivalry, sleeper watch) live on Pulse / race-odds now;
    // this signal set is strictly NHL-centric.
    let (hot, cold, projections, games, news) = tokio::join!(
        compute_hot_players(state, league_id),
        compute_cold_hands(state, league_id),
        compute_series_projections(state),
        compute_todays_games(state, hockey_today),
        scrape_headlines(),
    );

    let mut todays_games = games.unwrap_or_default();
    enrich_games_with_ownership(state, league_id, &mut todays_games).await;

    let mut series_projections = projections.unwrap_or_default();
    enrich_projections(state, league_id, &mut series_projections).await;

    let (hot_players, hot_cold_is_regular_season) = match hot {
        Ok(result) => (result.signals, result.is_regular_season),
        Err(_) => (Vec::new(), false),
    };

    Ok(InsightsSignals {
        hot_players,
        cold_hands: cold.unwrap_or_default(),
        series_projections,
        todays_games,
        news_headlines: news.unwrap_or_default(),
        hot_cold_is_regular_season,
    })
}

/// Fill in team strength (standings points) and fantasy ownership tags on
/// each series projection. Two NHL API hits (standings + roster mapping);
/// failures degrade gracefully to a bare projection.
async fn enrich_projections(
    state: &Arc<AppState>,
    league_id: &str,
    projections: &mut [TeamSeriesProjection],
) {
    if projections.is_empty() {
        return;
    }

    // Team ratings: during the playoffs use the dynamic playoff Elo
    // (same source the Stanley Cup Odds table reads), so both surfaces
    // on this page agree on which team is "stronger." Before playoffs
    // fall back to the standings-points blend.
    let ratings: HashMap<String, f32> = match state.nhl_client.get_standings_raw().await {
        Ok(json) => {
            if crate::api::game_type() == 3 {
                match crate::infra::prediction::compute_current_elo(
                    &state.db,
                    &json,
                    crate::api::season(),
                )
                .await
                {
                    Ok(elo) => elo,
                    Err(_) => crate::domain::prediction::team_ratings::from_standings(&json),
                }
            } else {
                crate::domain::prediction::team_ratings::from_standings(&json)
            }
        }
        Err(_) => HashMap::new(),
    };

    // Fantasy ownership map: nhl_team_abbrev -> [{fantasy_team_name, count}].
    let ownership: HashMap<String, Vec<crate::api::dtos::insights::RosteredPlayerTag>> =
        if league_id.is_empty() {
            HashMap::new()
        } else {
            let abbrevs: HashSet<&str> = projections.iter().map(|p| p.team_abbrev.as_str()).collect();
            let abbrev_vec: Vec<&str> = abbrevs.into_iter().collect();
            match state
                .db
                .get_fantasy_teams_for_nhl_teams(&abbrev_vec, league_id)
                .await
            {
                Ok(teams) => {
                    let mut by_abbrev: HashMap<String, HashMap<String, usize>> = HashMap::new();
                    for team in &teams {
                        for player in &team.players {
                            *by_abbrev
                                .entry(player.nhl_team.clone())
                                .or_default()
                                .entry(team.team_name.clone())
                                .or_insert(0) += 1;
                        }
                    }
                    by_abbrev
                        .into_iter()
                        .map(|(abbrev, counts)| {
                            let mut tags: Vec<_> = counts
                                .into_iter()
                                .map(|(fantasy_team_name, count)| {
                                    crate::api::dtos::insights::RosteredPlayerTag {
                                        fantasy_team_name,
                                        count,
                                    }
                                })
                                .collect();
                            tags.sort_by(|a, b| b.count.cmp(&a.count));
                            (abbrev, tags)
                        })
                        .collect()
                }
                Err(_) => HashMap::new(),
            }
        };

    for p in projections.iter_mut() {
        p.team_rating = ratings.get(&p.team_abbrev).copied();
        p.opponent_rating = ratings.get(&p.opponent_abbrev).copied();
        p.rostered_tags = ownership.get(&p.team_abbrev).cloned().unwrap_or_default();
    }
}

// ---------------------------------------------------------------------------
// Hot players (top 5 by recent form)
// ---------------------------------------------------------------------------

/// Return value from [`compute_hot_players`] carrying both the ranked
/// signals and a flag indicating whether the data came from regular-season
/// leaders (pre-playoff fallback). The flag bubbles up to `InsightsSignals`
/// so the UI and Claude narrative can use the correct "season pts" vs
/// "playoff pts" label.
struct HotPlayersResult {
    signals: Vec<HotPlayerSignal>,
    is_regular_season: bool,
}

async fn compute_hot_players(
    state: &Arc<AppState>,
    league_id: &str,
) -> Result<HotPlayersResult> {
    let playoff_stats = state
        .nhl_client
        .get_skater_stats(&season(), game_type())
        .await?;

    // Playoffs haven't produced any point totals yet — fall back to
    // regular-season leaders so the Hot card isn't empty the night before
    // Game 1. We also drop the "form L5" fetch in that case because playoff
    // game logs are empty; regular-season L5 comes instead.
    let playoff_has_data = playoff_stats
        .points
        .iter()
        .any(|p| (p.value as i32) > 0);
    let use_rs_fallback = !playoff_has_data;

    let (stats, fallback_game_type) = if use_rs_fallback {
        let rs = state
            .nhl_client
            .get_skater_stats(&season(), 2)
            .await?;
        (rs, 2u8)
    } else {
        (playoff_stats, game_type())
    };

    // Take top 20 by the relevant season's points.
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
                    .get_player_form(player_id, &season(), fallback_game_type, 5)
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
                nhl_id: player_id,
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

    Ok(HotPlayersResult {
        signals,
        is_regular_season: use_rs_fallback,
    })
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

        // Fetch rich landing data, cached per-game. Pre-game matchup
        // (leaders + goalies) only exists in the NHL landing response
        // while the game is FUT; once puck drops the response shape
        // swaps to live recap and the matchup block is gone. Capturing
        // it once — ideally at the 10am UTC prewarm before any game
        // starts — and never overwriting with an empty pull keeps the
        // sidebar intact for games that go live later in the day.
        let landing = get_or_fetch_landing_cached(state, game.id, hockey_today)
            .await
            .unwrap_or_else(LandingCached::default);
        let venue = landing.venue;
        let home_record = landing.home_record;
        let away_record = landing.away_record;
        let points_leaders = landing.points_leaders;
        let goals_leaders = landing.goals_leaders;
        let assists_leaders = landing.assists_leaders;
        let home_goalie = landing.home_goalie;
        let away_goalie = landing.away_goalie;

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
            rostered_player_tags: Vec::new(),
        });
    }

    Ok(signals)
}

/// Enrich each `TodaysGameSignal` with "your team has N players in this game"
/// tags for the given league. Called after `compute_todays_games` so it can
/// reuse the same signal list.
pub async fn enrich_games_with_ownership(
    state: &Arc<AppState>,
    league_id: &str,
    signals: &mut [TodaysGameSignal],
) {
    if league_id.is_empty() {
        return;
    }
    let mut nhl_teams = HashSet::<String>::new();
    for g in signals.iter() {
        nhl_teams.insert(g.home_team.clone());
        nhl_teams.insert(g.away_team.clone());
    }
    let abbrev_refs: Vec<&str> = nhl_teams.iter().map(|s| s.as_str()).collect();
    let ft = match state
        .db
        .get_fantasy_teams_for_nhl_teams(&abbrev_refs, league_id)
        .await
    {
        Ok(v) => v,
        Err(_) => return,
    };

    // Build team -> HashMap<fantasy_team_name, count>
    let mut by_nhl: HashMap<String, HashMap<String, usize>> = HashMap::new();
    for team in &ft {
        for p in &team.players {
            *by_nhl
                .entry(p.nhl_team.clone())
                .or_default()
                .entry(team.team_name.clone())
                .or_insert(0) += 1;
        }
    }

    for g in signals.iter_mut() {
        let mut tags = Vec::new();
        for abbrev in [&g.home_team, &g.away_team] {
            if let Some(counts) = by_nhl.get(abbrev) {
                for (fantasy_name, count) in counts {
                    if let Some(existing) = tags
                        .iter_mut()
                        .find(|t: &&mut crate::api::dtos::insights::RosteredPlayerTag| {
                            t.fantasy_team_name == *fantasy_name
                        })
                    {
                        existing.count += count;
                    } else {
                        tags.push(crate::api::dtos::insights::RosteredPlayerTag {
                            fantasy_team_name: fantasy_name.clone(),
                            count: *count,
                        });
                    }
                }
            }
        }
        tags.sort_by(|a, b| b.count.cmp(&a.count));
        g.rostered_player_tags = tags;
    }
}

/// Subset of the NHL game-landing response that Insights actually displays.
/// Stored in `response_cache` as `insights_landing:{game_id}` with write-once
/// semantics — see [`get_or_fetch_landing_cached`] — so that a game whose
/// pre-game matchup was captured earlier in the day keeps its sidebar even
/// after puck drop, when the live response no longer includes the block.
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
struct LandingCached {
    venue: String,
    home_record: String,
    away_record: String,
    points_leaders: Option<(PlayerLeader, PlayerLeader)>,
    goals_leaders: Option<(PlayerLeader, PlayerLeader)>,
    assists_leaders: Option<(PlayerLeader, PlayerLeader)>,
    home_goalie: Option<GoalieStats>,
    away_goalie: Option<GoalieStats>,
}

impl LandingCached {
    fn has_matchup(&self) -> bool {
        self.points_leaders.is_some()
            || self.goals_leaders.is_some()
            || self.assists_leaders.is_some()
            || self.home_goalie.is_some()
            || self.away_goalie.is_some()
    }
}

fn build_landing_from_raw(landing: &serde_json::Value) -> LandingCached {
    let venue = landing
        .get("venue")
        .and_then(|v| v.get("default"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut out = LandingCached {
        venue,
        ..LandingCached::default()
    };

    let Some(matchup) = landing.get("matchup") else {
        return out;
    };

    if let Some(leaders) = matchup
        .get("skaterComparison")
        .and_then(|s| s.get("leaders"))
        .and_then(|l| l.as_array())
    {
        for leader in leaders {
            let cat = leader.get("category").and_then(|c| c.as_str()).unwrap_or("");
            let al = extract_player_leader(leader, "awayLeader");
            let hl = extract_player_leader(leader, "homeLeader");
            if let (Some(a), Some(h)) = (al, hl) {
                match cat {
                    "points" => out.points_leaders = Some((a, h)),
                    "goals" => out.goals_leaders = Some((a, h)),
                    "assists" => out.assists_leaders = Some((a, h)),
                    _ => {}
                }
            }
        }
    }

    if let Some(gc) = matchup.get("goalieComparison") {
        out.home_goalie = extract_goalie_stats(gc, "homeTeam");
        out.away_goalie = extract_goalie_stats(gc, "awayTeam");
        out.home_record = gc
            .get("homeTeam")
            .and_then(|t| t.get("teamTotals"))
            .and_then(|t| t.get("record"))
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();
        out.away_record = gc
            .get("awayTeam")
            .and_then(|t| t.get("teamTotals"))
            .and_then(|t| t.get("record"))
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();
    }

    out
}

/// Return a cached landing if present (matchup populated), otherwise fetch
/// from NHL. Caches only when the fetched matchup is populated — i.e. the
/// game was still FUT at fetch time. Post-puck-drop fetches are returned to
/// the caller for one-shot use but not written, so a later prewarm or
/// request during a FUT window can still populate the cache correctly.
async fn get_or_fetch_landing_cached(
    state: &Arc<AppState>,
    game_id: u32,
    date: &str,
) -> Option<LandingCached> {
    let cache_key = format!("insights_landing:{}", game_id);

    if let Ok(Some(cached)) = state
        .db
        .cache()
        .get_cached_response::<LandingCached>(&cache_key)
        .await
    {
        if cached.has_matchup() {
            return Some(cached);
        }
    }

    let landing = match state.nhl_client.get_game_landing_raw(game_id).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                game_id = game_id,
                error = %e,
                "insights: game landing fetch failed; leaders/goalies will be null"
            );
            return None;
        }
    };

    let built = build_landing_from_raw(&landing);
    if built.has_matchup() {
        let _ = state
            .db
            .cache()
            .store_response(&cache_key, date, &built)
            .await;
    }
    Some(built)
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
// News headlines from Daily Faceoff
// ---------------------------------------------------------------------------

async fn scrape_headlines() -> Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(crate::tuning::http::HEADLINE_SCRAPER_TIMEOUT)
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

    all_headlines.truncate(10);
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
        bracket: msg,
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
        "system": format!(r#"You are a veteran hockey columnist — think The Athletic, or a barstool analyst who actually watches every game. Dry, specific, opinionated, grounded in the numbers provided. You do NOT write like a marketing bot: no "dive in", "unleash", "game-changer", "exciting journey", hype adjectives, or bulleted listicles. Short punchy sentences mixed with longer analytical ones. Opinions should follow from the data — state them flatly, not breathlessly.

HARD RULES:
- Only reference stats, player names, records, and facts from the data provided below.
- Never make up or hallucinate statistics, records, or player information.
- If data is missing, say "stats aren't available yet" rather than inventing.
- Wrap player names in **double asterisks** for bold (e.g. **Connor McDavid**).
- The `hotColdIsRegularSeason` flag in the signals tells you whether hot/cold totals are playoff points or regular-season points. If it's `true`, say "regular-season points" — NEVER "playoff points" — because the playoffs haven't produced data yet.

This page is NHL-centric — the league-race narrative lives elsewhere. Stay out of fantasy-standings talk here; only note ownership tags when they sharpen an NHL story.

Return JSON with exactly these fields:

- **todays_watch**: 1–2 sentences previewing TODAY'S games specifically (the matchups listed under "TODAY'S GAMES" in the user message, {num_games} of them). Call out the biggest storyline of tonight's slate — a hot player, a lopsided matchup, an elimination game. Do NOT write about the full playoff bracket, the Cup race, or season-long team ratings; that belongs in `bracket`. If and only if `{num_games}` is 0, write exactly "No games on the slate today." — otherwise NEVER use that phrase.

- **game_narratives**: an array of exactly {num_games} strings, one per game in the same order. 2–3 sentences previewing each matchup — who's hot, where the edge is, what's at stake. Do NOT repeat the team names in the prefix; the header shows them.

- **hot_players**: 3–4 sentences on the hottest skaters. Cite form numbers and NHL Edge data when present. Respect the `hotColdIsRegularSeason` flag: call it "regular-season points" when true.

- **bracket**: 3–4 sentences on the playoff picture — who's favored, where the upsets could come from, which team's Stanley Cup path looks easiest or hardest. Lean on series state + team ratings from the data. This is the one field where full-bracket / season-long talk belongs — everything else should stay game-scoped or player-scoped."#),
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
        .timeout(crate::tuning::http::CLAUDE_TIMEOUT)
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
        bracket: parsed
            .get("bracket")
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

// ---------------------------------------------------------------------------
// Cold Hands (rostered players slumping — min 3 games played to avoid noise)
// ---------------------------------------------------------------------------

async fn compute_cold_hands(
    state: &Arc<AppState>,
    league_id: &str,
) -> Result<Vec<HotPlayerSignal>> {
    if league_id.is_empty() {
        return Ok(Vec::new());
    }

    // Rostered players in this league.
    let groups = state.db.get_nhl_teams_and_players(league_id).await.unwrap_or_default();
    let mut rostered: Vec<(i64, String, String, String, String)> = Vec::new(); // (nhl_id, name, team, position, fantasy_team)
    for g in groups {
        for p in g.players {
            rostered.push((
                p.nhl_id,
                p.name.clone(),
                p.nhl_team.clone(),
                p.position.clone(),
                p.fantasy_team_name.clone(),
            ));
        }
    }

    // Probe one rostered player's playoff game log to decide whether
    // playoff data exists; if it's empty for the pilot we fall back to
    // regular-season game logs so the card isn't silent pre-playoffs.
    let playoff_has_data = if let Some((pid, _, _, _, _)) = rostered.first() {
        matches!(
            state
                .nhl_client
                .get_player_game_log(*pid, &season(), game_type())
                .await,
            Ok(log) if !log.game_log.is_empty()
        )
    } else {
        false
    };
    let log_game_type = if playoff_has_data { game_type() } else { 2u8 };

    // Fetch form (L5) for each rostered player, keep those with games>=3 and points<=1.
    let form_futures: Vec<_> = rostered
        .iter()
        .map(|(nhl_id, _, _, _, _)| {
            let state = Arc::clone(state);
            let pid = *nhl_id;
            async move {
                match state
                    .nhl_client
                    .get_player_game_log(pid, &season(), log_game_type)
                    .await
                {
                    Ok(log) => {
                        let recent: Vec<_> = log.game_log.iter().rev().take(5).collect();
                        let games = recent.len();
                        let goals: i32 = recent.iter().map(|g| g.goals).sum();
                        let assists: i32 = recent.iter().map(|g| g.assists).sum();
                        let points: i32 = recent.iter().map(|g| g.points).sum();
                        Some((pid, games, goals, assists, points))
                    }
                    Err(_) => None,
                }
            }
        })
        .collect();

    let results = join_all(form_futures).await;

    let mut cold: Vec<HotPlayerSignal> = Vec::new();
    for ((nhl_id, name, team, position, fantasy_team), res) in rostered.iter().zip(results) {
        let Some((_, games, goals, assists, points)) = res else { continue };
        if games < 3 {
            continue; // not enough sample size
        }
        if points > 1 {
            continue; // only show real slumps
        }
        // Skip goalies — the scorer signal doesn't apply.
        if position.eq_ignore_ascii_case("G") {
            continue;
        }
        cold.push(HotPlayerSignal {
            nhl_id: *nhl_id,
            name: name.clone(),
            nhl_team: team.clone(),
            position: position.clone(),
            form_goals: goals,
            form_assists: assists,
            form_points: points,
            form_games: games,
            playoff_points: 0,
            fantasy_team: Some(fantasy_team.clone()),
            image_url: state.nhl_client.get_player_image_url(*nhl_id),
            top_speed: None,
            top_shot_speed: None,
        });
    }
    // Surface the coldest (lowest points) first, then fewest points ties broken by fewer games.
    cold.sort_by(|a, b| a.form_points.cmp(&b.form_points).then(b.form_games.cmp(&a.form_games)));
    cold.truncate(8);
    Ok(cold)
}

// ---------------------------------------------------------------------------
// Series projections — every team in the current round with heuristic odds
// ---------------------------------------------------------------------------

async fn compute_series_projections(
    state: &Arc<AppState>,
) -> Result<Vec<TeamSeriesProjection>> {
    use crate::nhl_api::nhl_constants::team_names;
    use crate::domain::prediction::series_projection as sp;

    let carousel = match state
        .nhl_client
        .get_playoff_carousel(format!("{}", season()))
        .await?
    {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let current_round = carousel.current_round as u32;
    let round = match carousel
        .rounds
        .iter()
        .find(|r| r.round_number == current_round as i64)
    {
        Some(r) => r,
        None => return Ok(Vec::new()),
    };

    let mut out = Vec::new();
    for series in &round.series {
        let top_wins = series.top_seed.wins as u32;
        let bot_wins = series.bottom_seed.wins as u32;
        for (abbrev, wins, opp_abbrev, opp_wins) in [
            (&series.top_seed.abbrev, top_wins, &series.bottom_seed.abbrev, bot_wins),
            (&series.bottom_seed.abbrev, bot_wins, &series.top_seed.abbrev, top_wins),
        ] {
            let state_code = sp::classify(wins, opp_wins);
            out.push(TeamSeriesProjection {
                team_abbrev: abbrev.clone(),
                team_name: team_names::get_team_name(abbrev).to_string(),
                opponent_abbrev: opp_abbrev.clone(),
                opponent_name: team_names::get_team_name(opp_abbrev).to_string(),
                round: current_round,
                wins,
                opponent_wins: opp_wins,
                series_state: state_code,
                series_label: state_code.label(wins, opp_wins),
                odds_to_advance: sp::probability_to_advance(wins, opp_wins),
                games_remaining: sp::games_remaining(wins, opp_wins),
                team_rating: None,     // populated by enrich_projections
                opponent_rating: None, // populated by enrich_projections
                rostered_tags: Vec::new(),
            });
        }
    }

    // Leaders first — sort by wins desc, then by odds desc, then abbrev asc.
    out.sort_by(|a, b| {
        b.wins
            .cmp(&a.wins)
            .then(
                b.odds_to_advance
                    .partial_cmp(&a.odds_to_advance)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.team_abbrev.cmp(&b.team_abbrev))
    });
    let _ = state;
    Ok(out)
}

