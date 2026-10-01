//! NHL → Postgres mirror steps shared by the meta poller (each on its own
//! cadence) and the admin rehydrate (all at once). Keeping one copy is
//! what stops the two pipelines drifting: a step added or fixed here
//! applies to both.
//!
//! Each step is freshness-gated by its caller-supplied TTL so a restart
//! or a back-to-back rehydrate doesn't refetch data written minutes ago.

use std::time::Duration;

use tracing::warn;

use crate::domain::models::nhl::GAME_TYPE_REGULAR;
use crate::error::Result;
use crate::infra::db::{cache_keys, nhl_mirror, FantasyDb};
use crate::infra::nhl::client::NhlClient;
use crate::tuning::live_mirror;

pub struct MirrorCtx<'a> {
    pub db: &'a FantasyDb,
    pub nhl: &'a NhlClient,
    pub season: u32,
    pub game_type: u8,
}

/// Result of a freshness-gated step.
#[derive(Debug)]
pub enum Step<T> {
    /// The mirror table was updated within the TTL; nothing fetched.
    Fresh,
    Ran(T),
}

fn gate(last: Option<chrono::DateTime<chrono::Utc>>, ttl: Duration) -> bool {
    nhl_mirror::is_stale(last, ttl)
}

#[derive(Debug, Default)]
pub struct ScheduleSync {
    pub upserted: usize,
    pub cancelled: u64,
    /// A game was added, cancelled, or changed state.
    pub changed: bool,
}

/// Mirror the schedule for `date`. When `invalidate_insights` is set and
/// a game was added, cancelled, or changed state, that date's insights
/// cache is dropped. Score-only changes deliberately don't invalidate:
/// the cached narrative is a day preview, and regenerating it on every
/// goal would mean an LLM call per goal per league.
pub async fn schedule_date(
    ctx: &MirrorCtx<'_>,
    date: &str,
    ttl: Duration,
    invalidate_insights: bool,
) -> Result<Step<ScheduleSync>> {
    let pool = ctx.db.pool();
    if !gate(
        nhl_mirror::last_update_nhl_games_for_date(pool, date).await?,
        ttl,
    ) {
        return Ok(Step::Fresh);
    }

    let schedule = ctx.nhl.get_schedule_by_date(date).await?;
    let games = schedule.games_for_date(date);
    let mut sync = ScheduleSync::default();
    for g in &games {
        match nhl_mirror::upsert_game(pool, g, date).await {
            Ok(changed) => {
                sync.upserted += 1;
                sync.changed |= changed;
            }
            Err(e) => warn!(date = %date, game_id = g.id, "mirror: upsert_game failed: {e}"),
        }
    }
    sync.cancelled = nhl_mirror::reconcile_schedule_for_date(
        pool,
        date,
        ctx.season as i32,
        ctx.game_type as i16,
        &games,
    )
    .await?;
    sync.changed |= sync.cancelled > 0;

    if invalidate_insights && sync.changed {
        let pattern = cache_keys::insights_for_date(ctx.season, ctx.game_type, date);
        ctx.db.cache().invalidate_by_like(&pattern).await?;
    }
    Ok(Step::Ran(sync))
}

/// Capture the write-once pre-game landing block for every FUT/PRE game
/// on `date` that doesn't have one yet. Most calls find nothing to do.
pub async fn landings_for_date(ctx: &MirrorCtx<'_>, date: &str) -> Result<usize> {
    let pool = ctx.db.pool();
    let mut captured = 0;
    for gid in nhl_mirror::list_games_without_landing_for_date(pool, date).await? {
        let landing = match ctx.nhl.get_game_landing_raw(gid as u32).await {
            Ok(l) => l,
            Err(e) => {
                warn!(game_id = gid, "mirror: landing fetch failed: {e}");
                continue;
            }
        };
        let matchup = landing.get("matchup").cloned().unwrap_or_default();
        if nhl_mirror::capture_game_landing(pool, gid, &matchup).await? {
            captured += 1;
        }
    }
    Ok(captured)
}

pub async fn skater_leaderboard(ctx: &MirrorCtx<'_>, ttl: Duration) -> Result<Step<usize>> {
    let pool = ctx.db.pool();
    let (season, gt) = (ctx.season as i32, ctx.game_type as i16);
    if !gate(
        nhl_mirror::last_update_nhl_skater_season_stats(pool, season, gt).await?,
        ttl,
    ) {
        return Ok(Step::Fresh);
    }
    let leaders = ctx.nhl.get_skater_stats(&ctx.season, ctx.game_type).await?;
    Ok(Step::Ran(
        nhl_mirror::upsert_skater_leaderboard(pool, season, gt, &leaders).await?,
    ))
}

