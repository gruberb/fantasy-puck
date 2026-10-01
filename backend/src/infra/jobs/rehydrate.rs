//! Admin "rehydrate": run every mirror step (shared with the meta poller
//! via [`super::mirror_steps`]) synchronously, plus the boxscore backfill
//! needed right after a deploy.
//!
//! Reachable via `GET /api/admin/rehydrate`. Safe to call repeatedly
//! (all writes are idempotent) but heavy on a cold mirror:
//! schedule-across-range + aggregates + 32 rosters + boxscores for
//! every known game. The pacing and freshness rules below keep a
//! repeat invocation cheap.

use std::sync::Arc;

use serde::Serialize;
use tracing::{info, warn};

use crate::domain::models::nhl::GAME_TYPE_PLAYOFFS;
use crate::domain::time::DATE_FORMAT;
use crate::infra::db::{nhl_mirror, FantasyDb};
use crate::infra::jobs::mirror_steps::{self, MirrorCtx, Step};
use crate::infra::nhl::client::NhlClient;
use crate::tuning::live_mirror;

/// Summary shape returned to the admin caller. Counters are
/// best-effort; individual failures are recorded in `errors` and the run
/// keeps going rather than aborting.
#[derive(Debug, Default, Serialize)]
pub struct RehydrateSummary {
    pub games_upserted: usize,
    pub games_cancelled: u64,
    pub skater_rows: usize,
    pub goalie_rows: usize,
    pub standings_rows: usize,
    pub rosters_upserted: usize,
    pub club_stats_rows: usize,
    pub rosters_skipped_fresh: bool,
    pub bracket_captured: bool,
    pub aggregates_skipped_fresh: bool,
    pub boxscore_games_processed: usize,
    pub boxscore_player_rows: usize,
    pub landing_captures: usize,
    pub errors: Vec<String>,
}

impl RehydrateSummary {
    /// Records the error for a step and returns its output if it ran.
    fn record<T>(&mut self, label: &str, result: crate::error::Result<Step<T>>) -> Option<T> {
        match result {
            Ok(Step::Ran(out)) => Some(out),
            Ok(Step::Fresh) => None,
            Err(e) => {
                self.errors.push(format!("{label}: {e}"));
                None
            }
        }
    }
}

