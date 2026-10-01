//! Typed repository for the eight NHL-mirror tables defined in
//! `supabase/migrations/20260420000000_nhl_mirror.sql`.
//!
//! All callers (meta poller, live poller, admin rehydrate, and the
//! post-redesign read-side handlers) go through this module. No NHL
//! API calls live here — the repository takes already-fetched domain
//! types and writes them, or reads straight from the tables.
//!
//! # Write-once vs. upsert
//!
//! - `upsert_game`, `upsert_player_game_stat`, `upsert_skater_leader`,
//!   `upsert_goalie_leader`, `upsert_standings_row`, `upsert_team_roster`,
//!   `upsert_playoff_bracket` — all idempotent; last writer wins.
//! - `capture_game_landing` is **write-once**: once the pre-game
//!   matchup block has been captured for a `game_id`, it is never
//!   overwritten, so the "game went LIVE and the landing block is
//!   now empty" case cannot clobber a good pre-game payload.
//!
//! # Advisory locks
//!
//! [`try_meta_lock`] and [`try_live_lock`] wrap `pg_try_advisory_lock`
//! so only one replica of the backend polls at a time. If the lock is
//! not acquired the caller should skip the tick.

use anyhow::Context;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};

use crate::domain::models::nhl::{
    default_name, BoxscorePlayer, GameBoxscore, GoalieStatsLeaders, Player, PlayoffCarousel,
    StatsLeaders, TodayGame,
};
use crate::error::{Error, Result};

// ---------------------------------------------------------------------
// Advisory lock keys
// ---------------------------------------------------------------------

/// Postgres advisory-lock key for the metadata poller. Held for the
/// duration of a single tick so two replicas of the backend cannot
/// both fire the meta tick simultaneously.
const META_LOCK_KEY: i64 = 884_471_193_001;

/// Postgres advisory-lock key for the live poller.
const LIVE_LOCK_KEY: i64 = 884_471_193_002;

/// Acquire the metadata-poller advisory lock. `pg_advisory_lock` is
/// session-scoped — the same connection that acquires it must
/// release it, otherwise Postgres emits
/// `you don't own a lock of type ExclusiveLock` and the lock leaks
/// until the holding session ends.
///
/// Callers therefore pass a dedicated `PgConnection` (acquired via
/// `pool.acquire()`), hold it for the duration of the tick, and
/// pass the same one to [`release_meta_lock`]. The connection is
/// *only* used for lock management; the tick body's own SQL goes
/// through the pool as usual.
pub async fn try_meta_lock(conn: &mut PgConnection) -> Result<bool> {
    try_lock(conn, META_LOCK_KEY).await
}

pub async fn release_meta_lock(conn: &mut PgConnection) -> Result<()> {
    release_lock(conn, META_LOCK_KEY).await
}

pub async fn try_live_lock(conn: &mut PgConnection) -> Result<bool> {
    try_lock(conn, LIVE_LOCK_KEY).await
}

pub async fn release_live_lock(conn: &mut PgConnection) -> Result<()> {
    release_lock(conn, LIVE_LOCK_KEY).await
}

async fn try_lock(conn: &mut PgConnection, key: i64) -> Result<bool> {
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(key)
        .fetch_one(&mut *conn)
        .await
        .map_err(Error::Database)?;
    Ok(acquired)
}

async fn release_lock(conn: &mut PgConnection, key: i64) -> Result<()> {
    let _: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .fetch_one(&mut *conn)
        .await
        .map_err(Error::Database)?;
    Ok(())
}

// ---------------------------------------------------------------------
// nhl_games
// ---------------------------------------------------------------------

/// Upsert a game row from a schedule payload. The live poller calls
/// [`update_game_live_state`] for mid-game score/period updates; this
/// function is the full-row writer used by the meta poller and the
/// rehydrate admin endpoint.
///
/// Returns `true` when the game is new or its `game_state` changed. The
/// meta poller re-writes every game each tick, so this is what tells it
/// whether date-scoped caches actually went stale.
pub async fn upsert_game(pool: &PgPool, game: &TodayGame, game_date: &str) -> Result<bool> {
    let period_number = game
        .period_descriptor
        .as_ref()
        .and_then(|p| p.number)
        .map(|n| n as i16);
    let period_type = game
        .period_descriptor
        .as_ref()
        .and_then(|p| p.period_type.clone());
    let series_status = game
        .series_status
        .as_ref()
        .map(|s| serde_json::to_value(s).unwrap_or(Value::Null));
    let (home_score, away_score) = match game.game_score.as_ref() {
        Some(s) => (Some(s.home), Some(s.away)),
        None => (game.home_team.score, game.away_team.score),
    };
    let game_state = game.game_state.as_str().to_string();
    let final_state_detected = matches!(game_state.as_str(), "FINAL" | "OFF");

    // The `prev` CTE reads the pre-statement snapshot, so it sees the
    // state before this upsert lands.
    let changed: bool = sqlx::query_scalar(
        r#"
        WITH prev AS (SELECT game_state FROM nhl_games WHERE game_id = $1)
        INSERT INTO nhl_games (
            game_id, season, game_type, game_date, start_time_utc, game_state,
            home_team, away_team, home_score, away_score,
            period_number, period_type, series_status, venue,
            final_state_detected_at, updated_at
        )
        VALUES ($1, $2, $3, $4::date, $5::timestamptz, $6,
                $7, $8, $9, $10, $11, $12, $13, $14,
                CASE WHEN $15::bool THEN NOW() ELSE NULL END, NOW())
        ON CONFLICT (game_id) DO UPDATE SET
            season = EXCLUDED.season,
            game_type = EXCLUDED.game_type,
            game_date = EXCLUDED.game_date,
            start_time_utc = EXCLUDED.start_time_utc,
            game_state = EXCLUDED.game_state,
            home_team = EXCLUDED.home_team,
            away_team = EXCLUDED.away_team,
            home_score = COALESCE(EXCLUDED.home_score, nhl_games.home_score),
            away_score = COALESCE(EXCLUDED.away_score, nhl_games.away_score),
            period_number = COALESCE(EXCLUDED.period_number, nhl_games.period_number),
            period_type = COALESCE(EXCLUDED.period_type, nhl_games.period_type),
            series_status = COALESCE(EXCLUDED.series_status, nhl_games.series_status),
            venue = EXCLUDED.venue,
            final_state_detected_at = CASE
                WHEN EXCLUDED.game_state IN ('FINAL', 'OFF')
                THEN COALESCE(nhl_games.final_state_detected_at, EXCLUDED.final_state_detected_at, NOW())
                ELSE nhl_games.final_state_detected_at
            END,
            updated_at = NOW()
        RETURNING NOT EXISTS (SELECT 1 FROM prev WHERE prev.game_state = nhl_games.game_state)
        "#,
    )
    .bind(game.id as i64)
    .bind(game.season as i32)
    .bind(game.game_type as i16)
    .bind(game_date)
    .bind(&game.start_time_utc)
    .bind(&game_state)
    .bind(&game.home_team.abbrev)
    .bind(&game.away_team.abbrev)
    .bind(home_score)
    .bind(away_score)
    .bind(period_number)
    .bind(period_type)
    .bind(series_status)
    .bind(&game.venue.default)
    .bind(final_state_detected)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;
    Ok(changed)
}

/// Mark unresolved schedule rows as cancelled when they disappeared
/// from the latest authoritative schedule payload for the same date.
pub async fn reconcile_schedule_for_date(
    pool: &PgPool,
    date: &str,
    season: i32,
    game_type: i16,
    games: &[TodayGame],
) -> Result<u64> {
    let fetched_ids: Vec<i64> = games.iter().map(|g| g.id as i64).collect();
    let result = sqlx::query(
        r#"
        UPDATE nhl_games
           SET game_state = 'CANCELLED',
               updated_at = NOW()
         WHERE game_date = $1::date
           AND season = $2
           AND game_type = $3
           AND game_state IN ('FUT', 'PRE')
           AND NOT (game_id = ANY($4::bigint[]))
        "#,
    )
    .bind(date)
    .bind(season)
    .bind(game_type)
    .bind(fetched_ids)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(result.rows_affected())
}

/// Update the live-state columns of an existing `nhl_games` row.
/// Does not touch schedule fields; the meta poller owns those.
pub async fn update_game_live_state(
    pool: &PgPool,
    game_id: i64,
    game_state: &str,
    home_score: Option<i32>,
    away_score: Option<i32>,
    period_number: Option<i16>,
    period_type: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE nhl_games
           SET game_state = $2,
               home_score = COALESCE($3, home_score),
               away_score = COALESCE($4, away_score),
               period_number = COALESCE($5, period_number),
               period_type = COALESCE($6, period_type),
               final_state_detected_at = CASE
                   WHEN $2 IN ('FINAL', 'OFF') THEN COALESCE(final_state_detected_at, NOW())
                   ELSE final_state_detected_at
               END,
               updated_at = NOW()
         WHERE game_id = $1
        "#,
    )
    .bind(game_id)
    .bind(game_state)
    .bind(home_score)
    .bind(away_score)
    .bind(period_number)
    .bind(period_type)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