/// Mirrors the configured game type and, in playoff mode, the regular
/// season too: the goalie rating bonus reads regular-season starters,
/// because playoff save percentage is part of what the model predicts.
pub async fn goalie_leaderboard(ctx: &MirrorCtx<'_>, ttl: Duration) -> Result<Step<usize>> {
    let pool = ctx.db.pool();
    let season = ctx.season as i32;
    let mut game_types = vec![ctx.game_type];
    if ctx.game_type != GAME_TYPE_REGULAR {
        game_types.push(GAME_TYPE_REGULAR);
    }
    let mut written = None;
    for gt in game_types {
        if !gate(
            nhl_mirror::last_update_nhl_goalie_season_stats(pool, season, gt as i16).await?,
            ttl,
        ) {
            continue;
        }
        let leaders = ctx.nhl.get_goalie_stats(&ctx.season, gt).await?;
        let n = nhl_mirror::upsert_goalie_leaderboard(pool, season, gt as i16, &leaders).await?;
        *written.get_or_insert(0) += n;
    }
    Ok(written.map_or(Step::Fresh, Step::Ran))
}

pub async fn standings(ctx: &MirrorCtx<'_>, ttl: Duration) -> Result<Step<usize>> {
    let pool = ctx.db.pool();
    let season = ctx.season as i32;
    if !gate(
        nhl_mirror::last_update_nhl_standings(pool, season).await?,
        ttl,
    ) {
        return Ok(Step::Fresh);
    }
    let payload = ctx.nhl.get_standings_raw().await?;
    Ok(Step::Ran(
        nhl_mirror::upsert_standings(pool, season, &payload).await?,
    ))
}

/// `Ran(false)` means the NHL hasn't published the carousel yet.
pub async fn playoff_bracket(ctx: &MirrorCtx<'_>, ttl: Duration) -> Result<Step<bool>> {
    let pool = ctx.db.pool();
    let season = ctx.season as i32;
    if !gate(
        nhl_mirror::last_update_nhl_playoff_bracket(pool, season).await?,
        ttl,
    ) {
        return Ok(Step::Fresh);
    }
    let Some(carousel) = ctx.nhl.get_playoff_carousel(ctx.season.to_string()).await? else {
        return Ok(Step::Ran(false));
    };
    nhl_mirror::upsert_playoff_bracket(pool, season, &carousel).await?;
    Ok(Step::Ran(true))
}

#[derive(Debug, Default)]
pub struct RosterSync {
    pub rosters: usize,
    pub club_stats_rows: usize,
    pub failures: Vec<String>,
}

/// All team rosters plus each team's full regular-season skater lines
/// (club stats). The two run together because the roster freshness gate
/// covers both: refreshing one without the other would leave the club
/// stats stale for a whole roster TTL. Club stats always use the regular
/// season because the projection model reads RS points-per-game from it.
/// Calls are paced by `ROSTER_FETCH_DELAY` to stay under NHL rate limits.
pub async fn rosters_and_club_stats(
    ctx: &MirrorCtx<'_>,
    ttl: Duration,
) -> Result<Step<RosterSync>> {
    let pool = ctx.db.pool();
    let season = ctx.season as i32;
    if !gate(
        nhl_mirror::last_update_nhl_team_rosters(pool, season).await?,
        ttl,
    ) {
        return Ok(Step::Fresh);
    }

    let teams = ctx.nhl.get_all_teams().await?;
    let mut sync = RosterSync::default();
    for (i, team) in teams.iter().enumerate() {
        if i > 0 {
            tokio::time::sleep(live_mirror::ROSTER_FETCH_DELAY).await;
        }
        let roster = match ctx.nhl.get_team_roster(team).await {
            Ok(players) => nhl_mirror::upsert_team_roster(pool, team, season, &players).await,
            Err(e) => Err(e),
        };
        match roster {
            Ok(()) => sync.rosters += 1,
            Err(e) => sync.failures.push(format!("roster {team}: {e}")),
        }

        tokio::time::sleep(live_mirror::ROSTER_FETCH_DELAY).await;
        let club = match ctx
            .nhl
            .get_club_stats(team, ctx.season, GAME_TYPE_REGULAR)
            .await
        {
            Ok(stats) => {
                nhl_mirror::upsert_team_club_stats(
                    pool,
                    season,
                    GAME_TYPE_REGULAR as i16,
                    team,
                    &stats.skaters,
                )
                .await
            }
            Err(e) => Err(e),
        };
        match club {
            Ok(n) => sync.club_stats_rows += n,
            Err(e) => sync.failures.push(format!("club stats {team}: {e}")),
        }
    }
    Ok(Step::Ran(sync))
}
