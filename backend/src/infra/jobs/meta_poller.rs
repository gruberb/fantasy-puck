//! Metadata poller.
//!
//! Every [`crate::tuning::live_mirror::META_POLL_INTERVAL`] (5 min in
//! production), fetches slow-moving NHL data and mirrors it into the
//! Postgres tables:
//!
//! - Recent, today, and tomorrow schedules → `nhl_games`
//! - Skater season leaderboard → `nhl_skater_season_stats`
//! - Goalie season leaderboard → `nhl_goalie_season_stats`
//! - League standings → `nhl_standings`
//! - Playoff carousel (playoffs only) → `nhl_playoff_bracket`
//! - Every `ROSTER_REFRESH_EVERY_N_META_TICKS` ticks (≈24 h): team rosters
//!   and per-team club stats → `nhl_team_rosters`, `nhl_skater_season_stats`
//!
//! The steps themselves live in [`super::mirror_steps`], shared with the
//! admin rehydrate.
//!
//! Leader election is via a Postgres advisory lock; on a multi-replica
//! deployment only one replica runs the work each tick. A non-leader
//! returns immediately and waits for the next tick.
//!
//! The poller swallows per-step errors (logging at `warn`) so a
//! transient NHL outage does not poison subsequent ticks.

use std::sync::Arc;

use chrono::{Duration as ChronoDuration, NaiveDate};
use tokio::time::{interval_at, Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::api::{game_type as cfg_game_type, season as cfg_season};
use crate::domain::models::nhl::GAME_TYPE_PLAYOFFS;
use crate::domain::time::DATE_FORMAT;
use crate::infra::db::{nhl_mirror, FantasyDb};
use crate::infra::jobs::mirror_steps::{self, MirrorCtx, Step};
use crate::infra::nhl::client::NhlClient;
use crate::tuning::live_mirror;

/// The aggregate and roster cadences tick off a counter started at
/// process boot, not a wall-clock cron.
pub async fn run(db: FantasyDb, nhl: Arc<NhlClient>, cancel: CancellationToken) {
    let start = Instant::now() + live_mirror::META_POLL_STARTUP_DELAY;
    let mut tick = interval_at(start, live_mirror::META_POLL_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut counter: u32 = 0;
    info!(
        interval_secs = live_mirror::META_POLL_INTERVAL.as_secs(),
        startup_delay_secs = live_mirror::META_POLL_STARTUP_DELAY.as_secs(),
        aggregates_every = live_mirror::AGGREGATES_REFRESH_EVERY_N_META_TICKS,
        roster_every = live_mirror::ROSTER_REFRESH_EVERY_N_META_TICKS,
        "meta_poller: started"
    );
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("meta_poller: shutdown");
                return;
            }
            _ = tick.tick() => {
                counter = counter.wrapping_add(1);
                let work = TickWork {
                    refresh_aggregates:
                        counter % live_mirror::AGGREGATES_REFRESH_EVERY_N_META_TICKS == 1,
                    refresh_rosters:
                        counter % live_mirror::ROSTER_REFRESH_EVERY_N_META_TICKS == 1,
                };
                run_one_tick(&db, &nhl, work).await;
            }
        }
    }
}

/// Which subsets of the meta-poller's work to run on this tick.
/// Every tick refreshes today's schedule (state transitions are the
/// only thing that benefits from tight polling). Aggregates and
/// rosters run on coarser cadences defined in
/// [`crate::tuning::live_mirror`].
#[derive(Debug, Clone, Copy)]
struct TickWork {
    /// Tomorrow's schedule, standings, skater/goalie leaderboards, and
    /// playoff carousel. All change only on game-end events, so
    /// 30-min is plenty.
    refresh_aggregates: bool,
    /// All 32 team rosters. Essentially static during playoffs;
    /// the 10:00 UTC daily prewarm also covers this, so
    /// 24-hour here is belt-and-braces.
    refresh_rosters: bool,
}

async fn run_one_tick(db: &FantasyDb, nhl: &Arc<NhlClient>, work: TickWork) {
    let pool = db.pool();
    // Hold a dedicated connection for the lock's lifetime so acquire
    // and release run on the same Postgres session — see the doc
    // on `nhl_mirror::try_meta_lock`.
    let mut lock_conn = match pool.acquire().await {
        Ok(c) => c,
        Err(e) => {
            warn!("meta_poller: failed to acquire lock connection: {}", e);
            return;
        }
    };
    match nhl_mirror::try_meta_lock(&mut lock_conn).await {
        Ok(true) => {}
        Ok(false) => {
            debug!("meta_poller: another replica holds the lock, skipping tick");
            return;
        }
        Err(e) => {
            warn!("meta_poller: failed to acquire lock: {}", e);
            return;
        }
    }
    let result = tick_body(db, nhl, work).await;
    if let Err(e) = nhl_mirror::release_meta_lock(&mut lock_conn).await {
        warn!("meta_poller: failed to release lock: {}", e);
    }
    if let Err(e) = result {
        warn!("meta_poller: tick failed: {}", e);
    }
}

