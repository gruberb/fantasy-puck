use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
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
    let (hot, cold, projections, games, news, last_night) = tokio::join!(
        compute_hot_players(state, league_id),
        compute_cold_hands(state, league_id),
        compute_series_projections(state),
        compute_todays_games(state, hockey_today),
        scrape_headlines(),
        compute_last_night(state, hockey_today),
    );

    let mut todays_games = games.unwrap_or_default();
    enrich_games_with_ownership(state, league_id, &mut todays_games).await;

    let mut series_projections = projections.unwrap_or_default();
    enrich_projections(state, league_id, &mut series_projections).await;

    let hot_players = hot.unwrap_or_default();

    Ok(InsightsSignals {
        hot_players,
        cold_hands: cold.unwrap_or_default(),
        series_projections,
        todays_games,
        news_headlines: news.unwrap_or_default(),
        last_night: last_night.unwrap_or_default(),
    })
}

/// Recap of games that finalised on the hockey-date preceding `today`.
/// Returns an empty list on the first hockey-day of a round (no prior
/// date) and when the previous slate was off / still live. Every entry
/// carries the final score, the top 1–3 scorers, and the resulting
/// series state so the narrator doesn't need to parse scores directly.
async fn compute_last_night(
    state: &Arc<AppState>,
    today: &str,
) -> Result<Vec<crate::api::dtos::insights::LastNightGame>> {
    use crate::api::dtos::insights::{LastNightGame, LastNightScorer};
    use crate::infra::db::nhl_mirror;

    let yesterday = match chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d") {
        Ok(d) => match d.pred_opt() {
            Some(p) => p.format("%Y-%m-%d").to_string(),
            None => return Ok(Vec::new()),
        },
        Err(_) => return Ok(Vec::new()),
    };

    // Clamp to the playoff window so pre-playoff days don't get recapped
    // once the cutover happens. Callers already scope other reads the
    // same way.
    if yesterday.as_str() < crate::api::playoff_start() {
        return Ok(Vec::new());
    }

    let pool = state.db.pool();
    let games = nhl_mirror::list_games_for_date(pool, &yesterday).await?;
    let finals: Vec<&nhl_mirror::NhlGameRow> = games
        .iter()
        .filter(|g| matches!(g.game_state.as_str(), "OFF" | "FINAL"))
        .collect();
    if finals.is_empty() {
        return Ok(Vec::new());
    }

    let game_ids: Vec<i64> = finals.iter().map(|g| g.game_id).collect();
    let player_stats = nhl_mirror::list_player_game_stats_for_games(pool, &game_ids)
        .await
        .unwrap_or_default();

    let mut by_game: HashMap<i64, Vec<&nhl_mirror::PlayerGameStatRow>> = HashMap::new();
    for stat in &player_stats {
        if stat.points > 0 {
            by_game.entry(stat.game_id).or_default().push(stat);
        }
    }

    let recaps: Vec<LastNightGame> = finals
        .iter()
        .map(|g| {
            let mut top = by_game.remove(&g.game_id).unwrap_or_default();
            top.sort_by(|a, b| b.points.cmp(&a.points).then(b.goals.cmp(&a.goals)));
            let top_scorers: Vec<LastNightScorer> = top
                .into_iter()
                .take(3)
                .map(|s| LastNightScorer {
                    name: s.name.clone(),
                    team: s.team_abbrev.clone(),
                    goals: s.goals,
                    assists: s.assists,
                    points: s.points,
                })
                .collect();

            let (hs, as_) = (g.home_score.unwrap_or(0), g.away_score.unwrap_or(0));
            let headline = if hs >= as_ {
                format!("{} wins {}-{}", g.home_team, hs, as_)
            } else {
                format!("{} wins {}-{}", g.away_team, as_, hs)
            };

            let series_after = g.series_status.as_ref().and_then(|v| {
                let s: Option<crate::domain::models::nhl::SeriesStatus> =
                    serde_json::from_value(v.clone()).ok();
                s.map(|s| {
                    if s.top_seed_wins == s.bottom_seed_wins {
                        format!("Series tied {}-{}", s.top_seed_wins, s.bottom_seed_wins)
                    } else if s.top_seed_wins > s.bottom_seed_wins {
                        format!(
                            "{} leads series {}-{}",
                            s.top_seed_team_abbrev,
                            s.top_seed_wins,
                            s.bottom_seed_wins
                        )
                    } else {
                        format!(
                            "{} leads series {}-{}",
                            s.bottom_seed_team_abbrev,
                            s.bottom_seed_wins,
                            s.top_seed_wins
                        )
                    }
                })
            });

            LastNightGame {
                home_team: g.home_team.clone(),
                away_team: g.away_team.clone(),
                home_score: hs,
                away_score: as_,
                headline,
                series_after,
                top_scorers,
            }
        })
        .collect();

    Ok(recaps)
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
    // fall back to the standings-points blend. Standings JSON is
    // reconstructed from the mirrored nhl_standings table — zero NHL
    // calls at request time.
    let pool = state.db.pool();
    let ratings: HashMap<String, f32> = match crate::infra::db::nhl_mirror::load_standings_payload(
        pool,
        crate::api::season() as i32,
    )
    .await
    {
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
                    // Key by (fantasy_team_id, fantasy_team_name) so the
                    // id survives into the tag for frontend deep-linking.
                    let mut by_abbrev: HashMap<String, HashMap<(i64, String), usize>> =
                        HashMap::new();
                    for team in &teams {
                        for player in &team.players {
                            *by_abbrev
                                .entry(player.nhl_team.clone())
                                .or_default()
                                .entry((team.team_id, team.team_name.clone()))
                                .or_insert(0) += 1;
                        }
                    }
                    by_abbrev
                        .into_iter()
                        .map(|(abbrev, counts)| {
                            let mut tags: Vec<_> = counts
                                .into_iter()
                                .map(
                                    |((fantasy_team_id, fantasy_team_name), count)| {
                                        crate::api::dtos::insights::RosteredPlayerTag {
                                            fantasy_team_id,
                                            fantasy_team_name,
                                            count,
                                        }
                                    },
                                )
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

async fn compute_hot_players(
    state: &Arc<AppState>,
    league_id: &str,
) -> Result<Vec<HotPlayerSignal>> {
    let pool = state.db.pool();

    // Top-20 season leaders come from the mirror. The meta poller writes
    // nhl_skater_season_stats every ~30 minutes, so this read is always
    // a single indexed scan rather than a live NHL call.
    let leaders = crate::infra::db::nhl_mirror::list_skater_season_stats(
        pool,
        season() as i32,
        game_type() as i16,
    )
    .await
    .unwrap_or_default();

    // Playoffs haven't produced data yet → empty Hot card. The UI
    // renders a "playoffs haven't started" empty state; we no longer
    // fall back to the regular-season leaderboard.
    if leaders.iter().all(|r| r.points == 0) {
        return Ok(Vec::new());
    }

    let top_players: Vec<_> = leaders.into_iter().take(20).collect();

    // Build fantasy ownership mapping if league is provided
    let ownership: HashMap<i64, String> = if !league_id.is_empty() {
        build_ownership_map(state, league_id).await
    } else {
        HashMap::new()
    };

    // Form data (last 5 completed games per player) also from the
    // mirror. Previously this was 20 sequential NHL calls per cache
    // miss, which routinely tripped the per-IP rate limit and blocked
    // the daily prewarm from writing its cache — cascading into every
    // user request re-running the whole pipeline including Claude.
    let top_player_ids: Vec<i64> = top_players.iter().map(|p| p.player_id).collect();
    let form_rows = crate::infra::db::nhl_mirror::list_player_form(pool, &top_player_ids, 5)
        .await
        .unwrap_or_default();
    let form_by_player: HashMap<i64, (i32, i32, i32, usize)> = form_rows
        .into_iter()
        .map(|r| {
            (
                r.player_id,
                (
                    r.goals as i32,
                    r.assists as i32,
                    r.points as i32,
                    r.games as usize,
                ),
            )
        })
        .collect();

    let mut signals: Vec<(i64, HotPlayerSignal)> = Vec::with_capacity(top_players.len());
    for player in &top_players {
        let name = format!("{} {}", player.first_name, player.last_name);
        let (form_goals, form_assists, form_points, form_games) = form_by_player
            .get(&player.player_id)
            .copied()
            .unwrap_or((0, 0, 0, 0));
        signals.push((
            player.player_id,
            HotPlayerSignal {
                nhl_id: player.player_id,
                name,
                nhl_team: player.team_abbrev.clone(),
                position: player.position.clone(),
                form_goals,
                form_assists,
                form_points,
                form_games,
                playoff_points: player.points,
                fantasy_team: ownership.get(&player.player_id).cloned(),
                image_url: state.nhl_client.get_player_image_url(player.player_id),
                top_speed: None,
                top_shot_speed: None,
            },
        ));
    }

    // Sort by form points desc, take top 5
    signals.sort_by(|a, b| b.1.form_points.cmp(&a.1.form_points));
    signals.truncate(5);

    // Edge data (top skating speed, top shot speed) comes from the
    // mirror — the nightly edge_refresher job writes nhl_skater_edge at
    // 09:30 UTC. Zero NHL calls in the request path. Missing rows just
    // leave the speed tiles blank, which is preferable to blocking the
    // page while we fan out to the NHL Edge endpoint.
    let top5_ids: Vec<i64> = signals.iter().map(|(pid, _)| *pid).collect();
    let edge_rows = crate::infra::db::nhl_mirror::list_skater_edge(pool, &top5_ids)
        .await
        .unwrap_or_default();
    let edge_by_player: HashMap<i64, (Option<f32>, Option<f32>)> = edge_rows
        .into_iter()
        .map(|r| (r.player_id, (r.top_speed_mph, r.top_shot_speed_mph)))
        .collect();

    let signals: Vec<HotPlayerSignal> = signals
        .into_iter()
        .map(|(pid, mut signal)| {
            if let Some((speed, shot)) = edge_by_player.get(&pid) {
                signal.top_speed = speed.map(|v| v as f64);
                signal.top_shot_speed = shot.map(|v| v as f64);
            }
            signal
        })
        .collect();

    Ok(signals)
}

// ---------------------------------------------------------------------------
// Today's games
// ---------------------------------------------------------------------------

async fn compute_todays_games(
    state: &Arc<AppState>,
    hockey_today: &str,
) -> Result<Vec<TodaysGameSignal>> {
    let pool = state.db.pool();

    // Today's slate, standings context, and yesterday's results all
    // come from the mirror. No NHL calls in the request path.
    let games = crate::infra::db::nhl_mirror::list_games_for_date(pool, hockey_today)
        .await
        .unwrap_or_default();

    let yesterday = {
        let date = chrono::NaiveDate::parse_from_str(hockey_today, "%Y-%m-%d")
            .unwrap_or_else(|_| {
                Utc::now()
                    .with_timezone(&chrono_tz::America::New_York)
                    .date_naive()
            });
        (date - chrono::Duration::days(1)).format("%Y-%m-%d").to_string()
    };

    // Standings context per team. In playoff mode the streak is
    // computed from `nhl_games` so it reflects this postseason — the
    // `nhl_standings.streak_code` column is regular-season-only and
    // freezes at season's end, which would mislabel an eliminated team
    // as `W1` if they happened to win their final regular-season game.
    // L10 is dropped in playoff mode for the same reason: it's a
    // regular-season concept and the series banner already conveys
    // recent form.
    let is_playoffs = crate::api::game_type() == 3;
    let standings_map: HashMap<String, (String, String)> = if is_playoffs {
        let streaks = crate::infra::db::nhl_mirror::list_team_playoff_streaks(
            pool,
            crate::api::season() as i32,
        )
        .await
        .unwrap_or_default();
        streaks
            .into_iter()
            .map(|(team, streak)| (team, (streak, String::new())))
            .collect()
    } else {
        let standings_rows = crate::infra::db::nhl_mirror::list_team_standings_context(
            pool,
            crate::api::season() as i32,
        )
        .await
        .unwrap_or_default();
        standings_rows
            .into_iter()
            .map(|r| {
                let streak = match (r.streak_code.as_deref(), r.streak_count) {
                    (Some(code), Some(n)) if !code.is_empty() => format!("{}{}", code, n),
                    _ => String::new(),
                };
                let l10 = format!(
                    "{}-{}-{}",
                    r.l10_wins.unwrap_or(0),
                    r.l10_losses.unwrap_or(0),
                    r.l10_ot_losses.unwrap_or(0)
                );
                (r.team_abbrev, (streak, l10))
            })
            .collect()
    };

    // Yesterday's final scores → "W 4-2 vs OPP" caption per team.
    // Mirror's `list_games_for_date` already carries home/away scores
    // on every row (live poller writes them on game-end), so we only
    // need one query here instead of the old NHL `/score/{date}` call.
    let yesterday_games = crate::infra::db::nhl_mirror::list_games_for_date(pool, &yesterday)
        .await
        .unwrap_or_default();
    let last_result_map: HashMap<String, String> = build_last_result_map(&yesterday_games);

    let mut signals: Vec<TodaysGameSignal> = Vec::with_capacity(games.len());

    for game in &games {
        let home = game.home_team.clone();
        let away = game.away_team.clone();

        // Series status was stored as JSON by the meta poller;
        // deserialize back to the typed shape for the label.
        let (series_context, is_elimination) = series_context_from_row(game);

        // Pre-game matchup (leaders + goalies + records) from the
        // write-once `nhl_game_landing` mirror row. Venue comes
        // directly from the game row so an empty matchup still
        // yields a correct address in the header.
        let landing_matchup = crate::infra::db::nhl_mirror::get_game_landing_matchup(
            pool, game.game_id,
        )
        .await
        .ok()
        .flatten();
        let landing = landing_matchup
            .as_ref()
            .map(build_landing_from_matchup)
            .unwrap_or_default();

        // Empty strings collapse to `None` so the API response and the
        // Claude prompt builder both treat "no data" uniformly. In
        // playoff mode the standings_map carries an empty L10 by
        // construction (see comment near `is_playoffs` above).
        let lift = |s: &String| (!s.is_empty()).then(|| s.clone());
        let (home_streak, home_l10) = standings_map
            .get(&home)
            .map(|(s, l)| (lift(s), lift(l)))
            .unwrap_or((None, None));
        let (away_streak, away_l10) = standings_map
            .get(&away)
            .map(|(s, l)| (lift(s), lift(l)))
            .unwrap_or((None, None));

        let home_last_result = last_result_map.get(&home).cloned();
        let away_last_result = last_result_map.get(&away).cloned();

        signals.push(TodaysGameSignal {
            home_team: home,
            away_team: away,
            home_record: landing.home_record,
            away_record: landing.away_record,
            venue: game.venue.clone().unwrap_or_default(),
            start_time: game.start_time_utc.to_rfc3339(),
            series_context,
            is_elimination,
            points_leaders: landing.points_leaders,
            goals_leaders: landing.goals_leaders,
            assists_leaders: landing.assists_leaders,
            home_goalie: landing.home_goalie,
            away_goalie: landing.away_goalie,
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

/// Build the "yesterday's result" caption per team from the mirror's
/// game rows. Only completed games (state OFF/FINAL) contribute —
/// LIVE-state rows lurking on the wrong date would otherwise render
/// a partial score as if it were final.
fn build_last_result_map(
    games: &[crate::infra::db::nhl_mirror::NhlGameRow],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for g in games {
        if !matches!(g.game_state.as_str(), "OFF" | "FINAL") {
            continue;
        }
        let (Some(home_score), Some(away_score)) = (g.home_score, g.away_score) else {
            continue;
        };
        let (home_result, away_result) = if home_score > away_score {
            (
                format!("W {}-{} vs {}", home_score, away_score, g.away_team),
                format!("L {}-{} @ {}", away_score, home_score, g.home_team),
            )
        } else {
            (
                format!("L {}-{} vs {}", home_score, away_score, g.away_team),
                format!("W {}-{} @ {}", away_score, home_score, g.home_team),
            )
        };
        map.insert(g.home_team.clone(), home_result);
        map.insert(g.away_team.clone(), away_result);
    }
    map
}

/// Deserialise the JSONB `series_status` column on an `nhl_games` row
/// back to the typed shape and format the "PIT leads 2-1" caption.
/// Returns `(None, false)` if the column is null (regular-season game
/// or a playoff game before Round 1 seeding was published).
fn series_context_from_row(
    game: &crate::infra::db::nhl_mirror::NhlGameRow,
) -> (Option<String>, bool) {
    let Some(raw) = game.series_status.as_ref() else {
        return (None, false);
    };
    let ss: crate::domain::models::nhl::SeriesStatus = match serde_json::from_value(raw.clone()) {
        Ok(v) => v,
        Err(_) => return (None, false),
    };
    let leader = if ss.top_seed_wins >= ss.bottom_seed_wins {
        &ss.top_seed_team_abbrev
    } else {
        &ss.bottom_seed_team_abbrev
    };
    let context = format!(
        "{} - {} leads {}-{}",
        ss.series_title,
        leader,
        ss.top_seed_wins.max(ss.bottom_seed_wins),
        ss.top_seed_wins.min(ss.bottom_seed_wins),
    );
    let elim = ss.top_seed_wins == 3 || ss.bottom_seed_wins == 3;
    (Some(context), elim)
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

    // Build nhl_abbrev → {(fantasy_team_id, name) → player_count}.
    // Keying on the tuple keeps the id around so each tag carries a
    // deep-linkable team_id for the frontend.
    let mut by_nhl: HashMap<String, HashMap<(i64, String), usize>> = HashMap::new();
    for team in &ft {
        for p in &team.players {
            *by_nhl
                .entry(p.nhl_team.clone())
                .or_default()
                .entry((team.team_id, team.team_name.clone()))
                .or_insert(0) += 1;
        }
    }

    for g in signals.iter_mut() {
        let mut tags: Vec<crate::api::dtos::insights::RosteredPlayerTag> = Vec::new();
        for abbrev in [&g.home_team, &g.away_team] {
            if let Some(counts) = by_nhl.get(abbrev) {
                for ((ft_id, fantasy_name), count) in counts {
                    if let Some(existing) = tags.iter_mut().find(|t| t.fantasy_team_id == *ft_id) {
                        existing.count += count;
                    } else {
                        tags.push(crate::api::dtos::insights::RosteredPlayerTag {
                            fantasy_team_id: *ft_id,
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

/// Subset of the NHL game-landing response that Insights displays for
/// each game card — skater leaders, goalies, team records. Venue lives
/// on the game row itself so it isn't duplicated here.
///
/// The matchup JSON is captured by the meta poller (and admin rehydrate)
/// at the first tick that sees a game in FUT/PRE state, stored in
/// `nhl_game_landing`. Capture is write-once: once the NHL response
/// swaps from pre-game matchup to live recap, we never overwrite the
/// mirror row, so the sidebar stays populated for the full hockey-date.
#[derive(Default, Clone)]
struct LandingCached {
    home_record: String,
    away_record: String,
    points_leaders: Option<(PlayerLeader, PlayerLeader)>,
    goals_leaders: Option<(PlayerLeader, PlayerLeader)>,
    assists_leaders: Option<(PlayerLeader, PlayerLeader)>,
    home_goalie: Option<GoalieStats>,
    away_goalie: Option<GoalieStats>,
}

fn build_landing_from_matchup(matchup: &serde_json::Value) -> LandingCached {
    let mut out = LandingCached::default();

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

fn extract_player_leader(leader: &serde_json::Value, side: &str) -> Option<PlayerLeader> {
    let p = leader.get(side)?;
    Some(PlayerLeader {
        player_id: p.get("playerId").and_then(|v| v.as_i64()),
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
        last_night: String::new(),
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

    // Human-readable last-night recaps. We hand Claude the scores, the
    // post-game series state, and the top scorers; the narrator chooses
    // which games are worth a sub-heading and writes the prose.
    let mut last_night_summary = String::new();
    for g in &signals.last_night {
        last_night_summary.push_str(&format!(
            "\n--- {} @ {} · Final {} {} – {} {} · {} ---\n",
            g.away_team, g.home_team, g.away_team, g.away_score, g.home_team, g.home_score, g.headline
        ));
        if let Some(s) = &g.series_after {
            last_night_summary.push_str(&format!("  Series: {}\n", s));
        }
        for scorer in &g.top_scorers {
            last_night_summary.push_str(&format!(
                "  {} ({}): {}G {}A, {} pts\n",
                scorer.name, scorer.team, scorer.goals, scorer.assists, scorer.points
            ));
        }
    }

    let num_games = signals.todays_games.len();
    let num_last_night = signals.last_night.len();

    let request_body = serde_json::json!({
        "model": "claude-haiku-4-5-20251001",
        "max_tokens": 3072,
        "system": format!(r#"You are a veteran hockey columnist — think The Athletic, or a barstool analyst who actually watches every game. Dry, specific, opinionated, grounded in the numbers provided. You do NOT write like a marketing bot: no "dive in", "unleash", "game-changer", "exciting journey", hype adjectives, or bulleted listicles. Short punchy sentences mixed with longer analytical ones. Opinions should follow from the data — state them flatly, not breathlessly.

HARD RULES:
- Only reference stats, player names, records, and facts from the data provided below.
- Never make up or hallucinate statistics, records, or player information.
- If data is missing, say "stats aren't available yet" rather than inventing.
- Wrap player names in **double asterisks** for bold (e.g. **Connor McDavid**).
- Hot/Cold numbers are playoff totals. If `hotPlayers` is empty it's because no playoff data exists yet — acknowledge that and don't invent names.

This page is NHL-centric — the league-race narrative lives elsewhere. Stay out of fantasy-standings talk here; only note ownership tags when they sharpen an NHL story.

Return JSON with exactly these fields:

- **todays_watch**: 1–2 sentences previewing TODAY'S games specifically (the matchups listed under "TODAY'S GAMES" in the user message, {num_games} of them). Call out the biggest storyline of tonight's slate — a hot player, a lopsided matchup, an elimination game. Do NOT write about the full playoff bracket, the Cup race, or season-long team ratings; that belongs in `bracket`. If and only if `{num_games}` is 0, write exactly "No games on the slate today." — otherwise NEVER use that phrase.

- **game_narratives**: an array of exactly {num_games} strings, one per game in the same order. 2–3 sentences previewing each matchup — who's hot, where the edge is, what's at stake. Do NOT repeat the team names in the prefix; the header shows them.

- **hot_players**: 3–4 sentences on the hottest skaters. Cite form numbers and NHL Edge data when present. If the Hot list is empty, say the playoffs haven't produced data yet rather than padding.

- **bracket**: 3–4 sentences on the playoff picture — who's favored, where the upsets could come from, which team's Stanley Cup path looks easiest or hardest. Lean on series state + team ratings from the data. This is the one field where full-bracket / season-long talk belongs — everything else should stay game-scoped or player-scoped.

- **last_night**: Daily Faceoff-style recap of the {num_last_night} completed games under "LAST NIGHT". Format: one `### Headline` per game (name the story, not the teams — e.g. "Andersen slams the door on Ottawa" not "Hurricanes beat Senators"), followed by one 2–4 sentence paragraph covering what actually happened — final score, the turning moment, the top scorer, and the resulting series state. Voice is a veteran beat writer filing at midnight: specific, direct, no hype. Wrap player names in **double asterisks**. Skip hot takes about the whole series; this is a Day N recap, not a prediction. If `{num_last_night}` is 0, return an empty string for this field (not "No games last night", just `""`)."#),
        "messages": [
            {
                "role": "user",
                "content": format!(
                    "Generate insights as JSON.\n\n=== LAST NIGHT ({num_last_night} completed games) ==={}\n\n=== TODAY'S GAMES ({num_games} games — generate exactly {num_games} game_narratives in this order) ==={}\n\n=== NHL EDGE DATA ===\n{}\n\n=== FULL DATA ===\n{}",
                    last_night_summary, game_summaries, edge_summary, signals_json
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
        last_night: parsed
            .get("last_night")
            .and_then(|v| v.as_str())
            .unwrap_or("")
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
    let mut rostered: Vec<(i64, String, String, String, String)> = Vec::new();
    for g in groups {
        for p in g.players {
            // Goalies don't carry the scorer signal — filter here so
            // they never enter the form batch in the first place.
            if p.position.eq_ignore_ascii_case("G") {
                continue;
            }
            rostered.push((
                p.nhl_id,
                p.name.clone(),
                p.nhl_team.clone(),
                p.position.clone(),
                p.fantasy_team_name.clone(),
            ));
        }
    }

    // Batched L5 form from the mirror — one SQL query instead of N
    // NHL calls per rostered player. If the mirror has no data yet
    // (pre-first-playoff-game), the query returns nothing and the
    // card is empty — which is the correct product behaviour.
    let ids: Vec<i64> = rostered.iter().map(|(pid, _, _, _, _)| *pid).collect();
    let form_rows = crate::infra::db::nhl_mirror::list_player_form(state.db.pool(), &ids, 5)
        .await
        .unwrap_or_default();
    let form_by_player: HashMap<i64, (i64, i64, i64, i64)> = form_rows
        .into_iter()
        .map(|r| (r.player_id, (r.games, r.goals, r.assists, r.points)))
        .collect();

    let mut cold: Vec<HotPlayerSignal> = Vec::new();
    for (nhl_id, name, team, position, fantasy_team) in &rostered {
        let Some(&(games, goals, assists, points)) = form_by_player.get(nhl_id) else {
            continue;
        };
        if games < 3 {
            continue; // not enough sample size
        }
        if points > 1 {
            continue; // only show real slumps
        }
        cold.push(HotPlayerSignal {
            nhl_id: *nhl_id,
            name: name.clone(),
            nhl_team: team.clone(),
            position: position.clone(),
            form_goals: goals as i32,
            form_assists: assists as i32,
            form_points: points as i32,
            form_games: games as usize,
            playoff_points: 0,
            fantasy_team: Some(fantasy_team.clone()),
            image_url: state.nhl_client.get_player_image_url(*nhl_id),
            top_speed: None,
            top_shot_speed: None,
        });
    }
    // Coldest (lowest points) first; ties broken by larger sample (more games).
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
    use crate::infra::nhl::constants::team_names;
    use crate::domain::prediction::series_projection as sp;

    // Bracket comes straight from the mirror. Meta poller refreshes
    // nhl_playoff_bracket every aggregate cadence (~30 min), so this
    // is a single SELECT + deserialize rather than a live NHL call.
    let carousel = match crate::infra::db::nhl_mirror::get_playoff_carousel(
        state.db.pool(),
        season() as i32,
    )
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