/// Game IDs on `date` whose state indicates they are worth polling
/// live: currently LIVE, CRIT, or PRE (warm-up). FUT games will
/// transition via the meta poller; OFF/FINAL games are settled.
pub async fn list_live_game_ids_for_date(pool: &PgPool, date: &str) -> Result<Vec<i64>> {
    let rows: Vec<i64> = sqlx::query_scalar(
        r#"
        SELECT game_id FROM nhl_games
         WHERE game_date = $1::date
           AND game_state IN ('LIVE', 'CRIT', 'PRE')
           AND game_state <> 'CANCELLED'
        "#,
    )
    .bind(date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Games the live poller should re-check on every tick:
///   - anything in `LIVE` / `CRIT` from *any* date (self-heal for rows
///     stuck in a non-final state after a crash or rate-limit blip),
///   - plus `PRE` rows for `today` (today's upcoming puck-drops).
///
/// The date-scoped variant (`list_live_game_ids_for_date`) is still
/// used by `process_daily_rankings` as a "is the day settled yet"
/// safety gate; that caller wants strict per-date semantics.
pub async fn list_games_needing_poll(pool: &PgPool, today: &str) -> Result<Vec<i64>> {
    let rows: Vec<i64> = sqlx::query_scalar(
        r#"
        SELECT game_id FROM nhl_games
         WHERE (game_state IN ('LIVE', 'CRIT')
            OR (game_state = 'PRE' AND game_date = $1::date))
           AND game_state <> 'CANCELLED'
        "#,
    )
    .bind(today)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Current state of a game, for transition detection.
#[derive(Debug, sqlx::FromRow)]
pub struct StartedGameRow {
    pub game_id: i64,
    pub home_team: String,
    pub away_team: String,
    pub game_state: String,
}

/// Every game of `season` that has started (anything but FUT and
/// CANCELLED), for the rehydrate boxscore backfill.
pub async fn list_started_games(pool: &PgPool, season: i32) -> Result<Vec<StartedGameRow>> {
    sqlx::query_as::<_, StartedGameRow>(
        r#"
        SELECT game_id, home_team, away_team, game_state
          FROM nhl_games
         WHERE season = $1
           AND game_state NOT IN ('FUT', 'CANCELLED')
        "#,
    )
    .bind(season)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

pub async fn player_game_stats_is_empty(pool: &PgPool) -> Result<bool> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM nhl_player_game_stats)")
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(!exists)
}

pub async fn get_game_state(pool: &PgPool, game_id: i64) -> Result<Option<String>> {
    let state: Option<String> =
        sqlx::query_scalar("SELECT game_state FROM nhl_games WHERE game_id = $1")
            .bind(game_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;
    Ok(state)
}

/// Game IDs in `FINAL`/`OFF` whose boxscore has not yet been sealed and
/// whose first observed final-state timestamp is older than `grace`.
/// Used by the live poller's final-sync pass to give NHL post-buzzer
/// scoring corrections time to land before the row becomes immutable.
///
/// `updated_at` is intentionally ignored here because the meta poller
/// keeps refreshing completed schedule rows; using it would move the
/// grace window forward forever on busy playoff days.
pub async fn list_games_needing_final_sync(
    pool: &PgPool,
    grace: chrono::Duration,
) -> Result<Vec<i64>> {
    let rows: Vec<i64> = sqlx::query_scalar(
        r#"
        SELECT game_id FROM nhl_games
         WHERE game_state IN ('FINAL', 'OFF')
           AND stats_finalized_at IS NULL
           AND COALESCE(final_state_detected_at, updated_at) < NOW() - ($1 || ' seconds')::interval
        "#,
    )
    .bind(grace.num_seconds().to_string())
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Stamp `stats_finalized_at = NOW()` on a game. Idempotent: a second
/// call within the same second is a no-op semantically and the SQL
/// merely overwrites the timestamp. Callers should only invoke this
/// after a successful `upsert_boxscore_players` for the game so the
/// "sealed" promise holds.
pub async fn mark_game_stats_finalized(pool: &PgPool, game_id: i64) -> Result<()> {
    sqlx::query("UPDATE nhl_games SET stats_finalized_at = NOW() WHERE game_id = $1")
        .bind(game_id)
        .execute(pool)
        .await
        .map_err(Error::Database)?;
    Ok(())
}

// ---------------------------------------------------------------------
// nhl_player_game_stats
// ---------------------------------------------------------------------

/// Replace every `nhl_player_game_stats` row for `game_id` from the
/// boxscore (skaters and goalies, both teams) in one batched upsert, and
/// derive the scoreboard line from the same payload. The schedule
/// endpoint drops `game_score` for completed playoff games and
/// `get_game_data` can 404 mid-game; the boxscore always has the goals.
///
/// Returns the number of player rows written.
pub async fn upsert_boxscore_players(
    pool: &PgPool,
    game_id: i64,
    home_abbrev: &str,
    away_abbrev: &str,
    boxscore: &GameBoxscore,
) -> Result<usize> {
    let home = &boxscore.player_by_game_stats.home_team;
    let away = &boxscore.player_by_game_stats.away_team;
    let players: Vec<(&str, &BoxscorePlayer)> = home
        .all_players()
        .map(|p| (home_abbrev, p))
        .chain(away.all_players().map(|p| (away_abbrev, p)))
        .collect();

    let n = players.len();
    let (mut ids, mut teams, mut positions, mut names) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    let (mut goals, mut assists, mut points) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    let (mut sog, mut pim, mut plus_minus, mut hits, mut toi) = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    for (team, p) in &players {
        let g = p.goals.unwrap_or(0);
        let a = p.assists.unwrap_or(0);
        ids.push(p.player_id as i64);
        teams.push(team.to_string());
        positions.push(p.position.clone());
        names.push(
            p.name
                .get("default")
                .cloned()
                .unwrap_or_else(|| format!("Player {}", p.player_id)),
        );
        goals.push(g);
        assists.push(a);
        points.push(p.points.unwrap_or(g + a));
        sog.push(p.sog);
        pim.push(p.pim);
        plus_minus.push(p.plus_minus);
        hits.push(p.hits);
        toi.push(p.toi.as_deref().and_then(parse_toi_seconds));
    }

    let mut tx = pool.begin().await.map_err(Error::Database)?;
    sqlx::query(
        r#"
        INSERT INTO nhl_player_game_stats (
            game_id, player_id, team_abbrev, position, name,
            goals, assists, points, sog, pim, plus_minus, hits, toi_seconds, updated_at
        )
        SELECT $1, u.*, NOW()
          FROM UNNEST($2::bigint[], $3::text[], $4::text[], $5::text[],
                      $6::int[], $7::int[], $8::int[], $9::int[], $10::int[],
                      $11::int[], $12::int[], $13::int[])
            AS u(player_id, team_abbrev, position, name, goals, assists, points,
                 sog, pim, plus_minus, hits, toi_seconds)
        ON CONFLICT (game_id, player_id) DO UPDATE SET
            team_abbrev = EXCLUDED.team_abbrev,
            position = EXCLUDED.position,
            name = EXCLUDED.name,
            goals = EXCLUDED.goals,
            assists = EXCLUDED.assists,
            points = EXCLUDED.points,
            sog = EXCLUDED.sog,
            pim = EXCLUDED.pim,
            plus_minus = EXCLUDED.plus_minus,
            hits = EXCLUDED.hits,
            toi_seconds = COALESCE(EXCLUDED.toi_seconds, nhl_player_game_stats.toi_seconds),
            updated_at = NOW()
        "#,
    )
    .bind(game_id)
    .bind(&ids)
    .bind(&teams)
    .bind(&positions)
    .bind(&names)
    .bind(&goals)
    .bind(&assists)
    .bind(&points)
    .bind(&sog)
    .bind(&pim)
    .bind(&plus_minus)
    .bind(&hits)
    .bind(&toi)
    .execute(&mut *tx)
    .await
    .map_err(Error::Database)?;

    sqlx::query(
        "UPDATE nhl_games SET home_score = $2, away_score = $3, updated_at = NOW() WHERE game_id = $1",
    )
    .bind(game_id)
    .bind(home.skater_goals())
    .bind(away.skater_goals())
    .execute(&mut *tx)
    .await
    .map_err(Error::Database)?;

    tx.commit().await.map_err(Error::Database)?;
    Ok(n)
}

/// Parse `"MM:SS"` (as emitted by the NHL boxscore) into total seconds.
/// Returns `None` for empty strings, malformed values, or non-numeric
/// parts — callers treat `None` as "no data" and preserve whatever was
/// previously stored (via `COALESCE` in the upsert).
fn parse_toi_seconds(toi: &str) -> Option<i32> {
    let trimmed = toi.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (m, s) = trimmed.split_once(':')?;
    let minutes: i32 = m.trim().parse().ok()?;
    let seconds: i32 = s.trim().parse().ok()?;
    if minutes < 0 || !(0..60).contains(&seconds) {
        return None;
    }
    Some(minutes * 60 + seconds)
}

#[cfg(test)]
mod parse_toi_tests {
    use super::parse_toi_seconds;

    #[test]
    fn well_formed_values_parse() {
        assert_eq!(parse_toi_seconds("18:24"), Some(18 * 60 + 24));
        assert_eq!(parse_toi_seconds("0:05"), Some(5));
        assert_eq!(parse_toi_seconds("26:04"), Some(26 * 60 + 4));
    }

    #[test]
    fn malformed_returns_none() {
        assert_eq!(parse_toi_seconds(""), None);
        assert_eq!(parse_toi_seconds("foo"), None);
        assert_eq!(parse_toi_seconds("18"), None);
        assert_eq!(parse_toi_seconds("18:xx"), None);
        assert_eq!(parse_toi_seconds("-1:30"), None);
        assert_eq!(parse_toi_seconds("20:75"), None);
    }
}

// ---------------------------------------------------------------------
// nhl_skater_season_stats
// ---------------------------------------------------------------------

/// One skater's season line. `None` counting stats mean "this source
/// doesn't know", not zero: the leaderboard only lists a player in the
/// categories where they rank top-N, while club stats carries every
/// number. Unknown values never overwrite known ones.
#[derive(Debug, Default)]
pub struct SkaterSeasonUpsert {
    pub player_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub team_abbrev: String,
    pub position: String,
    pub goals: Option<i32>,
    pub assists: Option<i32>,
    pub points: Option<i32>,
    pub plus_minus: Option<i32>,
    pub faceoff_pct: Option<f32>,
    pub toi_per_game: Option<i32>,
    pub sog: Option<i32>,
}

/// Batched writer shared by the leaderboard and club-stats sources.
/// One statement: the CTE updates existing rows with
/// `COALESCE(new, old)` and the INSERT adds the rest. `ON CONFLICT DO
/// NOTHING` covers a concurrent insert of the same key, which the next
/// tick reconciles.
async fn upsert_skater_season_rows(
    pool: &PgPool,
    season: i32,
    game_type: i16,
    rows: &[SkaterSeasonUpsert],
) -> Result<usize> {
    if rows.is_empty() {
        return Ok(0);
    }
    let col = |f: fn(&SkaterSeasonUpsert) -> String| rows.iter().map(f).collect::<Vec<_>>();
    let opt = |f: fn(&SkaterSeasonUpsert) -> Option<i32>| rows.iter().map(f).collect::<Vec<_>>();
    let ids: Vec<i64> = rows.iter().map(|r| r.player_id).collect();
    let faceoff: Vec<Option<f32>> = rows.iter().map(|r| r.faceoff_pct).collect();
    let headshots: Vec<String> = rows
        .iter()
        .map(|r| {
            format!(
                "https://assets.nhle.com/mugs/nhl/{season}/{}/{}.png",
                r.team_abbrev, r.player_id
            )
        })
        .collect();

    sqlx::query(
        r#"
        WITH u AS (
            SELECT * FROM UNNEST($3::bigint[], $4::text[], $5::text[], $6::text[], $7::text[],
                                 $8::int[], $9::int[], $10::int[], $11::int[], $12::real[],
                                 $13::int[], $14::int[], $15::text[])
              AS u(player_id, first_name, last_name, team_abbrev, position,
                   goals, assists, points, plus_minus, faceoff_pct,
                   toi_per_game, sog, headshot_url)
        ),
        updated AS (
            UPDATE nhl_skater_season_stats s SET
                first_name = u.first_name,
                last_name = u.last_name,
                team_abbrev = u.team_abbrev,
                position = u.position,
                goals = COALESCE(u.goals, s.goals),
                assists = COALESCE(u.assists, s.assists),
                points = COALESCE(u.points, s.points),
                plus_minus = COALESCE(u.plus_minus, s.plus_minus),
                faceoff_pct = COALESCE(u.faceoff_pct, s.faceoff_pct),
                toi_per_game = COALESCE(u.toi_per_game, s.toi_per_game),
                sog = COALESCE(u.sog, s.sog),
                headshot_url = u.headshot_url,
                updated_at = NOW()
              FROM u
             WHERE s.player_id = u.player_id AND s.season = $1 AND s.game_type = $2
            RETURNING s.player_id
        )
        INSERT INTO nhl_skater_season_stats (
            player_id, season, game_type, first_name, last_name,
            team_abbrev, position, goals, assists, points,
            plus_minus, faceoff_pct, toi_per_game, sog, headshot_url, updated_at
        )
        SELECT u.player_id, $1, $2, u.first_name, u.last_name, u.team_abbrev, u.position,
               COALESCE(u.goals, 0), COALESCE(u.assists, 0), COALESCE(u.points, 0),
               u.plus_minus, u.faceoff_pct, u.toi_per_game, u.sog, u.headshot_url, NOW()
          FROM u
         WHERE u.player_id NOT IN (SELECT player_id FROM updated)
        ON CONFLICT (player_id, season, game_type) DO NOTHING
        "#,
    )
    .bind(season)
    .bind(game_type)
    .bind(&ids)
    .bind(col(|r| r.first_name.clone()))
    .bind(col(|r| r.last_name.clone()))
    .bind(col(|r| r.team_abbrev.clone()))
    .bind(col(|r| r.position.clone()))
    .bind(opt(|r| r.goals))
    .bind(opt(|r| r.assists))
    .bind(opt(|r| r.points))
    .bind(opt(|r| r.plus_minus))
    .bind(&faceoff)
    .bind(opt(|r| r.toi_per_game))
    .bind(opt(|r| r.sog))
    .bind(&headshots)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows.len())
}

/// Flatten the per-category skater leaderboard into one row per player.
/// A category the player isn't ranked in stays `None`.
pub async fn upsert_skater_leaderboard(
    pool: &PgPool,
    season: i32,
    game_type: i16,
    leaders: &StatsLeaders,
) -> Result<usize> {
    use std::collections::HashMap;

    let mut map: HashMap<i64, SkaterSeasonUpsert> = HashMap::new();
    let mut apply = |list: &[Player], set: fn(&mut SkaterSeasonUpsert, f64)| {
        for p in list {
            let row = map
                .entry(p.id as i64)
                .or_insert_with(|| SkaterSeasonUpsert {
                    player_id: p.id as i64,
                    first_name: default_name(&p.first_name),
                    last_name: default_name(&p.last_name),
                    team_abbrev: p.team_abbrev.clone(),
                    position: p.position.clone(),
                    ..Default::default()
                });
            set(row, p.value);
        }
    };
    apply(&leaders.goals, |r, v| r.goals = Some(v as i32));
    apply(&leaders.assists, |r, v| r.assists = Some(v as i32));
    apply(&leaders.points, |r, v| r.points = Some(v as i32));
    apply(&leaders.plus_minus, |r, v| r.plus_minus = Some(v as i32));
    apply(&leaders.faceoff_leaders, |r, v| {
        r.faceoff_pct = Some(v as f32)
    });
    // TOI arrives as seconds per game.
    apply(&leaders.toi, |r, v| r.toi_per_game = Some(v as i32));

    let rows: Vec<SkaterSeasonUpsert> = map.into_values().collect();
    upsert_skater_season_rows(pool, season, game_type, &rows).await
}

/// Upsert the full per-team skater season line from
/// `/v1/club-stats/{team}/{season}/{gt}`: every skater the team has
/// dressed, so the projection model covers depth players.
/// `avgTimeOnIcePerGame` is float seconds per game; rounded on write.
pub async fn upsert_team_club_stats(
    pool: &PgPool,
    season: i32,
    game_type: i16,
    team_abbrev: &str,
    skaters: &[crate::domain::models::nhl::ClubStatsSkater],
) -> Result<usize> {
    let rows: Vec<SkaterSeasonUpsert> = skaters
        .iter()
        .map(|s| SkaterSeasonUpsert {
            player_id: s.player_id,
            first_name: default_name(&s.first_name),
            last_name: default_name(&s.last_name),
            team_abbrev: team_abbrev.to_string(),
            position: s.position_code.clone(),
            goals: Some(s.goals),
            assists: Some(s.assists),
            points: Some(s.points),
            plus_minus: s.plus_minus,
            faceoff_pct: s.faceoff_winning_pctg,
            toi_per_game: s.avg_time_on_ice_per_game.map(|f| f.round() as i32),
            sog: s.shots,
        })
        .collect();
    upsert_skater_season_rows(pool, season, game_type, &rows).await
}

// ---------------------------------------------------------------------
// nhl_goalie_season_stats
// ---------------------------------------------------------------------

pub async fn upsert_goalie_leaderboard(
    pool: &PgPool,
    season: i32,
    game_type: i16,
    leaders: &GoalieStatsLeaders,
) -> Result<usize> {
    use std::collections::HashMap;

    #[derive(Default)]
    struct Row {
        team: String,
        name: String,
        wins: Option<i32>,
        gaa: Option<f32>,
        save_pctg: Option<f32>,
        shutouts: Option<i32>,
    }
    let mut map: HashMap<i64, Row> = HashMap::new();
    let mut apply = |list: &[Player], set: fn(&mut Row, f64)| {
        for p in list {
            let row = map.entry(p.id as i64).or_insert_with(|| Row {
                team: p.team_abbrev.clone(),
                name: format!(
                    "{} {}",
                    default_name(&p.first_name),
                    default_name(&p.last_name)
                )
                .trim()
                .to_string(),
                ..Default::default()
            });
            set(row, p.value);
        }
    };
    apply(&leaders.wins, |r, v| r.wins = Some(v as i32));
    apply(&leaders.goals_against_average, |r, v| {
        r.gaa = Some(v as f32)
    });
    apply(&leaders.save_pctg, |r, v| r.save_pctg = Some(v as f32));
    apply(&leaders.shutouts, |r, v| r.shutouts = Some(v as i32));

    let n = map.len();
    let mut ids = Vec::with_capacity(n);
    let mut teams = Vec::with_capacity(n);
    let mut names = Vec::with_capacity(n);
    let mut wins = Vec::with_capacity(n);
    let mut gaa = Vec::with_capacity(n);
    let mut save = Vec::with_capacity(n);
    let mut shutouts = Vec::with_capacity(n);
    for (id, r) in map {
        ids.push(id);
        teams.push(r.team);
        names.push(r.name);
        wins.push(r.wins);
        gaa.push(r.gaa);
        save.push(r.save_pctg);
        shutouts.push(r.shutouts);
    }

    sqlx::query(
        r#"
        INSERT INTO nhl_goalie_season_stats (
            player_id, season, game_type, team_abbrev, name,
            record, wins, gaa, save_pctg, shutouts, updated_at
        )
        SELECT u.player_id, $1, $2, u.team_abbrev, u.name, NULL, u.wins, u.gaa, u.save_pctg, u.shutouts, NOW()
          FROM UNNEST($3::bigint[], $4::text[], $5::text[], $6::int[], $7::real[], $8::real[], $9::int[])
            AS u(player_id, team_abbrev, name, wins, gaa, save_pctg, shutouts)
        ON CONFLICT (player_id, season, game_type) DO UPDATE SET
            team_abbrev = EXCLUDED.team_abbrev,
            name = EXCLUDED.name,
            wins = COALESCE(EXCLUDED.wins, nhl_goalie_season_stats.wins),
            gaa = COALESCE(EXCLUDED.gaa, nhl_goalie_season_stats.gaa),
            save_pctg = COALESCE(EXCLUDED.save_pctg, nhl_goalie_season_stats.save_pctg),
            shutouts = COALESCE(EXCLUDED.shutouts, nhl_goalie_season_stats.shutouts),
            updated_at = NOW()
        "#,
    )
    .bind(season)
    .bind(game_type)
    .bind(&ids)
    .bind(&teams)
    .bind(&names)
    .bind(&wins)
    .bind(&gaa)
    .bind(&save)
    .bind(&shutouts)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(n)
}

// ---------------------------------------------------------------------
// nhl_standings
// ---------------------------------------------------------------------

pub async fn upsert_standings(pool: &PgPool, season: i32, payload: &Value) -> Result<usize> {
    let entries = payload
        .get("standings")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::nhl_api("standings payload missing 'standings' array"))?;

    let int = |row: &Value, key: &str| row.get(key).and_then(Value::as_i64).map(|v| v as i32);
    let (mut teams, mut points, mut gp, mut wins, mut losses, mut otl) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let (mut pct, mut streak_code, mut streak_count, mut l10_w, mut l10_l, mut l10_otl) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let mut raw = Vec::new();
    for row in entries {
        let team = row
            .get("teamAbbrev")
            .and_then(|v| v.get("default"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if team.is_empty() {
            continue;
        }
        teams.push(team.to_string());
        points.push(int(row, "points").unwrap_or(0));
        gp.push(int(row, "gamesPlayed").unwrap_or(0));
        wins.push(int(row, "wins").unwrap_or(0));
        losses.push(int(row, "losses").unwrap_or(0));
        otl.push(int(row, "otLosses").unwrap_or(0));
        pct.push(
            row.get("pointPctg")
                .and_then(Value::as_f64)
                .map(|v| v as f32),
        );
        streak_code.push(
            row.get("streakCode")
                .and_then(Value::as_str)
                .map(String::from),
        );
        streak_count.push(int(row, "streakCount"));
        l10_w.push(int(row, "l10Wins"));
        l10_l.push(int(row, "l10Losses"));
        l10_otl.push(int(row, "l10OtLosses"));
        raw.push(row.clone());
    }

    sqlx::query(
        r#"
        INSERT INTO nhl_standings (
            season, team_abbrev, points, games_played, wins, losses, ot_losses,
            point_pctg, streak_code, streak_count, l10_wins, l10_losses, l10_ot_losses,
            raw, updated_at
        )
        SELECT $1, u.*, NOW()
          FROM UNNEST($2::text[], $3::int[], $4::int[], $5::int[], $6::int[], $7::int[],
                      $8::real[], $9::text[], $10::int[], $11::int[], $12::int[], $13::int[],
                      $14::jsonb[])
            AS u(team_abbrev, points, games_played, wins, losses, ot_losses,
                 point_pctg, streak_code, streak_count, l10_wins, l10_losses, l10_ot_losses, raw)
        ON CONFLICT (season, team_abbrev) DO UPDATE SET
            points = EXCLUDED.points,
            games_played = EXCLUDED.games_played,
            wins = EXCLUDED.wins,
            losses = EXCLUDED.losses,
            ot_losses = EXCLUDED.ot_losses,
            point_pctg = EXCLUDED.point_pctg,
            streak_code = EXCLUDED.streak_code,
            streak_count = EXCLUDED.streak_count,
            l10_wins = EXCLUDED.l10_wins,
            l10_losses = EXCLUDED.l10_losses,
            l10_ot_losses = EXCLUDED.l10_ot_losses,
            raw = EXCLUDED.raw,
            updated_at = NOW()
        "#,
    )
    .bind(season)
    .bind(&teams)
    .bind(&points)
    .bind(&gp)
    .bind(&wins)
    .bind(&losses)
    .bind(&otl)
    .bind(&pct)
    .bind(&streak_code)
    .bind(&streak_count)
    .bind(&l10_w)
    .bind(&l10_l)
    .bind(&l10_otl)
    .bind(&raw)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(teams.len())
}

// ---------------------------------------------------------------------
// nhl_team_rosters
// ---------------------------------------------------------------------

pub async fn upsert_team_roster(
    pool: &PgPool,
    team_abbrev: &str,
    season: i32,
    roster: &[Player],
) -> Result<()> {
    // Serialization failure must not overwrite a good roster with NULL.
    let json = serde_json::to_value(roster).context("failed to serialize roster")?;
    sqlx::query(
        r#"
        INSERT INTO nhl_team_rosters (team_abbrev, season, roster, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (team_abbrev, season) DO UPDATE SET
            roster = EXCLUDED.roster,
            updated_at = NOW()
        "#,
    )
    .bind(team_abbrev)
    .bind(season)
    .bind(json)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

/// The mirrored roster for `team_abbrev`, or `None` if it hasn't been
/// captured for `season` yet.
pub async fn get_team_roster(
    pool: &PgPool,
    team_abbrev: &str,
    season: i32,
) -> Result<Option<Vec<Player>>> {
    let raw: Option<Value> = sqlx::query_scalar(
        "SELECT roster FROM nhl_team_rosters WHERE team_abbrev = $1 AND season = $2",
    )
    .bind(team_abbrev)
    .bind(season)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;
    raw.map(|v| serde_json::from_value(v).context("failed to decode mirrored roster"))
        .transpose()
        .map_err(Error::from)
}

// ---------------------------------------------------------------------
// nhl_playoff_bracket
// ---------------------------------------------------------------------

pub async fn upsert_playoff_bracket(
    pool: &PgPool,
    season: i32,
    carousel: &PlayoffCarousel,
) -> Result<()> {
    let json = serde_json::to_value(carousel).context("failed to serialize playoff carousel")?;
    sqlx::query(
        r#"
        INSERT INTO nhl_playoff_bracket (season, carousel, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (season) DO UPDATE SET
            carousel = EXCLUDED.carousel,
            updated_at = NOW()
        "#,
    )
    .bind(season)
    .bind(json)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

// ---------------------------------------------------------------------
// nhl_game_landing (write-once)
// ---------------------------------------------------------------------

/// Capture the pre-game matchup block for `game_id` **if and only if**
/// one has not already been captured. The NHL API replaces the
/// matchup block with a LIVE-state shape once the game starts, which
/// would overwrite the pre-game data we want to surface on Insights
/// all day — so we insert with `ON CONFLICT DO NOTHING` and skip
/// captures whose `matchup` looks empty.
pub async fn capture_game_landing(pool: &PgPool, game_id: i64, matchup: &Value) -> Result<bool> {
    if matchup.is_null() || matchup.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        return Ok(false);
    }
    let inserted = sqlx::query(
        r#"
        INSERT INTO nhl_game_landing (game_id, matchup, captured_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (game_id) DO NOTHING
        "#,
    )
    .bind(game_id)
    .bind(matchup)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(inserted.rows_affected() > 0)
}

// ---------------------------------------------------------------------
// nhl_skater_edge
// ---------------------------------------------------------------------

/// Upsert NHL Edge telemetry (top skating speed, top shot speed) for a
/// single player. Called by the nightly `edge_refresher` job.
pub async fn upsert_skater_edge(
    pool: &PgPool,
    player_id: i64,
    top_speed_mph: Option<f32>,
    top_shot_speed_mph: Option<f32>,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO nhl_skater_edge (player_id, top_speed_mph, top_shot_speed_mph, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (player_id) DO UPDATE SET
            top_speed_mph      = EXCLUDED.top_speed_mph,
            top_shot_speed_mph = EXCLUDED.top_shot_speed_mph,
            updated_at         = NOW()
        "#,
    )
    .bind(player_id)
    .bind(top_speed_mph)
    .bind(top_shot_speed_mph)
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

/// Timestamp of the most recent Edge refresh, across all players.
/// The refresher uses this to skip a run when yesterday's data is
/// still within the freshness window.
pub async fn last_update_nhl_skater_edge(
    pool: &PgPool,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let ts: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT MAX(updated_at) FROM nhl_skater_edge")
            .fetch_one(pool)
            .await
            .map_err(Error::Database)?;
    Ok(ts)
}

// ---------------------------------------------------------------------
// Read-side queries consumed by handlers (Phase 3+).
// ---------------------------------------------------------------------

use sqlx::FromRow;

/// One row per (fantasy_team, rostered_player) with that player's
/// NHL performance on `date`. A team with no rostered player in a
/// completed game that day will not appear. Caller is expected to
/// LEFT JOIN the handler response against every league team so that
/// the final output lists every team, zeros included.
#[derive(Debug, FromRow)]
pub struct LeagueTeamPlayerDailyRow {
    pub team_id: i64,
    pub team_name: String,
    pub nhl_id: i64,
    pub player_name: String,
    pub nhl_team: String,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}

/// Every rostered skater from `league_id` who appeared in any game
/// on `date`, with their boxscore stats for that game.
pub async fn list_league_player_stats_for_date(
    pool: &PgPool,
    league_id: &str,
    date: &str,
) -> Result<Vec<LeagueTeamPlayerDailyRow>> {
    let rows = sqlx::query_as::<_, LeagueTeamPlayerDailyRow>(
        r#"
        SELECT
            ft.id              AS team_id,
            ft.name            AS team_name,
            fp.nhl_id          AS nhl_id,
            fp.name            AS player_name,
            fp.nhl_team        AS nhl_team,
            pgs.goals          AS goals,
            pgs.assists        AS assists,
            pgs.points         AS points
        FROM nhl_player_game_stats pgs
        JOIN nhl_games  g  ON g.game_id = pgs.game_id
        JOIN fantasy_players fp ON fp.nhl_id = pgs.player_id
        JOIN fantasy_teams   ft ON ft.id    = fp.team_id
        WHERE ft.league_id = $1::uuid
          AND g.game_date  = $2::date
        ORDER BY ft.id, pgs.points DESC, pgs.goals DESC
        "#,
    )
    .bind(league_id)
    .bind(date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Row shape matching the `nhl_skater_season_stats` columns the
/// overall rankings handler actually reads. Thinner than the full
/// mirror row — the handler only needs id + name + counting stats.
#[derive(Debug, FromRow)]
pub struct SkaterSeasonRow {
    pub player_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub team_abbrev: String,
    pub position: String,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
}

/// Snapshot of a single team's standings-derived context used on
/// the Insights page: streak string and L10 record, plus the raw
/// points figure so callers don't need a second query for ratings.
#[derive(Debug, FromRow)]
pub struct TeamStandingsContextRow {
    pub team_abbrev: String,
    pub points: i32,
    pub streak_code: Option<String>,
    pub streak_count: Option<i32>,
    pub l10_wins: Option<i32>,
    pub l10_losses: Option<i32>,
    pub l10_ot_losses: Option<i32>,
}

/// Every team's streak + L10 + points context, pulled from the
/// mirrored standings. Safe to call every request — the meta poller
/// keeps this fresh on the aggregate cadence.
pub async fn list_team_standings_context(
    pool: &PgPool,
    season: i32,
) -> Result<Vec<TeamStandingsContextRow>> {
    let rows = sqlx::query_as::<_, TeamStandingsContextRow>(
        r#"
        SELECT team_abbrev, points, streak_code, streak_count,
               l10_wins, l10_losses, l10_ot_losses
          FROM nhl_standings
         WHERE season = $1
        "#,
    )
    .bind(season)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Per-team playoff streak strings (`"W4"`, `"L3"`, `"OT1"`) computed
/// from `nhl_games` rows for the requested postseason. Walks each
/// team's completed playoff games in reverse chronological order and
/// counts the consecutive run of identical result kinds.
///
/// Result kinds:
///
/// - `W`: any win (regulation or OT — playoffs don't have shootouts)
/// - `L`: regulation loss
/// - `OT`: overtime loss (`period_type = 'OT'` and the team lost)
///
/// Why: `nhl_standings.streak_code` is regular-season-only and freezes
/// at season's end. A team eliminated 4-0 in the playoffs would still
/// show a regular-season `W1` if they happened to win their final
/// regular-season game. The Insights card uses this helper instead in
/// playoff mode.
pub async fn list_team_playoff_streaks(
    pool: &PgPool,
    season: i32,
) -> Result<std::collections::HashMap<String, String>> {
    // (game_id, home_team, away_team, home_score, away_score, period_type)
    type FinalGameRow = (
        i64,
        String,
        String,
        Option<i32>,
        Option<i32>,
        Option<String>,
    );
    let rows: Vec<FinalGameRow> = sqlx::query_as(
        r#"
        SELECT game_id, home_team, away_team, home_score, away_score, period_type
          FROM nhl_games
         WHERE season = $1
           AND game_type = 3
           AND game_state IN ('OFF', 'FINAL')
           AND home_score IS NOT NULL
           AND away_score IS NOT NULL
         ORDER BY game_date DESC, game_id DESC
        "#,
    )
    .bind(season)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;

    // Bucket each team's results in the order returned (already
    // reverse-chronological), tagging each game with the kind of
    // result that team experienced. The first kind in each bucket
    // anchors the streak; we count forward until the kind changes.
    let mut by_team: std::collections::HashMap<String, Vec<&'static str>> =
        std::collections::HashMap::new();
    for (_, home, away, home_score, away_score, period_type) in &rows {
        let (Some(hs), Some(as_)) = (home_score, away_score) else {
            continue;
        };
        let ot = matches!(period_type.as_deref(), Some("OT"));
        let (home_kind, away_kind) = match hs.cmp(as_) {
            std::cmp::Ordering::Greater => ("W", if ot { "OT" } else { "L" }),
            std::cmp::Ordering::Less => (if ot { "OT" } else { "L" }, "W"),
            std::cmp::Ordering::Equal => continue,
        };
        by_team.entry(home.clone()).or_default().push(home_kind);
        by_team.entry(away.clone()).or_default().push(away_kind);
    }

    let mut out = std::collections::HashMap::with_capacity(by_team.len());
    for (team, kinds) in by_team {
        let Some(&first) = kinds.first() else {
            continue;
        };
        let count = kinds.iter().take_while(|&&k| k == first).count();
        out.insert(team, format!("{}{}", first, count));
    }
    Ok(out)
}

/// The NHL-shaped standings payload for functions that still take it
/// (`team_ratings::from_standings`, `playoff_elo::seed_from_standings`,
/// `compute_current_elo`). Served from the mirrored raw entries; rows
/// written before `raw` existed fall back to a reconstruction from the
/// typed columns, which lacks the home/road splits.
pub async fn load_standings_payload(pool: &PgPool, season: i32) -> Result<Value> {
    let raw: Vec<Value> =
        sqlx::query_scalar("SELECT raw FROM nhl_standings WHERE season = $1 AND raw IS NOT NULL")
            .bind(season)
            .fetch_all(pool)
            .await
            .map_err(Error::Database)?;
    if !raw.is_empty() {
        return Ok(serde_json::json!({ "standings": raw }));
    }
    let rows = list_team_standings_context(pool, season).await?;
    let arr: Vec<Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "teamAbbrev": { "default": r.team_abbrev },
                "points": r.points,
                "streakCode": r.streak_code,
                "streakCount": r.streak_count,
                "l10Wins": r.l10_wins,
                "l10Losses": r.l10_losses,
                "l10OtLosses": r.l10_ot_losses,
            })
        })
        .collect();
    Ok(serde_json::json!({ "standings": arr }))
}

/// Pre-game matchup JSON for a given game_id, as written by the
/// meta poller / rehydrate when the game was still in FUT/PRE. The
/// `matchup` block is the live-response shape's NHL calls the
/// landing handler consumed — we store it whole so the handler
/// can re-parse via its existing `build_landing_from_raw`.
pub async fn get_game_landing_matchup(pool: &PgPool, game_id: i64) -> Result<Option<Value>> {
    let row: Option<Value> =
        sqlx::query_scalar("SELECT matchup FROM nhl_game_landing WHERE game_id = $1")
            .bind(game_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;
    Ok(row)
}

/// Game IDs for `date` that are FUT or PRE and have no
/// `nhl_game_landing` row yet. The meta poller uses this to fetch
/// pre-game matchup data exactly once per game — after puck drop the
/// NHL response replaces `matchup` with live-recap data, so a
/// second capture would clobber the pre-game payload.
pub async fn list_games_without_landing_for_date(pool: &PgPool, date: &str) -> Result<Vec<i64>> {
    let rows: Vec<i64> = sqlx::query_scalar(
        r#"
        SELECT g.game_id
          FROM nhl_games g
     LEFT JOIN nhl_game_landing l ON l.game_id = g.game_id
         WHERE g.game_date = $1::date
           AND g.game_state IN ('FUT', 'PRE')
           AND g.game_state <> 'CANCELLED'
           AND l.game_id IS NULL
        "#,
    )
    .bind(date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// NHL Edge row — top speed and top shot speed per player, populated
/// by the nightly edge_refresher job.
#[derive(Debug, FromRow)]
pub struct SkaterEdgeRow {
    pub player_id: i64,
    pub top_speed_mph: Option<f32>,
    pub top_shot_speed_mph: Option<f32>,
}

/// Edge telemetry for the given player IDs. Missing players simply
/// don't appear in the result — the handler falls back to blank
/// speed tiles, which is the correct UX when the nightly refresher
/// hasn't covered a particular player (e.g. a recent call-up).
pub async fn list_skater_edge(pool: &PgPool, player_ids: &[i64]) -> Result<Vec<SkaterEdgeRow>> {
    if player_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, SkaterEdgeRow>(
        r#"
        SELECT player_id, top_speed_mph, top_shot_speed_mph
          FROM nhl_skater_edge
         WHERE player_id = ANY($1)
        "#,
    )
    .bind(player_ids)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// All skaters on the season leaderboard for `(season, game_type)`,
/// ordered by points desc then goals desc. Handlers that only need
/// the subset rostered in a given league filter in memory.
pub async fn list_skater_season_stats(
    pool: &PgPool,
    season: i32,
    game_type: i16,
) -> Result<Vec<SkaterSeasonRow>> {
    let rows = sqlx::query_as::<_, SkaterSeasonRow>(
        r#"
        SELECT
            player_id, first_name, last_name, team_abbrev, position,
            goals, assists, points
        FROM nhl_skater_season_stats
        WHERE season = $1 AND game_type = $2
        ORDER BY points DESC, goals DESC
        "#,
    )
    .bind(season)
    .bind(game_type)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

#[derive(Debug, FromRow)]
pub struct SkaterLeaderRow {
    pub player_id: i64,
    pub first_name: String,
    pub last_name: String,
    pub team_abbrev: String,
    pub position: String,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
    pub plus_minus: Option<i32>,
    pub toi_per_game: Option<i32>,
}

/// Top `limit` skaters by points from the mirrored season lines.
pub async fn list_skater_leaders(
    pool: &PgPool,
    season: i32,
    game_type: i16,
    limit: i64,
) -> Result<Vec<SkaterLeaderRow>> {
    sqlx::query_as::<_, SkaterLeaderRow>(
        r#"
        SELECT player_id, first_name, last_name, team_abbrev, position,
               goals, assists, points, plus_minus, toi_per_game
          FROM nhl_skater_season_stats
         WHERE season = $1 AND game_type = $2
         ORDER BY points DESC, goals DESC
         LIMIT $3
        "#,
    )
    .bind(season)
    .bind(game_type)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

#[derive(Debug, FromRow)]
pub struct GoalieSeasonRow {
    pub player_id: i64,
    pub team_abbrev: String,
    pub wins: Option<i32>,
    pub save_pctg: Option<f32>,
}

pub async fn list_goalie_season_stats(
    pool: &PgPool,
    season: i32,
    game_type: i16,
) -> Result<Vec<GoalieSeasonRow>> {
    sqlx::query_as::<_, GoalieSeasonRow>(
        r#"
        SELECT player_id, team_abbrev, wins, save_pctg
          FROM nhl_goalie_season_stats
         WHERE season = $1 AND game_type = $2
        "#,
    )
    .bind(season)
    .bind(game_type)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

/// Full mirror row for `nhl_games` as the games/match-day handlers
/// consume it. Schedule fields + live state + series status.
#[derive(Debug, FromRow)]
pub struct NhlGameRow {
    pub game_id: i64,
    pub season: i32,
    pub game_type: i16,
    pub game_date: chrono::NaiveDate,
    pub start_time_utc: chrono::DateTime<chrono::Utc>,
    pub game_state: String,
    pub home_team: String,
    pub away_team: String,
    pub home_score: Option<i32>,
    pub away_score: Option<i32>,
    pub period_number: Option<i16>,
    pub period_type: Option<String>,
    pub series_status: Option<serde_json::Value>,
    pub venue: Option<String>,
}

/// All games scheduled for `date`, ordered by `start_time_utc` so
/// tonight's slate renders in kick-off order.
pub async fn list_games_for_date(pool: &PgPool, date: &str) -> Result<Vec<NhlGameRow>> {
    let rows = sqlx::query_as::<_, NhlGameRow>(
        r#"
        SELECT
            game_id, season, game_type, game_date, start_time_utc, game_state,
            home_team, away_team, home_score, away_score,
            period_number, period_type, series_status, venue
        FROM nhl_games
        WHERE game_date = $1::date
          AND game_state <> 'CANCELLED'
        ORDER BY start_time_utc
        "#,
    )
    .bind(date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Row shape for per-player per-game stats that handlers render.
#[derive(Debug, Clone, FromRow)]
pub struct PlayerGameStatRow {
    pub game_id: i64,
    pub player_id: i64,
    pub team_abbrev: String,
    pub position: String,
    pub name: String,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
    pub sog: Option<i32>,
    pub pim: Option<i32>,
    pub plus_minus: Option<i32>,
    pub hits: Option<i32>,
    pub toi_seconds: Option<i32>,
}

/// Every player row from `nhl_player_game_stats` for the given games.
/// Caller groups by `game_id` in-memory.
pub async fn list_player_game_stats_for_games(
    pool: &PgPool,
    game_ids: &[i64],
) -> Result<Vec<PlayerGameStatRow>> {
    if game_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, PlayerGameStatRow>(
        r#"
        SELECT
            game_id, player_id, team_abbrev, position, name,
            goals, assists, points,
            sog, pim, plus_minus, hits,
            toi_seconds
        FROM nhl_player_game_stats
        WHERE game_id = ANY($1)
        "#,
    )
    .bind(game_ids)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Per-player playoff aggregate from `nhl_player_game_stats`, clamped
/// to the active window. Powers the fantasy-team breakdown page where
/// every rostered skater gets a full box line (G/A/P/SOG/PIM/+/-/HIT)
/// plus average TOI.
#[derive(Debug, FromRow)]
pub struct PlayerPlayoffRollupRow {
    pub player_id: i64,
    pub games: i64,
    pub goals: i64,
    pub assists: i64,
    pub points: i64,
    pub sog: i64,
    pub pim: i64,
    pub plus_minus: i64,
    pub hits: i64,
    pub total_toi_seconds: i64,
}

pub async fn list_player_playoff_rollup(
    pool: &PgPool,
    player_ids: &[i64],
    season: i32,
    window: crate::infra::db::DateWindow<'_>,
) -> Result<Vec<PlayerPlayoffRollupRow>> {
    if player_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, PlayerPlayoffRollupRow>(
        r#"
        SELECT pgs.player_id,
               COUNT(*)::bigint                              AS games,
               COALESCE(SUM(pgs.goals), 0)::bigint           AS goals,
               COALESCE(SUM(pgs.assists), 0)::bigint         AS assists,
               COALESCE(SUM(pgs.points), 0)::bigint          AS points,
               COALESCE(SUM(pgs.sog), 0)::bigint             AS sog,
               COALESCE(SUM(pgs.pim), 0)::bigint             AS pim,
               COALESCE(SUM(pgs.plus_minus), 0)::bigint      AS plus_minus,
               COALESCE(SUM(pgs.hits), 0)::bigint            AS hits,
               COALESCE(SUM(pgs.toi_seconds), 0)::bigint     AS total_toi_seconds
          FROM nhl_player_game_stats pgs
          JOIN nhl_games g ON g.game_id = pgs.game_id
         WHERE pgs.player_id = ANY($1)
           AND g.season      = $2
           AND g.game_type   = 3
           AND g.stats_finalized_at IS NOT NULL
           AND ($3::text IS NULL OR g.game_date::text >= $3)
           AND ($4::text IS NULL OR g.game_date::text <= $4)
         GROUP BY pgs.player_id
        "#,
    )
    .bind(player_ids)
    .bind(season)
    .bind(window.min_date)
    .bind(window.max_date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// One recent playoff game per player, ordered newest-first. Used to
/// power the diagnosis narrator's "26:04 TOI in Game 2" phrasing and
/// the UI's recent-games strip.
#[derive(Debug, FromRow)]
pub struct PlayerRecentGameRow {
    pub player_id: i64,
    pub game_date: chrono::NaiveDate,
    pub opponent: String,
    pub toi_seconds: Option<i32>,
    pub goals: i32,
    pub assists: i32,
    pub points: i32,
    pub sog: Option<i32>,
    pub plus_minus: Option<i32>,
}

pub async fn list_player_recent_games(
    pool: &PgPool,
    player_ids: &[i64],
    season: i32,
    limit_per_player: i32,
) -> Result<Vec<PlayerRecentGameRow>> {
    if player_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, PlayerRecentGameRow>(
        r#"
        WITH ranked AS (
            SELECT pgs.player_id,
                   g.game_date,
                   CASE WHEN g.home_team = pgs.team_abbrev
                        THEN g.away_team ELSE g.home_team END AS opponent,
                   pgs.toi_seconds, pgs.goals, pgs.assists, pgs.points,
                   pgs.sog, pgs.plus_minus,
                   ROW_NUMBER() OVER (
                       PARTITION BY pgs.player_id
                       ORDER BY g.game_date DESC, g.game_id DESC
                   ) AS rn
              FROM nhl_player_game_stats pgs
              JOIN nhl_games g ON g.game_id = pgs.game_id
             WHERE pgs.player_id = ANY($1)
               AND g.season      = $2
               AND g.game_type   = 3
               AND g.stats_finalized_at IS NOT NULL
        )
        SELECT player_id, game_date, opponent,
               toi_seconds, goals, assists, points, sog, plus_minus
          FROM ranked
         WHERE rn <= $3
         ORDER BY player_id, game_date DESC
        "#,
    )
    .bind(player_ids)
    .bind(season)
    .bind(limit_per_player)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Aggregated last-N-games form for a batch of players. Considers
/// only completed games (`game_state IN (OFF, FINAL)`) so in-progress
/// partials don't distort the line.
#[derive(Debug, FromRow)]
pub struct PlayerFormRow {
    pub player_id: i64,
    pub games: i64,
    pub goals: i64,
    pub assists: i64,
    pub points: i64,
    /// Time on ice from the single most recent completed game,
    /// formatted later as `MM:SS` by the handler.
    pub latest_toi_seconds: Option<i32>,
}

pub async fn list_player_form(
    pool: &PgPool,
    player_ids: &[i64],
    num_games: i32,
) -> Result<Vec<PlayerFormRow>> {
    if player_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, PlayerFormRow>(
        r#"
        WITH recent AS (
            SELECT pgs.player_id, pgs.goals, pgs.assists, pgs.points,
                   pgs.toi_seconds, g.game_date,
                   ROW_NUMBER() OVER (
                       PARTITION BY pgs.player_id
                       ORDER BY g.game_date DESC, g.game_id DESC
                   ) AS rn
              FROM nhl_player_game_stats pgs
              JOIN nhl_games g ON g.game_id = pgs.game_id
             WHERE pgs.player_id = ANY($1)
               AND g.stats_finalized_at IS NOT NULL
        )
        SELECT player_id,
               COUNT(*)::bigint                                AS games,
               COALESCE(SUM(goals), 0)::bigint                 AS goals,
               COALESCE(SUM(assists), 0)::bigint               AS assists,
               COALESCE(SUM(points), 0)::bigint                AS points,
               (ARRAY_AGG(toi_seconds ORDER BY game_date DESC))[1] AS latest_toi_seconds
          FROM recent
         WHERE rn <= $2
         GROUP BY player_id
        "#,
    )
    .bind(player_ids)
    .bind(num_games)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Season-to-date playoff totals from `nhl_player_game_stats`, used
/// by the extended-games handler to render the per-player playoff
/// line ("12 GP — 3G 5A 8P"). Restricted to `game_type = 3`.
#[derive(Debug, FromRow)]
pub struct PlayerPlayoffTotalsRow {
    pub player_id: i64,
    pub games: i64,
    pub goals: i64,
    pub assists: i64,
    pub points: i64,
}

pub async fn list_player_playoff_totals(
    pool: &PgPool,
    player_ids: &[i64],
    season: i32,
) -> Result<Vec<PlayerPlayoffTotalsRow>> {
    if player_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, PlayerPlayoffTotalsRow>(
        r#"
        SELECT pgs.player_id,
               COUNT(*)::bigint             AS games,
               COALESCE(SUM(pgs.goals), 0)::bigint   AS goals,
               COALESCE(SUM(pgs.assists), 0)::bigint AS assists,
               COALESCE(SUM(pgs.points), 0)::bigint  AS points
          FROM nhl_player_game_stats pgs
          JOIN nhl_games g ON g.game_id = pgs.game_id
         WHERE pgs.player_id = ANY($1)
           AND g.season      = $2
           AND g.game_type   = 3
           AND g.stats_finalized_at IS NOT NULL
         GROUP BY pgs.player_id
        "#,
    )
    .bind(player_ids)
    .bind(season)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Season-to-date totals per fantasy team in a league, computed by
/// summing every rostered player's `nhl_player_game_stats` rows for
/// completed games in the current season + game_type.
///
/// This replaces the older "NHL skater leaderboard" source used by
/// `get_rankings`. That endpoint only returns the top ~25 skaters
/// per category, so any rostered player outside it contributed 0
/// goals + 0 assists even after scoring — producing overall rankings
/// that silently understated teams whose rostered depth players
/// scored.
///
/// Every team in the league appears in the result (LEFT JOIN), so a
/// team with no rostered appearances still renders with zeros and
/// gets a rank.
#[derive(Debug, FromRow)]
pub struct LeagueTeamSeasonTotalsRow {
    pub team_id: i64,
    pub team_name: String,
    pub goals: i64,
    pub assists: i64,
    pub points: i64,
}

pub async fn list_league_team_season_totals(
    pool: &PgPool,
    league_id: &str,
    season: i32,
    game_type: i16,
    window: crate::infra::db::DateWindow<'_>,
) -> Result<Vec<LeagueTeamSeasonTotalsRow>> {
    // CASE-gated SUMs: the season/game_type predicates cannot live on
    // the LEFT JOIN's ON clause alone. A non-matching nhl_games row
    // would null out `g` but still leave `pgs.points` in scope, so a
    // plain SUM(pgs.points) would include stats from other game types
    // or seasons. Same reasoning for the optional date window.
    //
    // `g.stats_finalized_at IS NOT NULL` excludes games that ended but
    // whose boxscore has not been post-buzzer-synced yet; without it the
    // dashboard would show a "FINAL but slightly stale" total that the
    // live poller's final-sync pass is about to correct, and reload
    // a few minutes later with a different number.
    let rows = sqlx::query_as::<_, LeagueTeamSeasonTotalsRow>(
        r#"
        SELECT
            ft.id   AS team_id,
            ft.name AS team_name,
            COALESCE(SUM(CASE WHEN g.season = $2
                              AND g.game_type = $3
                              AND g.stats_finalized_at IS NOT NULL
                              AND ($4::text IS NULL OR g.game_date::text >= $4)
                              AND ($5::text IS NULL OR g.game_date::text <= $5)
                              THEN pgs.goals   ELSE 0 END), 0)::bigint AS goals,
            COALESCE(SUM(CASE WHEN g.season = $2
                              AND g.game_type = $3
                              AND g.stats_finalized_at IS NOT NULL
                              AND ($4::text IS NULL OR g.game_date::text >= $4)
                              AND ($5::text IS NULL OR g.game_date::text <= $5)
                              THEN pgs.assists ELSE 0 END), 0)::bigint AS assists,
            COALESCE(SUM(CASE WHEN g.season = $2
                              AND g.game_type = $3
                              AND g.stats_finalized_at IS NOT NULL
                              AND ($4::text IS NULL OR g.game_date::text >= $4)
                              AND ($5::text IS NULL OR g.game_date::text <= $5)
                              THEN pgs.points  ELSE 0 END), 0)::bigint AS points
        FROM fantasy_teams ft
        LEFT JOIN fantasy_players fp
               ON fp.team_id = ft.id
        LEFT JOIN nhl_player_game_stats pgs
               ON pgs.player_id = fp.nhl_id
        LEFT JOIN nhl_games g
               ON g.game_id = pgs.game_id
        WHERE ft.league_id = $1::uuid
        GROUP BY ft.id, ft.name
        ORDER BY points DESC, ft.name
        "#,
    )
    .bind(league_id)
    .bind(season)
    .bind(game_type)
    .bind(window.min_date)
    .bind(window.max_date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Per-rostered-player counting-stat aggregate for a single fantasy
/// team. Same finalisation + season/game_type/date-window rules as
/// [`list_league_team_season_totals`] so the team detail page's
/// `Total Points` is a strict refinement of the dashboard row, never a
/// disagreement.
///
/// Returns one row per `(player_nhl_id)` actually rostered on the team,
/// even if they have no finalised games yet (LEFT JOIN), so the caller
/// can render a stable roster list with zeros instead of a shorter list
/// that mysteriously gains a player on first scoring contribution.
#[derive(Debug, FromRow)]
pub struct TeamPlayerSeasonTotalsRow {
    pub nhl_id: i64,
    pub goals: i64,
    pub assists: i64,
    pub points: i64,
}

pub async fn list_team_player_season_totals(
    pool: &PgPool,
    team_id: i64,
    season: i32,
    game_type: i16,
    window: crate::infra::db::DateWindow<'_>,
) -> Result<Vec<TeamPlayerSeasonTotalsRow>> {
    let rows = sqlx::query_as::<_, TeamPlayerSeasonTotalsRow>(
        r#"
        SELECT
            fp.nhl_id AS nhl_id,
            COALESCE(SUM(CASE WHEN g.season = $2
                              AND g.game_type = $3
                              AND g.stats_finalized_at IS NOT NULL
                              AND ($4::text IS NULL OR g.game_date::text >= $4)
                              AND ($5::text IS NULL OR g.game_date::text <= $5)
                              THEN pgs.goals   ELSE 0 END), 0)::bigint AS goals,
            COALESCE(SUM(CASE WHEN g.season = $2
                              AND g.game_type = $3
                              AND g.stats_finalized_at IS NOT NULL
                              AND ($4::text IS NULL OR g.game_date::text >= $4)
                              AND ($5::text IS NULL OR g.game_date::text <= $5)
                              THEN pgs.assists ELSE 0 END), 0)::bigint AS assists,
            COALESCE(SUM(CASE WHEN g.season = $2
                              AND g.game_type = $3
                              AND g.stats_finalized_at IS NOT NULL
                              AND ($4::text IS NULL OR g.game_date::text >= $4)
                              AND ($5::text IS NULL OR g.game_date::text <= $5)
                              THEN pgs.points  ELSE 0 END), 0)::bigint AS points
        FROM fantasy_players fp
        LEFT JOIN nhl_player_game_stats pgs
               ON pgs.player_id = fp.nhl_id
        LEFT JOIN nhl_games g
               ON g.game_id = pgs.game_id
        WHERE fp.team_id = $1
        GROUP BY fp.nhl_id
        "#,
    )
    .bind(team_id)
    .bind(season)
    .bind(game_type)
    .bind(window.min_date)
    .bind(window.max_date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Counting-stat aggregate for an arbitrary set of NHL players. Same
/// finalisation rules as [`list_league_team_season_totals`]. Used by
/// the sleeper-pick handler, which needs totals for a sparse set of
/// players that don't all live on the same fantasy team.
///
/// Players in `nhl_ids` with no finalised games are omitted from the
/// result; the caller is expected to default them to zero, which is
/// the existing behaviour of the leaderboard-based path.
pub async fn list_player_season_totals(
    pool: &PgPool,
    nhl_ids: &[i64],
    season: i32,
    game_type: i16,
    window: crate::infra::db::DateWindow<'_>,
) -> Result<Vec<TeamPlayerSeasonTotalsRow>> {
    if nhl_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, TeamPlayerSeasonTotalsRow>(
        r#"
        SELECT
            pgs.player_id                       AS nhl_id,
            COALESCE(SUM(pgs.goals), 0)::bigint AS goals,
            COALESCE(SUM(pgs.assists), 0)::bigint AS assists,
            COALESCE(SUM(pgs.points), 0)::bigint AS points
        FROM nhl_player_game_stats pgs
        JOIN nhl_games g ON g.game_id = pgs.game_id
        WHERE pgs.player_id = ANY($1)
          AND g.season = $2
          AND g.game_type = $3
          AND g.stats_finalized_at IS NOT NULL
          AND ($4::text IS NULL OR g.game_date::text >= $4)
          AND ($5::text IS NULL OR g.game_date::text <= $5)
        GROUP BY pgs.player_id
        "#,
    )
    .bind(nhl_ids)
    .bind(season)
    .bind(game_type)
    .bind(window.min_date)
    .bind(window.max_date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Deserialise the most recent playoff bracket JSONB for `season`
/// into the typed `PlayoffCarousel` shape. Returns `None` if the
/// bracket hasn't been captured yet.
pub async fn get_playoff_carousel(
    pool: &PgPool,
    season: i32,
) -> Result<Option<crate::domain::models::nhl::PlayoffCarousel>> {
    let raw: Option<Value> =
        sqlx::query_scalar("SELECT carousel FROM nhl_playoff_bracket WHERE season = $1")
            .bind(season)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;
    let Some(v) = raw else { return Ok(None) };
    let c: crate::domain::models::nhl::PlayoffCarousel =
        serde_json::from_value(v).context("carousel decode")?;
    Ok(Some(c))
}

/// League IDs whose rostered players appear in `game_id`. Used by
/// the live poller to target narrative-cache invalidation at just
/// the leagues whose Pulse would change when this game ends.
pub async fn list_leagues_with_player_in_game(pool: &PgPool, game_id: i64) -> Result<Vec<String>> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT ft.league_id::text
          FROM nhl_player_game_stats pgs
          JOIN fantasy_players fp ON fp.nhl_id = pgs.player_id
          JOIN fantasy_teams  ft ON ft.id = fp.team_id
         WHERE pgs.game_id = $1
        "#,
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

// ---------------------------------------------------------------------
// Freshness checks — used by the meta poller to skip redundant NHL
// fetches when the mirror tables were updated recently enough.
// Each helper returns the most recent `updated_at` in the relevant
// mirror table, or `None` if the table is empty (which callers
// treat as "always stale → always fetch").
// ---------------------------------------------------------------------

pub async fn last_update_nhl_games_for_date(
    pool: &PgPool,
    date: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let v = sqlx::query_scalar("SELECT MAX(updated_at) FROM nhl_games WHERE game_date = $1::date")
        .bind(date)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(v)
}

pub async fn last_update_nhl_skater_season_stats(
    pool: &PgPool,
    season: i32,
    game_type: i16,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let v = sqlx::query_scalar(
        "SELECT MAX(updated_at) FROM nhl_skater_season_stats WHERE season = $1 AND game_type = $2",
    )
    .bind(season)
    .bind(game_type)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;
    Ok(v)
}

pub async fn last_update_nhl_goalie_season_stats(
    pool: &PgPool,
    season: i32,
    game_type: i16,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let v = sqlx::query_scalar(
        "SELECT MAX(updated_at) FROM nhl_goalie_season_stats WHERE season = $1 AND game_type = $2",
    )
    .bind(season)
    .bind(game_type)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;
    Ok(v)
}

pub async fn last_update_nhl_standings(
    pool: &PgPool,
    season: i32,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let v = sqlx::query_scalar("SELECT MAX(updated_at) FROM nhl_standings WHERE season = $1")
        .bind(season)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(v)
}

pub async fn last_update_nhl_playoff_bracket(
    pool: &PgPool,
    season: i32,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let v = sqlx::query_scalar("SELECT MAX(updated_at) FROM nhl_playoff_bracket WHERE season = $1")
        .bind(season)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(v)
}

pub async fn last_update_nhl_team_rosters(
    pool: &PgPool,
    season: i32,
) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    let v = sqlx::query_scalar("SELECT MAX(updated_at) FROM nhl_team_rosters WHERE season = $1")
        .bind(season)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;
    Ok(v)
}

/// True if `last` is `None` (no data at all) or older than `max_age`.
pub fn is_stale(last: Option<chrono::DateTime<chrono::Utc>>, max_age: std::time::Duration) -> bool {
    match last {
        None => true,
        Some(t) => {
            let age = (chrono::Utc::now() - t).to_std().unwrap_or_default();
            age >= max_age
        }
    }
}

/// Sum points per player from `nhl_player_game_stats` for the given
/// (season, game_type) and date window. Zero-point players are omitted,
/// so callers treat a missing key as 0. Counts every scorer, unlike the
/// NHL stats-leaders endpoint's top 25 per category.
///
/// Same rule as every other fantasy aggregate (Rankings included): only
/// games whose boxscore has been post-buzzer-synced
/// (`stats_finalized_at IS NOT NULL`) count, so Race Odds "Current" and
/// Rankings never disagree mid-evening.
pub async fn sum_player_points(
    pool: &PgPool,
    player_ids: &[i64],
    season: i32,
    game_type: i16,
    window: crate::infra::db::DateWindow<'_>,
) -> Result<std::collections::HashMap<i64, i32>> {
    if player_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT pgs.player_id, COALESCE(SUM(pgs.points), 0)::bigint AS points
          FROM nhl_player_game_stats pgs
          JOIN nhl_games g ON g.game_id = pgs.game_id
         WHERE pgs.player_id = ANY($1)
           AND g.season      = $2
           AND g.game_type   = $3
           AND g.stats_finalized_at IS NOT NULL
           AND ($4::date IS NULL OR g.game_date >= $4::date)
           AND ($5::date IS NULL OR g.game_date <= $5::date)
         GROUP BY pgs.player_id
        "#,
    )
    .bind(player_ids)
    .bind(season)
    .bind(game_type)
    .bind(window.min_date)
    .bind(window.max_date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows.into_iter().map(|(id, p)| (id, p as i32)).collect())
}

/// One leaderboard row for every skater who has appeared in the current
/// `(season, game_type)`, with totals summed across their boxscore rows
/// and TOI expressed as seconds per game. Metadata comes from the most
/// recent game appearance so mid-season trades resolve to the current team.
#[derive(Debug, FromRow)]
pub struct TopSkaterRow {
    pub player_id: i64,
    pub name: String,
    pub position: String,
    pub team_abbrev: String,
    pub goals: i64,
    pub assists: i64,
    pub points: i64,
    pub pim: i64,
    pub plus_minus: i64,
    pub toi: Option<i64>,
}

pub async fn list_top_skaters(
    pool: &PgPool,
    season: i32,
    game_type: i16,
    limit: i64,
) -> Result<Vec<TopSkaterRow>> {
    let rows = sqlx::query_as::<_, TopSkaterRow>(
        r#"
        WITH agg AS (
            SELECT pgs.player_id,
                   COALESCE(SUM(pgs.goals),   0)::bigint AS goals,
                   COALESCE(SUM(pgs.assists), 0)::bigint AS assists,
                   COALESCE(SUM(pgs.points),  0)::bigint AS points,
                   COALESCE(SUM(pgs.pim),     0)::bigint AS pim,
                   COALESCE(SUM(pgs.plus_minus), 0)::bigint AS plus_minus,
                   ROUND(AVG(pgs.toi_seconds))::bigint AS toi,
                   (ARRAY_AGG(pgs.name        ORDER BY pgs.updated_at DESC))[1] AS name,
                   (ARRAY_AGG(pgs.position    ORDER BY pgs.updated_at DESC))[1] AS position,
                   (ARRAY_AGG(pgs.team_abbrev ORDER BY pgs.updated_at DESC))[1] AS team_abbrev
              FROM nhl_player_game_stats pgs
              JOIN nhl_games g ON g.game_id = pgs.game_id
             WHERE g.season    = $1
               AND g.game_type = $2
               AND g.stats_finalized_at IS NOT NULL
             GROUP BY pgs.player_id
        )
        SELECT
            player_id, name, position, team_abbrev,
            goals, assists, points, pim, plus_minus, toi
          FROM agg
         WHERE position <> 'G'
         ORDER BY points DESC, goals DESC, player_id
         LIMIT $3
        "#,
    )
    .bind(season)
    .bind(game_type)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}
