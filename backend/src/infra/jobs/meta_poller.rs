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
use tokio::time::{interval, MissedTickBehavior};
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
    let mut tick = interval(live_mirror::META_POLL_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut counter: u32 = 0;
    info!(
        interval_secs = live_mirror::META_POLL_INTERVAL.as_secs(),
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
                let refresh_rosters =
                    counter % live_mirror::ROSTER_REFRESH_EVERY_N_META_TICKS == 1;
                run_one_tick(&db, &nhl, refresh_rosters).await;
            }
        }
    }
}

async fn run_one_tick(db: &FantasyDb, nhl: &Arc<NhlClient>, refresh_rosters: bool) {
    let pool = db.pool();
    match nhl_mirror::try_meta_lock(pool).await {
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
    let result = tick_body(db, nhl, refresh_rosters).await;
    if let Err(e) = nhl_mirror::release_meta_lock(pool).await {
        warn!("meta_poller: failed to release lock: {}", e);
    }
    if let Err(e) = result {
        warn!("meta_poller: tick failed: {}", e);
    }
}

async fn tick_body(
    db: &FantasyDb,
    nhl: &Arc<NhlClient>,
    refresh_rosters: bool,
) -> anyhow::Result<()> {
    let season = cfg_season();
    let game_type = cfg_game_type();
    let pool = db.pool();

    // ---- Schedule: today + tomorrow ----
    let today = Utc::now().date_naive();
    for offset in [0i64, 1] {
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

    // ---- Rosters (every Nth tick) ----
    if refresh_rosters {
        match nhl.get_all_teams().await {
            Ok(teams) => {
                let mut count = 0;
                for team in &teams {
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