async fn tick_body(db: &FantasyDb, nhl: &Arc<NhlClient>, work: TickWork) -> anyhow::Result<()> {
    let ctx = MirrorCtx {
        db,
        nhl,
        season: cfg_season(),
        game_type: cfg_game_type(),
    };

    // Freshness thresholds: a source is skipped if its mirror table was
    // updated more recently than this, so a restart's counter=1 tick
    // doesn't refetch everything the previous process just wrote.
    let today_ttl = live_mirror::META_POLL_INTERVAL;
    let agg_ttl =
        live_mirror::META_POLL_INTERVAL * live_mirror::AGGREGATES_REFRESH_EVERY_N_META_TICKS;
    let roster_ttl =
        live_mirror::META_POLL_INTERVAL * live_mirror::ROSTER_REFRESH_EVERY_N_META_TICKS;

    let today: NaiveDate = crate::domain::time::hockey_today_date();
    let today_str = today.format(DATE_FORMAT).to_string();
    if crate::api::past_season_end(&today_str) {
        debug!(date = %today_str, "meta_poller: past season end, skipping tick");
        return Ok(());
    }

    // The previous two ET dates plus today, every tick: late games and
    // post-buzzer corrections land on yesterday's slate. Only today's
    // insights depend on the schedule, so only today invalidates them.
    for days_back in [2, 1, 0] {
        let date = (today - ChronoDuration::days(days_back))
            .format(DATE_FORMAT)
            .to_string();
        let is_today = days_back == 0;
        log_step(
            "schedule",
            &date,
            mirror_steps::schedule_date(&ctx, &date, today_ttl, is_today).await,
        );
    }

    match mirror_steps::landings_for_date(&ctx, &today_str).await {
        Ok(0) => {}
        Ok(n) => debug!(captured = n, "meta_poller: landings captured"),
        Err(e) => warn!("meta_poller: landing capture failed: {e}"),
    }

    if !work.refresh_aggregates {
        return Ok(());
    }

    let tomorrow = (today + ChronoDuration::days(1))
        .format(DATE_FORMAT)
        .to_string();
    log_step(
        "schedule",
        &tomorrow,
        mirror_steps::schedule_date(&ctx, &tomorrow, agg_ttl, false).await,
    );
    log_step(
        "skater leaderboard",
        "",
        mirror_steps::skater_leaderboard(&ctx, agg_ttl).await,
    );
    log_step(
        "goalie leaderboard",
        "",
        mirror_steps::goalie_leaderboard(&ctx, agg_ttl).await,
    );
    log_step(
        "standings",
        "",
        mirror_steps::standings(&ctx, agg_ttl).await,
    );
    if ctx.game_type == GAME_TYPE_PLAYOFFS {
        log_step(
            "playoff bracket",
            "",
            mirror_steps::playoff_bracket(&ctx, agg_ttl).await,
        );
    }

    if work.refresh_rosters {
        match mirror_steps::rosters_and_club_stats(&ctx, roster_ttl).await {
            Ok(Step::Fresh) => debug!("meta_poller: rosters fresh, skipping"),
            Ok(Step::Ran(sync)) => {
                for f in &sync.failures {
                    warn!("meta_poller: {f}");
                }
                info!(
                    rosters = sync.rosters,
                    skaters = sync.club_stats_rows,
                    "meta_poller: rosters + per-team season stats refreshed"
                );
            }
            Err(e) => warn!("meta_poller: rosters failed: {e}"),
        }
    }

    Ok(())
}

fn log_step<T: std::fmt::Debug>(label: &str, scope: &str, result: crate::error::Result<Step<T>>) {
    match result {
        Ok(Step::Fresh) => debug!(scope, "meta_poller: {label} fresh, skipping"),
        Ok(Step::Ran(out)) => debug!(scope, ?out, "meta_poller: {label} mirrored"),
        Err(e) => warn!(scope, "meta_poller: {label} failed: {e}"),
    }
}
