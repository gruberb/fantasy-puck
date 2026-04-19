//! Metadata poller.
//!
//! Every [`crate::tuning::live_mirror::META_POLL_INTERVAL`] (5 min in
//! production), fetches slow-moving NHL data and mirrors it into the
//! Postgres tables:
//!
//! - Today + tomorrow's schedule → `nhl_games`
//! - Skater season leaderboard → `nhl_skater_season_stats`
//! - Goalie season leaderboard → `nhl_goalie_season_stats`
//! - League standings → `nhl_standings`
//! - Playoff carousel (playoffs only) → `nhl_playoff_bracket`
//! - Every 6th tick (≈30 min): team rosters → `nhl_team_rosters`
//!
//! Leader election is via a Postgres advisory lock; on a multi-replica
//! deployment only one replica runs the work each tick. A non-leader
//! returns immediately and waits for the next tick.
//!
//! The poller swallows per-step errors (logging at `warn`) so a
//! transient NHL outage does not poison subsequent ticks.

use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use tokio::time::{interval_at, Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::api::{game_type as cfg_game_type, season as cfg_season};
use crate::infra::db::{nhl_mirror, FantasyDb};
use crate::infra::nhl::client::NhlClient;
use crate::tuning::live_mirror;

/// Tick counter lives inside `run` so tests can construct a fresh
/// poller. The 6-tick roster cadence is not a wall-clock cron; it
/// ticks off a tick counter started at process boot.
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
    /// Tomorrow's schedule + standings + skater/goalie leaderboards
    /// + playoff carousel. All change only on game-end events, so
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

async fn tick_body(
    db: &FantasyDb,
    nhl: &Arc<NhlClient>,
    work: TickWork,
) -> anyhow::Result<()> {
    let season = cfg_season();
    let game_type = cfg_game_type();
    let pool = db.pool();

    // ---- Today's schedule — every tick.
    // This is the only data that genuinely benefits from 5-min
    // polling: `game_state` transitions FUT → PRE → LIVE → OFF on
    // a short timescale and the live-poller only picks up the
    // switch once a game is already LIVE/CRIT/PRE.
    let today = Utc::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();
    match nhl.get_schedule_by_date(&today_str).await {
        Ok(schedule) => {
            let games = schedule.games_for_date(&today_str);
            for g in &games {
                if let Err(e) = nhl_mirror::upsert_game(pool, g, &today_str).await {
                    warn!(date = %today_str, game_id = g.id, "meta_poller: upsert_game failed: {}", e);
                }
            }
            debug!(date = %today_str, count = games.len(), "meta_poller: today's schedule mirrored");
        }
        Err(e) => warn!(date = %today_str, "meta_poller: today's schedule fetch failed: {}", e),
    }

    if !work.refresh_aggregates {
        return Ok(());
    }

    // ---- Everything below runs on the aggregates cadence
    // (default: every 6th tick = 30 min). Standings / leaderboards /
    // carousel only change when a game ends, so 5-min polling of
    // them was mostly wasted NHL calls. Tomorrow's schedule sits
    // here too — postponements are rare.

    // ---- Schedule: tomorrow ----
    for offset in [1i64] {
        let date = (today + ChronoDuration::days(offset))
            .format("%Y-%m-%d")
            .to_string();
        match nhl.get_schedule_by_date(&date).await {
            Ok(schedule) => {
                let games = schedule.games_for_date(&date);
                for g in &games {
                    if let Err(e) = nhl_mirror::upsert_game(pool, g, &date).await {
                        warn!(date = %date, game_id = g.id, "meta_poller: upsert_game failed: {}", e);
                    }
                }
                debug!(date = %date, count = games.len(), "meta_poller: schedule mirrored");
            }
            Err(e) => warn!(date = %date, "meta_poller: schedule fetch failed: {}", e),
        }
    }

    // ---- Skater leaderboard ----
    match nhl.get_skater_stats(&season, game_type).await {
        Ok(leaders) => {
            match nhl_mirror::upsert_skater_leaderboard(pool, season as i32, game_type as i16, &leaders).await {
                Ok(n) => debug!(count = n, "meta_poller: skater leaderboard mirrored"),
                Err(e) => warn!("meta_poller: skater upsert failed: {}", e),
            }
        }
        Err(e) => warn!("meta_poller: skater leaderboard fetch failed: {}", e),
    }

    // ---- Goalie leaderboard: use raw payload so we don't need a
    //      typed struct for every leaderboard category. ----
    match nhl
        .get_goalie_stats(&season, game_type)
        .await
    {
        Ok(payload) => {
            let json = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);
            match nhl_mirror::upsert_goalie_leaderboard(pool, season as i32, game_type as i16, &json).await {
                Ok(n) => debug!(count = n, "meta_poller: goalie leaderboard mirrored"),
                Err(e) => warn!("meta_poller: goalie upsert failed: {}", e),
            }
        }
        Err(e) => warn!("meta_poller: goalie leaderboard fetch failed: {}", e),
    }

    // ---- Standings ----
    match nhl.get_standings_raw().await {
        Ok(payload) => match nhl_mirror::upsert_standings(pool, season as i32, &payload).await {
            Ok(n) => debug!(count = n, "meta_poller: standings mirrored"),
            Err(e) => warn!("meta_poller: standings upsert failed: {}", e),
        },
        Err(e) => warn!("meta_poller: standings fetch failed: {}", e),
    }

    // ---- Playoff carousel (playoffs only) ----
    if game_type == 3 {
        match nhl.get_playoff_carousel(season.to_string()).await {
            Ok(Some(carousel)) => {
                let json = serde_json::to_value(&carousel).unwrap_or(serde_json::Value::Null);
                if let Err(e) = nhl_mirror::upsert_playoff_bracket(pool, season as i32, &json).await {
                    warn!("meta_poller: bracket upsert failed: {}", e);
                } else {
                    debug!("meta_poller: playoff carousel mirrored");
                }
            }
            Ok(None) => debug!("meta_poller: playoff carousel not published yet"),
            Err(e) => warn!("meta_poller: playoff carousel fetch failed: {}", e),
        }
    }

    // ---- Rosters (every Nth tick, default 24 h) ----
    //
    // Paced: 32 back-to-back roster fetches at full speed trip NHL's
    // per-IP rate limit (~20 req/s observed). A 250 ms sleep between
    // calls keeps us well under the threshold and the whole pass
    // still completes in ~14 s per run.
    if work.refresh_rosters {
        match nhl.get_all_teams().await {
            Ok(teams) => {
                let mut count = 0;
                for (i, team) in teams.iter().enumerate() {
                    if i > 0 {
                        tokio::time::sleep(live_mirror::ROSTER_FETCH_DELAY).await;
                    }
                    match nhl.get_team_roster(team).await {
                        Ok(players) => {
                            if let Err(e) =
                                nhl_mirror::upsert_team_roster(pool, team, season as i32, &players)
                                    .await
                            {
                                warn!(team = %team, "meta_poller: roster upsert failed: {}", e);
                            } else {
                                count += 1;
                            }
                        }
                        Err(e) => warn!(team = %team, "meta_poller: roster fetch failed: {}", e),
                    }
                }
                info!(count, "meta_poller: team rosters refreshed");
            }
            Err(e) => warn!("meta_poller: team list fetch failed: {}", e),
        }
    }

    Ok(())
}
