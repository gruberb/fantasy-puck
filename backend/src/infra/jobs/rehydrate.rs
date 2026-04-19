//! Admin "rehydrate" — run every poller step synchronously plus any
//! one-shot backfills needed right after a deploy.
//!
//! Reachable via `GET /api/admin/rehydrate`. Safe to call repeatedly
//! (all writes are idempotent) but heavy on a cold mirror:
//! schedule-across-range + aggregates + 32 rosters + boxscores for
//! every known game. The pacing and freshness rules below keep a
//! repeat invocation cheap.

use std::sync::Arc;

use serde::Serialize;
use tracing::{info, warn};

use crate::infra::db::{nhl_mirror, FantasyDb};
use crate::infra::nhl::client::NhlClient;
use crate::tuning::live_mirror;

/// Summary shape returned to the admin caller. Counters are
/// best-effort; individual failures are logged and the function
/// keeps going rather than aborting the whole run.
#[derive(Debug, Default, Serialize)]
pub struct RehydrateSummary {
    pub games_upserted: usize,
    pub skater_rows: usize,
    pub goalie_rows: usize,
    pub standings_rows: usize,
    pub rosters_upserted: usize,
    pub rosters_skipped_fresh: bool,
    pub bracket_captured: bool,
    pub aggregates_skipped_fresh: bool,
    pub boxscore_games_processed: usize,
    pub boxscore_player_rows: usize,
    pub landing_captures: usize,
    pub errors: Vec<String>,
}

