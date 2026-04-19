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

use serde_json::Value;
use sqlx::{PgConnection, PgPool};

use crate::domain::models::nhl::{BoxscorePlayer, GameBoxscore, Player, StatsLeaders, TodayGame};
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

// PgPool is re-exported here so poller call sites don't have to
// import `sqlx::PgPool` directly just to type their signatures.
pub use sqlx::PgPool as Pool;

// ---------------------------------------------------------------------
// nhl_games
// ---------------------------------------------------------------------

/// Upsert a game row from a schedule payload. The live poller calls
/// [`update_game_live_state`] for mid-game score/period updates; this
/// function is the full-row writer used by the meta poller and the
/// rehydrate admin endpoint.
pub async fn upsert_game(pool: &PgPool, game: &TodayGame, game_date: &str) -> Result<()> {
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
        None => (None, None),
    };
    // GameState has a Display impl via serde; round-trip through
    // serde_json to get the canonical upstream spelling.
    let game_state = serde_json::to_value(&game.game_state)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "FUT".into());

    sqlx::query(
        r#"
        INSERT INTO nhl_games (
            game_id, season, game_type, game_date, start_time_utc, game_state,
            home_team, away_team, home_score, away_score,
            period_number, period_type, series_status, venue, updated_at
        )
        VALUES ($1, $2, $3, $4::date, $5::timestamptz, $6,
                $7, $8, $9, $10, $11, $12, $13, $14, NOW())
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
            updated_at = NOW()
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
    .execute(pool)
    .await
    .map_err(Error::Database)?;
    Ok(())
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
        "#,
    )
    .bind(date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// All game IDs on `date`, regardless of state. Used by rehydrate to
/// rebuild `nhl_player_game_stats` from completed games.
pub async fn list_all_game_ids_for_date(pool: &PgPool, date: &str) -> Result<Vec<i64>> {
    let rows: Vec<i64> = sqlx::query_scalar(
        "SELECT game_id FROM nhl_games WHERE game_date = $1::date",
    )
    .bind(date)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)?;
    Ok(rows)
}

