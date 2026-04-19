//! Nightly NHL Edge refresher.
//!
//! Reads the top N season-leader skaters (per [`tuning::live_mirror::
//! EDGE_REFRESH_TOP_N`]) from `nhl_skater_season_stats`, fetches their
//! NHL Edge telemetry sequentially with [`tuning::live_mirror::
//! EDGE_REFRESH_DELAY`] pacing, and upserts `nhl_skater_edge` rows.
//!
//! The Insights handler previously issued 5 concurrent NHL calls per
//! cache miss to populate the Hot card's top-skating-speed and top-
//! shot-speed tiles. Those five bonus calls contributed to the
//! playoff-evening 429 cascade on every cold render; moving the work
//! to a nightly job means the handler reads from Postgres and the
//! tiles survive any NHL outage during the day.
//!
//! The refresher is scheduled at 09:30 UTC — 30 minutes ahead of the
//! daily prewarm so the insights pre-warm reads fresh Edge data. A
//! freshness gate ([`tuning::live_mirror::EDGE_REFRESH_FRESHNESS`])
//! skips the run when a recent refresh already happened, so an admin
//! prewarm fired close to the cron time doesn't double up.

use std::sync::Arc;

use chrono::Utc;
use tracing::{debug, info, warn};

use crate::api::{game_type as cfg_game_type, season as cfg_season};
use crate::infra::db::{nhl_mirror, FantasyDb};
use crate::infra::nhl::client::NhlClient;
use crate::tuning::live_mirror;

/// Run one pass of the refresher. Force=true overrides the freshness
/// gate — used by the admin prewarm to guarantee fresh Edge data after
/// a manual trigger.
pub async fn run(db: &FantasyDb, nhl: Arc<NhlClient>, force: bool) -> RefreshSummary {
    let mut summary = RefreshSummary::default();
    let pool = db.pool();

    if !force {
        match nhl_mirror::last_update_nhl_skater_edge(pool).await {
            Ok(Some(ts)) => {
                let age = Utc::now().signed_duration_since(ts);
                if age.num_seconds() >= 0
                    && (age.num_seconds() as u64) < live_mirror::EDGE_REFRESH_FRESHNESS.as_secs()
                {
                    info!(
                        age_hours = age.num_hours(),
                        "edge_refresher: mirror still fresh, skipping"
                    );
                    summary.skipped_fresh = true;
                    return summary;
                }
            }
            Ok(None) => debug!("edge_refresher: no prior refresh, running"),
            Err(e) => warn!("edge_refresher: freshness lookup failed (running anyway): {}", e),
        }
    }

    // Take the top N season leaders for the current game_type.
    let leaders = match nhl_mirror::list_skater_season_stats(
        pool,
        cfg_season() as i32,
        cfg_game_type() as i16,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            warn!("edge_refresher: failed to list leaders: {}", e);
            return summary;
        }
    };
    let ids: Vec<i64> = leaders
        .into_iter()
        .take(live_mirror::EDGE_REFRESH_TOP_N)
        .map(|r| r.player_id)
        .collect();
    if ids.is_empty() {
        info!("edge_refresher: no leaders to refresh");
        return summary;
    }

    info!(
        count = ids.len(),
        pace_ms = live_mirror::EDGE_REFRESH_DELAY.as_millis() as u64,
        "edge_refresher: starting sequential refresh"
    );
    for (i, pid) in ids.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(live_mirror::EDGE_REFRESH_DELAY).await;
        }
        match nhl.get_skater_edge_detail(*pid).await {
            Ok(json) => {
                let top_speed = json
                    .get("topSkatingSpeed")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);
                let top_shot = json
                    .get("topShotSpeed")
                    .and_then(|v| v.as_f64())
                    .map(|v| v as f32);
                if let Err(e) =
                    nhl_mirror::upsert_skater_edge(pool, *pid, top_speed, top_shot).await
                {
                    warn!(player_id = pid, "edge_refresher: upsert failed: {}", e);
                    summary.errors += 1;
                } else {
                    summary.refreshed += 1;
                }
            }
            Err(e) => {
                warn!(player_id = pid, "edge_refresher: fetch failed: {}", e);
                summary.errors += 1;
            }
        }
    }

    info!(
        refreshed = summary.refreshed,
        errors = summary.errors,
        "edge_refresher: complete"
    );
    summary
}

#[derive(Debug, Default)]
pub struct RefreshSummary {
    pub refreshed: usize,
    pub errors: usize,
    pub skipped_fresh: bool,
}