pub async fn run(db: &FantasyDb, nhl: Arc<NhlClient>) -> RehydrateSummary {
    let mut summary = RehydrateSummary::default();
    let ctx = MirrorCtx {
        db,
        nhl: &nhl,
        season: crate::api::season(),
        game_type: crate::api::game_type(),
    };
    let pool = db.pool();

    // Same cadences as the meta poller, so a rehydrate right after the
    // poller populated everything is a cheap no-op.
    let schedule_ttl = live_mirror::META_POLL_INTERVAL;
    let agg_ttl =
        live_mirror::META_POLL_INTERVAL * live_mirror::AGGREGATES_REFRESH_EVERY_N_META_TICKS;
    let roster_ttl =
        live_mirror::META_POLL_INTERVAL * live_mirror::ROSTER_REFRESH_EVERY_N_META_TICKS;

    // ---- Schedule and landings: playoff start (or today) through tomorrow.
    let today = crate::domain::time::hockey_today_date();
    let today_str = today.format(DATE_FORMAT).to_string();
    let start = chrono::NaiveDate::parse_from_str(crate::api::playoff_start(), DATE_FORMAT)
        .unwrap_or(today)
        .min(today);
    for date in start
        .iter_days()
        .take_while(|d| *d <= today + chrono::Duration::days(1))
    {
        let date = date.format(DATE_FORMAT).to_string();
        let result =
            mirror_steps::schedule_date(&ctx, &date, schedule_ttl, date == today_str).await;
        if let Some(sync) = summary.record(&format!("schedule {date}"), result) {
            summary.games_upserted += sync.upserted;
            summary.games_cancelled += sync.cancelled;
        }
        match mirror_steps::landings_for_date(&ctx, &date).await {
            Ok(n) => summary.landing_captures += n,
            Err(e) => summary.errors.push(format!("landings {date}: {e}")),
        }
    }

    // ---- Aggregates.
    let mut any_aggregate_ran = false;
    let result = mirror_steps::skater_leaderboard(&ctx, agg_ttl).await;
    if let Some(n) = summary.record("skater leaderboard", result) {
        summary.skater_rows = n;
        any_aggregate_ran = true;
    }
    let result = mirror_steps::goalie_leaderboard(&ctx, agg_ttl).await;
    if let Some(n) = summary.record("goalie leaderboard", result) {
        summary.goalie_rows = n;
        any_aggregate_ran = true;
    }
    let result = mirror_steps::standings(&ctx, agg_ttl).await;
    if let Some(n) = summary.record("standings", result) {
        summary.standings_rows = n;
        any_aggregate_ran = true;
    }
    if ctx.game_type == GAME_TYPE_PLAYOFFS {
        let result = mirror_steps::playoff_bracket(&ctx, agg_ttl).await;
        if let Some(captured) = summary.record("playoff bracket", result) {
            summary.bracket_captured = captured;
            any_aggregate_ran = true;
        }
    }
    summary.aggregates_skipped_fresh = !any_aggregate_ran && summary.errors.is_empty();

    // ---- Rosters and club stats.
    match mirror_steps::rosters_and_club_stats(&ctx, roster_ttl).await {
        Ok(Step::Fresh) => summary.rosters_skipped_fresh = true,
        Ok(Step::Ran(sync)) => {
            summary.rosters_upserted = sync.rosters;
            summary.club_stats_rows = sync.club_stats_rows;
            summary.errors.extend(sync.failures);
        }
        Err(e) => summary.errors.push(format!("rosters: {e}")),
    }

    // ---- Boxscores for every started game this season. Upserting the
    // boxscore also derives the score, which is what backfills games that
    // finalized before the live poller ever saw them.
    let games = match nhl_mirror::list_started_games(pool, ctx.season as i32).await {
        Ok(g) => g,
        Err(e) => {
            summary.errors.push(format!("list games: {e}"));
            Vec::new()
        }
    };
    info!(games = games.len(), "rehydrate: processing boxscores");

    for g in &games {
        let is_final = matches!(g.game_state.as_str(), "FINAL" | "OFF");
        let boxscore = if is_final {
            nhl.get_game_boxscore_fresh(g.game_id as u32).await
        } else {
            nhl.get_game_boxscore(g.game_id as u32).await
        };
        let box_score = match boxscore {
            Ok(b) => b,
            Err(e) => {
                warn!(game_id = g.game_id, "rehydrate: fetch boxscore failed: {e}");
                continue;
            }
        };
        match nhl_mirror::upsert_boxscore_players(
            pool,
            g.game_id,
            &g.home_team,
            &g.away_team,
            &box_score,
        )
        .await
        {
            Ok(n) => {
                summary.boxscore_games_processed += 1;
                summary.boxscore_player_rows += n;
            }
            Err(e) => {
                warn!(
                    game_id = g.game_id,
                    "rehydrate: upsert boxscore failed: {e}"
                );
                continue;
            }
        }
        // Rehydrate is the explicit "I want the canonical box" path, so a
        // successful upsert of a final game seals it for aggregated reads.
        if is_final {
            if let Err(e) = nhl_mirror::mark_game_stats_finalized(pool, g.game_id).await {
                warn!(
                    game_id = g.game_id,
                    "rehydrate: mark_game_stats_finalized failed: {e}"
                );
            }
        }
    }

    info!(
        games = summary.games_upserted,
        cancelled = summary.games_cancelled,
        skaters = summary.skater_rows,
        goalies = summary.goalie_rows,
        standings = summary.standings_rows,
        rosters = summary.rosters_upserted,
        club_stats = summary.club_stats_rows,
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
