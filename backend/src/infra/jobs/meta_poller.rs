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

use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use chrono_tz::America::New_York;
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

    // Freshness thresholds. A fetch is skipped if the corresponding
    // mirror table was updated more recently than its threshold —
    // this prevents a server restart from re-running every source
    // on the first tick just because `counter` reset to 1.
    let today_ttl = live_mirror::META_POLL_INTERVAL;
    let agg_ttl = live_mirror::META_POLL_INTERVAL
        * (live_mirror::AGGREGATES_REFRESH_EVERY_N_META_TICKS);
    let roster_ttl = live_mirror::META_POLL_INTERVAL
        * (live_mirror::ROSTER_REFRESH_EVERY_N_META_TICKS);

    // ---- Today's schedule — every tick, unless the mirror was
    // touched in the last 5 minutes.
    //
    // "Today" is the *Eastern Time* date, not the UTC date. NHL's
    // `/schedule/{date}` keys games by ET local date — a 9 pm ET
    // game on April 18 is in the response for date "2026-04-18"
    // even when the wall clock at the server is already past UTC
    // midnight (April 19). Using `Utc::now().date_naive()` here
    // would skip every late-evening eastern slate during the
    // ~4-hour window between midnight UTC and midnight ET.
    let today: NaiveDate = Utc::now().with_timezone(&New_York).date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();
    let today_last = nhl_mirror::last_update_nhl_games_for_date(pool, &today_str)
        .await
        .unwrap_or(None);
    if nhl_mirror::is_stale(today_last, today_ttl) {
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
    } else {
        debug!(date = %today_str, "meta_poller: today's schedule fresh, skipping");
    }

    // ---- Landing capture for today's new FUT/PRE games ----
    //
    // `nhl_game_landing` is write-once: the pre-game matchup block
    // (leaders / goalies / venue / records) only appears in the NHL
    // landing response while the game is in FUT state, and we want to
    // keep it visible on the Insights sidebar for the entire hockey-
    // date. This loop catches exactly the newly-added FUT rows — the
    // `LEFT JOIN ... IS NULL` filter means each game is fetched at
    // most once per mirror lifecycle. Most ticks return an empty set
    // and cost nothing.
    match nhl_mirror::list_games_without_landing_for_date(pool, &today_str).await {
        Ok(ids) if !ids.is_empty() => {
            for gid in ids {
                match nhl.get_game_landing_raw(gid as u32).await {
                    Ok(landing) => {
                        let matchup = landing
                            .get("matchup")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null);
                        match nhl_mirror::capture_game_landing(pool, gid, &matchup).await {
                            Ok(true) => debug!(game_id = gid, "meta_poller: landing captured"),
                            Ok(false) => {
                                debug!(game_id = gid, "meta_poller: landing payload empty, skipped")
                            }
                            Err(e) => warn!(
                                game_id = gid,
                                "meta_poller: landing upsert failed: {}",
                                e
                            ),
                        }
                    }
                    Err(e) => warn!(
                        game_id = gid,
                        "meta_poller: landing fetch failed: {}",
                        e
                    ),
                }
            }
        }
        Ok(_) => debug!("meta_poller: all FUT/PRE games already have landing captured"),
        Err(e) => warn!("meta_poller: landing-pending query failed: {}", e),
    }

    if !work.refresh_aggregates {
        return Ok(());
    }

    // ---- Everything below runs on the aggregates cadence
    // (default: every 6th tick = 30 min). Each source is also
    // freshness-gated: on a server restart the counter=1 tick would
    // otherwise refetch every aggregate, even though the previous
    // process just wrote them a minute ago.

    // ---- Schedule: tomorrow ----
    let tomorrow = today + ChronoDuration::days(1);
    let tomorrow_str = tomorrow.format("%Y-%m-%d").to_string();
    let tomorrow_last = nhl_mirror::last_update_nhl_games_for_date(pool, &tomorrow_str)
        .await
        .unwrap_or(None);
    if nhl_mirror::is_stale(tomorrow_last, agg_ttl) {
        match nhl.get_schedule_by_date(&tomorrow_str).await {
            Ok(schedule) => {
                let games = schedule.games_for_date(&tomorrow_str);
                for g in &games {
                    if let Err(e) = nhl_mirror::upsert_game(pool, g, &tomorrow_str).await {
                        warn!(date = %tomorrow_str, game_id = g.id, "meta_poller: upsert_game failed: {}", e);
                    }
                }
                debug!(date = %tomorrow_str, count = games.len(), "meta_poller: schedule mirrored");
            }
            Err(e) => warn!(date = %tomorrow_str, "meta_poller: schedule fetch failed: {}", e),
        }
    } else {
        debug!(date = %tomorrow_str, "meta_poller: tomorrow's schedule fresh, skipping");
    }

    // ---- Skater leaderboard ----
    let skater_last =
        nhl_mirror::last_update_nhl_skater_season_stats(pool, season as i32, game_type as i16)
            .await
            .unwrap_or(None);
    if nhl_mirror::is_stale(skater_last, agg_ttl) {
        match nhl.get_skater_stats(&season, game_type).await {
            Ok(leaders) => {
                match nhl_mirror::upsert_skater_leaderboard(pool, season as i32, game_type as i16, &leaders).await {
                    Ok(n) => debug!(count = n, "meta_poller: skater leaderboard mirrored"),
                    Err(e) => warn!("meta_poller: skater upsert failed: {}", e),
                }
            }
            Err(e) => warn!("meta_poller: skater leaderboard fetch failed: {}", e),
        }
    } else {
        debug!("meta_poller: skater leaderboard fresh, skipping");
    }

    // ---- Goalie leaderboard ----
    let goalie_last =
        nhl_mirror::last_update_nhl_goalie_season_stats(pool, season as i32, game_type as i16)
            .await
            .unwrap_or(None);
    if nhl_mirror::is_stale(goalie_last, agg_ttl) {
        match nhl.get_goalie_stats(&season, game_type).await {
            Ok(payload) => {
                let json = serde_json::to_value(&payload).unwrap_or(serde_json::Value::Null);
                match nhl_mirror::upsert_goalie_leaderboard(pool, season as i32, game_type as i16, &json).await {
                    Ok(n) => debug!(count = n, "meta_poller: goalie leaderboard mirrored"),
                    Err(e) => warn!("meta_poller: goalie upsert failed: {}", e),
                }
            }
            Err(e) => warn!("meta_poller: goalie leaderboard fetch failed: {}", e),
        }
    } else {
        debug!("meta_poller: goalie leaderboard fresh, skipping");
    }

    // ---- Standings ----
    let standings_last = nhl_mirror::last_update_nhl_standings(pool, season as i32)
        .await
        .unwrap_or(None);
    if nhl_mirror::is_stale(standings_last, agg_ttl) {
        match nhl.get_standings_raw().await {
            Ok(payload) => match nhl_mirror::upsert_standings(pool, season as i32, &payload).await {
                Ok(n) => debug!(count = n, "meta_poller: standings mirrored"),
                Err(e) => warn!("meta_poller: standings upsert failed: {}", e),
            },
            Err(e) => warn!("meta_poller: standings fetch failed: {}", e),
        }
    } else {
        debug!("meta_poller: standings fresh, skipping");
    }

    // ---- Playoff carousel (playoffs only) ----
    if game_type == 3 {
        let bracket_last = nhl_mirror::last_update_nhl_playoff_bracket(pool, season as i32)
            .await
            .unwrap_or(None);
        if nhl_mirror::is_stale(bracket_last, agg_ttl) {
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
        } else {
            debug!("meta_poller: playoff carousel fresh, skipping");
        }
    }

    // ---- Rosters (every Nth tick, default 24 h). Same freshness
    // gate so a restart shortly after a previous roster refresh
    // doesn't re-run the whole 32-team pass.
    if work.refresh_rosters {
        let roster_last = nhl_mirror::last_update_nhl_team_rosters(pool, season as i32)
            .await
            .unwrap_or(None);
        if !nhl_mirror::is_stale(roster_last, roster_ttl) {
            debug!("meta_poller: rosters fresh, skipping");
            return Ok(());
        }
        match nhl.get_all_teams().await {
            Ok(teams) => {
                let mut roster_count = 0;
                let mut club_stats_count = 0;
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
                                roster_count += 1;
                            }
                        }
                        Err(e) => warn!(team = %team, "meta_poller: roster fetch failed: {}", e),
                    }

                    tokio::time::sleep(live_mirror::ROSTER_FETCH_DELAY).await;
                    // Full per-team skater season stats — the club-stats
                    // endpoint returns every skater who dressed, not just
                    // the top-25-per-category leaderboard. Always hit
                    // `game_type = 2` (regular season) because the
                    // projection model reads RS PPG from that row.
                    match nhl.get_club_stats(team, season, 2).await {
                        Ok(stats) => {
                            match nhl_mirror::upsert_team_club_stats(
                                pool,
                                season as i32,
                                2,
                                team,
                                &stats.skaters,
                            )
                            .await
                            {
                                Ok(n) => club_stats_count += n,
                                Err(e) => warn!(
                                    team = %team,
                                    "meta_poller: club-stats upsert failed: {}",
                                    e
                                ),
                            }
                        }
                        Err(e) => warn!(
                            team = %team,
                            "meta_poller: club-stats fetch failed: {}",
                            e
                        ),
                    }
                }
                info!(
                    rosters = roster_count,
                    skaters = club_stats_count,
                    "meta_poller: rosters + per-team season stats refreshed"
                );
            }
            Err(e) => warn!("meta_poller: team list fetch failed: {}", e),
        }
    }

    Ok(())
}