/// Current state of a game, for transition detection.
pub async fn get_game_state(pool: &PgPool, game_id: i64) -> Result<Option<String>> {
    let state: Option<String> =
        sqlx::query_scalar("SELECT game_state FROM nhl_games WHERE game_id = $1")
            .bind(game_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;
    Ok(state)
}

// ---------------------------------------------------------------------
// nhl_player_game_stats
// ---------------------------------------------------------------------

/// Replace every `nhl_player_game_stats` row for `game_id` from the
/// boxscore. The boxscore is the full set of skaters + goalies from
/// both teams; we upsert each one by `(game_id, player_id)`.
///
/// Returns the list of `player_id` values that were written, so the
/// caller can log coverage.
pub async fn upsert_boxscore_players(
    pool: &PgPool,
    game_id: i64,
    home_abbrev: &str,
    away_abbrev: &str,
    boxscore: &GameBoxscore,
) -> Result<usize> {
    let home = &boxscore.player_by_game_stats.home_team;
    let away = &boxscore.player_by_game_stats.away_team;

    let iter_home = home
        .forwards
        .iter()
        .chain(home.defense.iter())
        .chain(home.goalies.iter())
        .map(|p| (home_abbrev, p));
    let iter_away = away
        .forwards
        .iter()
        .chain(away.defense.iter())
        .chain(away.goalies.iter())
        .map(|p| (away_abbrev, p));

    let mut count = 0;
    let mut tx = pool.begin().await.map_err(Error::Database)?;
    for (team_abbrev, p) in iter_home.chain(iter_away) {
        upsert_boxscore_player(&mut tx, game_id, team_abbrev, p).await?;
        count += 1;
    }
    tx.commit().await.map_err(Error::Database)?;
    Ok(count)
}

async fn upsert_boxscore_player(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    game_id: i64,
    team_abbrev: &str,
    p: &BoxscorePlayer,
) -> Result<()> {
    let name = p
        .name
        .get("default")
        .cloned()
        .unwrap_or_else(|| format!("Player {}", p.player_id));
    let points = p
        .points
        .unwrap_or_else(|| p.goals.unwrap_or(0) + p.assists.unwrap_or(0));

    sqlx::query(
        r#"
        INSERT INTO nhl_player_game_stats (
            game_id, player_id, team_abbrev, position, name,
            goals, assists, points, sog, pim, plus_minus, hits, toi_seconds, updated_at
        )
        VALUES ($1, $2, $3, $4, $5,
                $6, $7, $8, $9, $10, $11, $12, NULL, NOW())
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
            updated_at = NOW()
        "#,
    )
    .bind(game_id)
    .bind(p.player_id as i64)
    .bind(team_abbrev)
    .bind(&p.position)
    .bind(&name)
    .bind(p.goals.unwrap_or(0))
    .bind(p.assists.unwrap_or(0))
    .bind(points)
    .bind(p.sog)
    .bind(p.pim)
    .bind(p.plus_minus)
    .bind(p.hits)
    .execute(&mut **tx)
    .await
    .map_err(Error::Database)?;
    Ok(())
}

// ---------------------------------------------------------------------
// nhl_skater_season_stats
// ---------------------------------------------------------------------

/// Materialize the skater leaderboard response into
/// `nhl_skater_season_stats`. The NHL response groups players by
/// category (goals, assists, points, plus_minus, faceoff, toi, ...);
/// we flatten into a single row per `(player_id, season, game_type)`
/// where each category's metric is stored in its canonical column,
/// using the *points* list's `value` for `points`, the *goals* list's
/// `value` for `goals`, and so on.
pub async fn upsert_skater_leaderboard(
    pool: &PgPool,
    season: i32,
    game_type: i16,
    leaders: &StatsLeaders,
) -> Result<usize> {
    use std::collections::HashMap;

    // Aggregate: player_id → (first_name, last_name, team, position, goals, assists, points, plus_minus, faceoff_pct, toi, sog)
    struct Row {
        first_name: String,
        last_name: String,
        team: String,
        position: String,
        goals: i32,
        assists: i32,
        points: i32,
        plus_minus: Option<i32>,
        faceoff_pct: Option<f32>,
        toi_per_game: Option<i32>,
        sog: Option<i32>,
    }
    let mut map: HashMap<i64, Row> = HashMap::new();

    let seed = |map: &mut HashMap<i64, Row>, p: &Player| {
        map.entry(p.id as i64).or_insert_with(|| Row {
            first_name: p
                .first_name
                .get("default")
                .cloned()
                .unwrap_or_default(),
            last_name: p
                .last_name
                .get("default")
                .cloned()
                .unwrap_or_default(),
            team: p.team_abbrev.clone(),
            position: p.position.clone(),
            goals: 0,
            assists: 0,
            points: 0,
            plus_minus: None,
            faceoff_pct: None,
            toi_per_game: None,
            sog: None,
        });
    };

    for p in &leaders.goals {
        seed(&mut map, p);
        if let Some(r) = map.get_mut(&(p.id as i64)) {
            r.goals = p.value as i32;
        }
    }
    for p in &leaders.assists {
        seed(&mut map, p);
        if let Some(r) = map.get_mut(&(p.id as i64)) {
            r.assists = p.value as i32;
        }
    }
    for p in &leaders.points {
        seed(&mut map, p);
        if let Some(r) = map.get_mut(&(p.id as i64)) {
            r.points = p.value as i32;
        }
    }
    for p in &leaders.plus_minus {
        seed(&mut map, p);
        if let Some(r) = map.get_mut(&(p.id as i64)) {
            r.plus_minus = Some(p.value as i32);
        }
    }
    for p in &leaders.faceoff_leaders {
        seed(&mut map, p);
        if let Some(r) = map.get_mut(&(p.id as i64)) {
            r.faceoff_pct = Some(p.value as f32);
        }
    }
    for p in &leaders.toi {
        seed(&mut map, p);
        if let Some(r) = map.get_mut(&(p.id as i64)) {
            // TOI comes as seconds-per-game.
            r.toi_per_game = Some(p.value as i32);
        }
    }

    let mut tx = pool.begin().await.map_err(Error::Database)?;
    let mut count = 0;
    for (player_id, row) in &map {
        let headshot_url =
            format!("https://assets.nhle.com/mugs/nhl/{}/{}/{}.png",
                season, row.team, player_id);
        sqlx::query(
            r#"
            INSERT INTO nhl_skater_season_stats (
                player_id, season, game_type, first_name, last_name,
                team_abbrev, position, goals, assists, points,
                plus_minus, faceoff_pct, toi_per_game, sog, headshot_url, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, $13, $14, $15, NOW())
            ON CONFLICT (player_id, season, game_type) DO UPDATE SET
                first_name = EXCLUDED.first_name,
                last_name = EXCLUDED.last_name,
                team_abbrev = EXCLUDED.team_abbrev,
                position = EXCLUDED.position,
                goals = EXCLUDED.goals,
                assists = EXCLUDED.assists,
                points = EXCLUDED.points,
                plus_minus = COALESCE(EXCLUDED.plus_minus, nhl_skater_season_stats.plus_minus),
                faceoff_pct = COALESCE(EXCLUDED.faceoff_pct, nhl_skater_season_stats.faceoff_pct),
                toi_per_game = COALESCE(EXCLUDED.toi_per_game, nhl_skater_season_stats.toi_per_game),
                sog = COALESCE(EXCLUDED.sog, nhl_skater_season_stats.sog),
                headshot_url = EXCLUDED.headshot_url,
                updated_at = NOW()
            "#,
        )
        .bind(player_id)
        .bind(season)
        .bind(game_type)
        .bind(&row.first_name)
        .bind(&row.last_name)
        .bind(&row.team)
        .bind(&row.position)
        .bind(row.goals)
        .bind(row.assists)
        .bind(row.points)
        .bind(row.plus_minus)
        .bind(row.faceoff_pct)
        .bind(row.toi_per_game)
        .bind(row.sog)
        .bind(&headshot_url)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;
        count += 1;
    }
    tx.commit().await.map_err(Error::Database)?;
    Ok(count)
}

// ---------------------------------------------------------------------
// nhl_goalie_season_stats (from raw goalie-stats-leaders payload)
// ---------------------------------------------------------------------

/// Upsert the goalie leaderboard. The payload shape is a loose
/// JSON; we consume the fields we care about by path and fall back
/// to defaults for missing ones.
pub async fn upsert_goalie_leaderboard(
    pool: &PgPool,
    season: i32,
    game_type: i16,
    payload: &Value,
) -> Result<usize> {
    use std::collections::HashMap;
    struct Row {
        team: String,
        name: String,
        gaa: Option<f32>,
        save_pctg: Option<f32>,
        shutouts: Option<i32>,
    }
    let mut map: HashMap<i64, Row> = HashMap::new();

    let seed_from = |map: &mut HashMap<i64, Row>, list_name: &str| {
        if let Some(list) = payload.get(list_name).and_then(Value::as_array) {
            for p in list {
                let Some(id) = p.get("id").and_then(Value::as_i64) else {
                    continue;
                };
                let first = p
                    .get("firstName")
                    .and_then(|v| v.get("default"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let last = p
                    .get("lastName")
                    .and_then(|v| v.get("default"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let team = p
                    .get("teamAbbrev")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let name = format!("{} {}", first, last).trim().to_string();
                map.entry(id).or_insert_with(|| Row {
                    team,
                    name,
                    gaa: None,
                    save_pctg: None,
                    shutouts: None,
                });
            }
        }
    };

    seed_from(&mut map, "wins");
    seed_from(&mut map, "savePctg");
    seed_from(&mut map, "goalsAgainstAverage");
    seed_from(&mut map, "shutouts");

    if let Some(list) = payload.get("goalsAgainstAverage").and_then(Value::as_array) {
        for p in list {
            if let (Some(id), Some(v)) = (
                p.get("id").and_then(Value::as_i64),
                p.get("value").and_then(Value::as_f64),
            ) {
                if let Some(row) = map.get_mut(&id) {
                    row.gaa = Some(v as f32);
                }
            }
        }
    }
    if let Some(list) = payload.get("savePctg").and_then(Value::as_array) {
        for p in list {
            if let (Some(id), Some(v)) = (
                p.get("id").and_then(Value::as_i64),
                p.get("value").and_then(Value::as_f64),
            ) {
                if let Some(row) = map.get_mut(&id) {
                    row.save_pctg = Some(v as f32);
                }
            }
        }
    }
    if let Some(list) = payload.get("shutouts").and_then(Value::as_array) {
        for p in list {
            if let (Some(id), Some(v)) = (
                p.get("id").and_then(Value::as_i64),
                p.get("value").and_then(Value::as_f64),
            ) {
                if let Some(row) = map.get_mut(&id) {
                    row.shutouts = Some(v as i32);
                }
            }
        }
    }

    let mut tx = pool.begin().await.map_err(Error::Database)?;
    let mut count = 0;
    for (player_id, row) in &map {
        sqlx::query(
            r#"
            INSERT INTO nhl_goalie_season_stats (
                player_id, season, game_type, team_abbrev, name,
                record, gaa, save_pctg, shutouts, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, NULL, $6, $7, $8, NOW())
            ON CONFLICT (player_id, season, game_type) DO UPDATE SET
                team_abbrev = EXCLUDED.team_abbrev,
                name = EXCLUDED.name,
                gaa = COALESCE(EXCLUDED.gaa, nhl_goalie_season_stats.gaa),
                save_pctg = COALESCE(EXCLUDED.save_pctg, nhl_goalie_season_stats.save_pctg),
                shutouts = COALESCE(EXCLUDED.shutouts, nhl_goalie_season_stats.shutouts),
                updated_at = NOW()
            "#,
        )
        .bind(player_id)
        .bind(season)
        .bind(game_type)
        .bind(&row.team)
        .bind(&row.name)
        .bind(row.gaa)
        .bind(row.save_pctg)
        .bind(row.shutouts)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;
        count += 1;
    }
    tx.commit().await.map_err(Error::Database)?;
    Ok(count)
}

// ---------------------------------------------------------------------
// nhl_standings
// ---------------------------------------------------------------------

pub async fn upsert_standings(pool: &PgPool, season: i32, payload: &Value) -> Result<usize> {
    let rows = payload
        .get("standings")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::NhlApi("standings payload missing 'standings' array".into()))?;

    let mut tx = pool.begin().await.map_err(Error::Database)?;
    let mut count = 0;
    for row in rows {
        let team = row
            .get("teamAbbrev")
            .and_then(|v| v.get("default"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if team.is_empty() {
            continue;
        }
        let points = row.get("points").and_then(Value::as_i64).unwrap_or(0) as i32;
        let gp = row.get("gamesPlayed").and_then(Value::as_i64).unwrap_or(0) as i32;
        let wins = row.get("wins").and_then(Value::as_i64).unwrap_or(0) as i32;
        let losses = row.get("losses").and_then(Value::as_i64).unwrap_or(0) as i32;
        let otl = row.get("otLosses").and_then(Value::as_i64).unwrap_or(0) as i32;
        let pct = row.get("pointPctg").and_then(Value::as_f64).map(|v| v as f32);
        let streak_code = row.get("streakCode").and_then(Value::as_str).map(String::from);
        let streak_count = row.get("streakCount").and_then(Value::as_i64).map(|v| v as i32);
        let l10_w = row.get("l10Wins").and_then(Value::as_i64).map(|v| v as i32);
        let l10_l = row.get("l10Losses").and_then(Value::as_i64).map(|v| v as i32);
        let l10_otl = row.get("l10OtLosses").and_then(Value::as_i64).map(|v| v as i32);

        sqlx::query(
            r#"
            INSERT INTO nhl_standings (
                season, team_abbrev, points, games_played, wins, losses, ot_losses,
                point_pctg, streak_code, streak_count, l10_wins, l10_losses, l10_ot_losses,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW())
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
                updated_at = NOW()
            "#,
        )
        .bind(season)
        .bind(&team)
        .bind(points)
        .bind(gp)
        .bind(wins)
        .bind(losses)
        .bind(otl)
        .bind(pct)
        .bind(streak_code)
        .bind(streak_count)
        .bind(l10_w)
        .bind(l10_l)
        .bind(l10_otl)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;
        count += 1;
    }
    tx.commit().await.map_err(Error::Database)?;
    Ok(count)
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
    let json = serde_json::to_value(roster).unwrap_or(Value::Null);
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

// ---------------------------------------------------------------------
// nhl_playoff_bracket
// ---------------------------------------------------------------------

pub async fn upsert_playoff_bracket(pool: &PgPool, season: i32, carousel: &Value) -> Result<()> {
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
    .bind(carousel)
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
pub async fn capture_game_landing(
    pool: &PgPool,
    game_id: i64,
    matchup: &Value,
) -> Result<bool> {
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
            goals, assists, points, toi_seconds
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
               AND g.game_state IN ('OFF', 'FINAL')
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
           AND g.game_state IN ('OFF', 'FINAL')
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
) -> Result<Vec<LeagueTeamSeasonTotalsRow>> {
    let rows = sqlx::query_as::<_, LeagueTeamSeasonTotalsRow>(
        r#"
        SELECT
            ft.id                          AS team_id,
            ft.name                        AS team_name,
            COALESCE(SUM(pgs.goals),   0)::bigint AS goals,
            COALESCE(SUM(pgs.assists), 0)::bigint AS assists,
            COALESCE(SUM(pgs.points),  0)::bigint AS points
        FROM fantasy_teams ft
        LEFT JOIN fantasy_players fp
               ON fp.team_id = ft.id
        LEFT JOIN nhl_player_game_stats pgs
               ON pgs.player_id = fp.nhl_id
        LEFT JOIN nhl_games g
               ON g.game_id   = pgs.game_id
              AND g.season    = $2
              AND g.game_type = $3
        WHERE ft.league_id = $1::uuid
        GROUP BY ft.id, ft.name
        ORDER BY points DESC, ft.name
        "#,
    )
    .bind(league_id)
    .bind(season)
    .bind(game_type)
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
    let raw: Option<Value> = sqlx::query_scalar(
        "SELECT carousel FROM nhl_playoff_bracket WHERE season = $1",
    )
    .bind(season)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;
    let Some(v) = raw else { return Ok(None) };
    let c: crate::domain::models::nhl::PlayoffCarousel =
        serde_json::from_value(v).map_err(|e| Error::Internal(format!("carousel decode: {e}")))?;
    Ok(Some(c))
}

/// League IDs whose rostered players appear in `game_id`. Used by
/// the live poller to target narrative-cache invalidation at just
/// the leagues whose Pulse would change when this game ends.
pub async fn list_leagues_with_player_in_game(
    pool: &PgPool,
    game_id: i64,
) -> Result<Vec<String>> {
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
