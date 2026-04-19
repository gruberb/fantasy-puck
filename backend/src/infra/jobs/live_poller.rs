//! Live-game poller.
//!
//! Every [`crate::tuning::live_mirror::LIVE_POLL_INTERVAL`] (60 s in
//! production), reads the set of games scheduled for today whose
//! state in `nhl_games` is LIVE/CRIT/PRE, and for each one fetches
//! the latest boxscore + score/period info from the NHL API and
//! updates:
//!
//! - `nhl_games.home_score`, `away_score`, `game_state`, `period_*`
//! - `nhl_player_game_stats` for every skater + goalie in the boxscore
//!
//! When no games are live the poller does one very cheap read
//! (`SELECT game_id … WHERE state IN …`) and returns, so off-night
//! cost is ~1 SQL query per minute per replica (leader only).
//!
//! Leader election is via a Postgres advisory lock (
//! [`nhl_mirror::try_live_lock`]).

use std::sync::Arc;

use chrono::Utc;
use tokio::time::{interval_at, Instant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::infra::db::{nhl_mirror, FantasyDb};
use crate::infra::nhl::client::NhlClient;
use crate::tuning::live_mirror;

pub async fn run(db: FantasyDb, nhl: Arc<NhlClient>, cancel: CancellationToken) {
    let start = Instant::now() + live_mirror::LIVE_POLL_STARTUP_DELAY;
    let mut tick = interval_at(start, live_mirror::LIVE_POLL_INTERVAL);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);

    info!(
        interval_secs = live_mirror::LIVE_POLL_INTERVAL.as_secs(),
        startup_delay_secs = live_mirror::LIVE_POLL_STARTUP_DELAY.as_secs(),
        "live_poller: started"
    );
    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("live_poller: shutdown");
                return;
            }
            _ = tick.tick() => {
                run_one_tick(&db, &nhl).await;
            }
        }
    }
}

async fn run_one_tick(db: &FantasyDb, nhl: &Arc<NhlClient>) {
    let pool = db.pool();
    // Dedicated session for lock lifecycle; see
    // `nhl_mirror::try_live_lock`.
    let mut lock_conn = match pool.acquire().await {
        Ok(c) => c,
        Err(e) => {
            warn!("live_poller: failed to acquire lock connection: {}", e);
            return;
        }
    };
    match nhl_mirror::try_live_lock(&mut lock_conn).await {
        Ok(true) => {}
        Ok(false) => {
            debug!("live_poller: another replica holds the lock, skipping tick");
            return;
        }
        Err(e) => {
            warn!("live_poller: failed to acquire lock: {}", e);
            return;
        }
    }
    let result = tick_body(db, nhl).await;
    if let Err(e) = nhl_mirror::release_live_lock(&mut lock_conn).await {
        warn!("live_poller: failed to release lock: {}", e);
    }
    if let Err(e) = result {
        warn!("live_poller: tick failed: {}", e);
    }
}

async fn tick_body(db: &FantasyDb, nhl: &Arc<NhlClient>) -> anyhow::Result<()> {
    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let pool = db.pool();

    let game_ids = nhl_mirror::list_live_game_ids_for_date(pool, &today).await?;
    if game_ids.is_empty() {
        debug!("live_poller: no live games");
        return Ok(());
    }

    for game_id in game_ids {
        if let Err(e) = poll_one_game(db, nhl, game_id).await {
            warn!(game_id, "live_poller: game tick failed: {}", e);
        }
    }
    Ok(())
}

async fn poll_one_game(
    db: &FantasyDb,
    nhl: &Arc<NhlClient>,
    game_id: i64,
) -> anyhow::Result<()> {
    let pool = db.pool();

    // Boxscore drives player stats and (as a side benefit) carries the
    // current gameState. The existing typed response is just the
    // per-player block, so we also read game_data for state/score
    // after.
    let box_score = nhl.get_game_boxscore(game_id as u32).await?;

    // Home/away abbrevs live on the game row we already have. Single
    // SELECT.
    let (home, away): (String, String) = sqlx::query_as(
        "SELECT home_team, away_team FROM nhl_games WHERE game_id = $1",
    )
    .bind(game_id)
    .fetch_one(pool)
    .await?;

    let written =
        nhl_mirror::upsert_boxscore_players(pool, game_id, &home, &away, &box_score).await?;
    debug!(game_id, players = written, "live_poller: boxscore upserted");

    // Live game_state + score + period info. `get_game_data` returns
    // `Option<GameData>` because the endpoint occasionally 404s for
    // games that have not registered in game-center yet; skip the
    // update in that case rather than clobber with defaults.
    if let Ok(Some(data)) = nhl.get_game_data(game_id as u32).await {
        let state_str = serde_json::to_value(&data.game_state)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "LIVE".into());
        let period_number: Option<i16> = data
            .period
            .as_ref()
            .and_then(|p| p.parse::<i16>().ok());
        nhl_mirror::update_game_live_state(
            pool,
            game_id,
            &state_str,
            data.home_score,
            data.away_score,
            period_number,
            data.period.as_deref(),
        )
        .await?;
    }

    Ok(())
}
