//! Admin "rehydrate" — run every poller step synchronously plus any
//! one-shot backfills needed right after a deploy.
//!
//! Reachable via `GET /api/admin/rehydrate`. Safe to call repeatedly
//! (all writes are idempotent) but heavy: a single invocation fans
//! out dozens of NHL calls.

use std::sync::Arc;

use serde::Serialize;
use tracing::{info, warn};

use crate::infra::db::{nhl_mirror, FantasyDb};
use crate::infra::nhl::client::NhlClient;

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
    pub bracket_captured: bool,
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

    // ---- Schedule: playoff start → today ----
    let today = chrono::Utc::now().date_naive();
    let mut dates: Vec<String> = Vec::new();
    // Start from the configured playoff_start if we're past it; else
    // just today + tomorrow.
    let playoff_start = crate::api::playoff_start();
    let start_naive = chrono::NaiveDate::parse_from_str(playoff_start, "%Y-%m-%d")
        .unwrap_or(today);
    let mut cursor = if start_naive <= today { start_naive } else { today };
    while cursor <= today + chrono::Duration::days(1) {
        dates.push(cursor.format("%Y-%m-%d").to_string());
        cursor += chrono::Duration::days(1);
    }
    for date in &dates {
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

    // ---- Skater leaderboard ----
    match nhl.get_skater_stats(&season, game_type).await {
        Ok(leaders) => {
            match nhl_mirror::upsert_skater_leaderboard(
                pool,
                season as i32,
                game_type as i16,
                &leaders,
            )
            .await
            {
                Ok(n) => summary.skater_rows = n,
                Err(e) => summary.errors.push(format!("skater upsert: {}", e)),
            }
        }
        Err(e) => summary.errors.push(format!("skater leaderboard: {}", e)),
    }

    // ---- Goalie leaderboard ----
    if let Ok(payload) = nhl.get_goalie_stats(&season, game_type).await {
        let json = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);
        match nhl_mirror::upsert_goalie_leaderboard(pool, season as i32, game_type as i16, &json)
            .await
        {
            Ok(n) => summary.goalie_rows = n,
            Err(e) => summary.errors.push(format!("goalie upsert: {}", e)),
        }
    }

    // ---- Standings ----
    if let Ok(payload) = nhl.get_standings_raw().await {
        match nhl_mirror::upsert_standings(pool, season as i32, &payload).await {
            Ok(n) => summary.standings_rows = n,
            Err(e) => summary.errors.push(format!("standings upsert: {}", e)),
        }
    }

    // ---- Playoff bracket ----
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

    // ---- Team rosters ----
    if let Ok(teams) = nhl.get_all_teams().await {
        for team in &teams {
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

    // ---- Boxscores + landing for every game we know about ----
    // Read the games we just upserted; for each, fetch the full
    // boxscore and, if FUT, snapshot the pre-game matchup.
    let game_rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT game_id, home_team, away_team, game_state FROM nhl_games",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    info!(games = game_rows.len(), "rehydrate: processing boxscores");

    for (gid, home, away, state) in &game_rows {
        // Pre-game matchup (write-once)
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

        // Boxscore. Skip FUT (no stats yet). CRIT/LIVE/OFF/FINAL all
        // carry the per-player block.
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
                    Err(e) => {
                        warn!(game_id = gid, "rehydrate: upsert boxscore failed: {}", e)
                    }
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
        bracket = summary.bracket_captured,
        boxscore_games = summary.boxscore_games_processed,
        player_rows = summary.boxscore_player_rows,
        landings = summary.landing_captures,
        errors = summary.errors.len(),
        "rehydrate: complete"
    );

    summary
}