pub async fn run(db: &FantasyDb, nhl: Arc<NhlClient>) -> RehydrateSummary {
    let mut summary = RehydrateSummary::default();
    let season = crate::api::season();
    let game_type = crate::api::game_type();
    let pool = db.pool();

    // Freshness thresholds mirror meta_poller's cadences so hitting
    // rehydrate twice back-to-back after the meta poller has
    // populated everything is a cheap no-op rather than a 35-call
    // fan-out. These can still be overridden by a cold-start boot
    // where the tables are genuinely empty.
    let schedule_ttl = live_mirror::META_POLL_INTERVAL;
    let agg_ttl = live_mirror::META_POLL_INTERVAL
        * live_mirror::AGGREGATES_REFRESH_EVERY_N_META_TICKS;
    let roster_ttl = live_mirror::META_POLL_INTERVAL
        * live_mirror::ROSTER_REFRESH_EVERY_N_META_TICKS;

    // ---- Schedule: playoff start → today (+1). Per-date freshness
    // gate so a repeat rehydrate skips dates the meta poller just
    // wrote.
    //
    // ET-today, not UTC — same rationale as in meta_poller. NHL's
    // schedule endpoint keys games by ET local date.
    let today = chrono::Utc::now()
        .with_timezone(&chrono_tz::America::New_York)
        .date_naive();
    let mut dates: Vec<String> = Vec::new();
    let playoff_start = crate::api::playoff_start();
    let start_naive = chrono::NaiveDate::parse_from_str(playoff_start, "%Y-%m-%d")
        .unwrap_or(today);
    let mut cursor = if start_naive <= today { start_naive } else { today };
    while cursor <= today + chrono::Duration::days(1) {
        dates.push(cursor.format("%Y-%m-%d").to_string());
        cursor += chrono::Duration::days(1);
    }
    for date in &dates {
        let last = nhl_mirror::last_update_nhl_games_for_date(pool, date)
            .await
            .unwrap_or(None);
        if !nhl_mirror::is_stale(last, schedule_ttl) {
            continue;
        }
        match nhl.get_schedule_by_date(date).await {
            Ok(schedule) => {
                let games = schedule.games_for_date(date);
                for g in &games {
                    if let Err(e) = nhl_mirror::upsert_game(pool, g, date).await {
                        summary.errors.push(format!("upsert_game {}: {}", g.id, e));
                    } else {
                        summary.games_upserted += 1;
                    }
                }
            }
            Err(e) => summary
                .errors
                .push(format!("schedule {}: {}", date, e)),
        }
    }

    // ---- Aggregates (standings, leaderboards, bracket). Gated
    // behind a single freshness check on standings as a proxy —
    // these four tables move together on game-end events.
    let agg_last = nhl_mirror::last_update_nhl_standings(pool, season as i32)
        .await
        .unwrap_or(None);
    if nhl_mirror::is_stale(agg_last, agg_ttl) {
        match nhl.get_skater_stats(&season, game_type).await {
            Ok(leaders) => match nhl_mirror::upsert_skater_leaderboard(
                pool,
                season as i32,
                game_type as i16,
                &leaders,
            )
            .await
            {
                Ok(n) => summary.skater_rows = n,
                Err(e) => summary.errors.push(format!("skater upsert: {}", e)),
            },
            Err(e) => summary.errors.push(format!("skater leaderboard: {}", e)),
        }

        if let Ok(payload) = nhl.get_goalie_stats(&season, game_type).await {
            let json = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);
            match nhl_mirror::upsert_goalie_leaderboard(
                pool,
                season as i32,
                game_type as i16,
                &json,
            )
            .await
            {
                Ok(n) => summary.goalie_rows = n,
                Err(e) => summary.errors.push(format!("goalie upsert: {}", e)),
            }
        }

        if let Ok(payload) = nhl.get_standings_raw().await {
            match nhl_mirror::upsert_standings(pool, season as i32, &payload).await {
                Ok(n) => summary.standings_rows = n,
                Err(e) => summary.errors.push(format!("standings upsert: {}", e)),
            }
        }

        if game_type == 3 {
            if let Ok(Some(carousel)) = nhl.get_playoff_carousel(season.to_string()).await {
                let json = serde_json::to_value(&carousel).unwrap_or(serde_json::Value::Null);
                if nhl_mirror::upsert_playoff_bracket(pool, season as i32, &json)
                    .await
                    .is_ok()
                {
                    summary.bracket_captured = true;
                }
            }
        }
    } else {
        summary.aggregates_skipped_fresh = true;
    }

    // ---- Team rosters, paced (250 ms between calls) so NHL's
    // per-IP rate limit never trips. Gated on roster_ttl so
    // repeat runs within 24 h are no-ops.
    let roster_last = nhl_mirror::last_update_nhl_team_rosters(pool, season as i32)
        .await
        .unwrap_or(None);
    if nhl_mirror::is_stale(roster_last, roster_ttl) {
        if let Ok(teams) = nhl.get_all_teams().await {
            for (i, team) in teams.iter().enumerate() {
                if i > 0 {
                    tokio::time::sleep(live_mirror::ROSTER_FETCH_DELAY).await;
                }
                if let Ok(players) = nhl.get_team_roster(team).await {
                    if nhl_mirror::upsert_team_roster(pool, team, season as i32, &players)
                        .await
                        .is_ok()
                    {
                        summary.rosters_upserted += 1;
                    }
                }
            }
        }
    } else {
        summary.rosters_skipped_fresh = true;
    }

    // ---- Boxscores + landing for every game we know about.
    // For each game:
    //   - If state is FUT/PRE, try the landing (write-once).
    //   - Unless state is FUT, fetch the boxscore and upsert
    //     per-player stats. `upsert_boxscore_players` also
    //     derives home_score / away_score from the boxscore
    //     itself — that's what backfills scores for games that
    //     finalized before the live poller ever saw them.
    let game_rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT game_id, home_team, away_team, game_state FROM nhl_games",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    info!(games = game_rows.len(), "rehydrate: processing boxscores");

    for (gid, home, away, state) in &game_rows {
        if state == "FUT" || state == "PRE" {
            if let Ok(landing) = nhl.get_game_landing_raw(*gid as u32).await {
                let matchup = landing
                    .get("matchup")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
                if let Ok(true) = nhl_mirror::capture_game_landing(pool, *gid, &matchup).await {
                    summary.landing_captures += 1;
                }
            }
        }

        if state == "FUT" {
            continue;
        }
        match nhl.get_game_boxscore(*gid as u32).await {
            Ok(box_score) => {
                match nhl_mirror::upsert_boxscore_players(pool, *gid, home, away, &box_score).await
                {
                    Ok(n) => {
                        summary.boxscore_games_processed += 1;
                        summary.boxscore_player_rows += n;
                    }
                    Err(e) => warn!(game_id = gid, "rehydrate: upsert boxscore failed: {}", e),
                }
            }
            Err(e) => warn!(game_id = gid, "rehydrate: fetch boxscore failed: {}", e),
        }
    }

    info!(
        games = summary.games_upserted,
        skaters = summary.skater_rows,
        goalies = summary.goalie_rows,
        standings = summary.standings_rows,
        rosters = summary.rosters_upserted,
        rosters_skipped_fresh = summary.rosters_skipped_fresh,
        bracket = summary.bracket_captured,
        aggregates_skipped_fresh = summary.aggregates_skipped_fresh,
        boxscore_games = summary.boxscore_games_processed,
        player_rows = summary.boxscore_player_rows,
        landings = summary.landing_captures,
        errors = summary.errors.len(),
        "rehydrate: complete"
    );

    summary
}
