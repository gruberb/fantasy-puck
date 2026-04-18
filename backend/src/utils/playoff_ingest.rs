//! Ingest completed playoff box scores into `playoff_skater_game_stats`.
//!
//! Two entry points:
//! - [`ingest_playoff_games_for_date`] — pulls the NHL schedule for a
//!   single date, filters to completed playoff games, and upserts one row
//!   per skater per game. Called by the 10am UTC scheduler job for
//!   yesterday's date.
//! - [`ingest_playoff_games_for_range`] — loops [`ingest_playoff_games_for_date`]
//!   across a date range. Used for the startup backfill.
//!
//! Idempotent: the upsert refreshes existing rows, so running the same
//! date twice does not duplicate data. Goalies are skipped (skater-only
//! fantasy format).

use std::sync::Arc;

use chrono::NaiveDate;
use tracing::{debug, info, warn};

use crate::db::FantasyDb;
use crate::error::Result;
use crate::models::nhl::{GameState, TodayGame, TodaySchedule};
use crate::NhlClient;

/// Ingest every completed playoff game on `date` (YYYY-MM-DD). Returns the
/// number of skater rows upserted.
pub async fn ingest_playoff_games_for_date(
    db: &FantasyDb,
    nhl: &Arc<NhlClient>,
    date: &str,
) -> Result<usize> {
    let schedule: TodaySchedule = match nhl.get_schedule_by_date(date).await {
        Ok(s) => s,
        Err(e) => {
            warn!(%date, error = %e, "playoff ingest: schedule fetch failed; skipping date");
            return Ok(0);
        }
    };
    let games = schedule.games_for_date(date);

    let mut total_rows = 0usize;
    for game in games {
        // Only playoff games; only completed. `game_type == 3` is
        // playoffs — everything else (regular season, preseason, special
        // events) is out of scope for this table.
        if game.game_type != 3 || !game.game_state.is_completed() {
            continue;
        }
        match ingest_single_game(db, nhl, date, &game).await {
            Ok(n) => {
                total_rows += n;
                debug!(
                    game_id = game.id,
                    rows = n,
                    date = %date,
                    "playoff ingest: game upserted"
                );
            }
            Err(e) => {
                warn!(
                    game_id = game.id,
                    date = %date,
                    error = %e,
                    "playoff ingest: single game failed; continuing"
                );
            }
        }
    }
    info!(
        date = %date,
        games = games_for_logging(&schedule, date),
        rows = total_rows,
        "playoff ingest: date complete"
    );
    Ok(total_rows)
}

/// Ingest every completed playoff game between `start` and `end` inclusive.
/// Errors on individual days are logged and skipped so one bad day doesn't
/// abort the whole backfill.
pub async fn ingest_playoff_games_for_range(
    db: &FantasyDb,
    nhl: &Arc<NhlClient>,
    start: &str,
    end: &str,
) -> Result<usize> {
    let Ok(start_d) = NaiveDate::parse_from_str(start, "%Y-%m-%d") else {
        warn!(%start, "playoff ingest: invalid start date");
        return Ok(0);
    };
    let Ok(end_d) = NaiveDate::parse_from_str(end, "%Y-%m-%d") else {
        warn!(%end, "playoff ingest: invalid end date");
        return Ok(0);
    };
    if end_d < start_d {
        return Ok(0);
    }

    let mut total = 0usize;
    let mut cursor = start_d;
    loop {
        let date_str = cursor.format("%Y-%m-%d").to_string();
        total += ingest_playoff_games_for_date(db, nhl, &date_str).await?;
        if cursor >= end_d {
            break;
        }
        match cursor.succ_opt() {
            Some(next) => cursor = next,
            None => break,
        }
    }
    Ok(total)
}

/// True when the table is empty and a backfill is warranted.
pub async fn is_playoff_skater_game_stats_empty(db: &FantasyDb) -> Result<bool> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playoff_skater_game_stats")
        .fetch_one(db.pool())
        .await
        .map_err(crate::error::Error::Database)?;
    Ok(count == 0)
}

/// Re-backfill a season's `playoff_game_results` via the playoff-series
/// endpoint instead of date-by-date schedule iteration. For each series
/// in the carousel we fetch the full games list (which reliably contains
/// every game's ID, home/away, and score even for historical seasons)
/// and upsert team-level rows.
///
/// Skater-level stats (`playoff_skater_game_stats`) are not populated
/// here — this path is team-level only and aimed at fixing the
/// calibration ground truth. Running this on a season that already has
/// data is safe (upserts are idempotent).
///
/// `season` is the 8-digit season string (e.g. `20222023`), matching the
/// shape `playoff_game_results.season` stores. `short_year` is the
/// 4-digit calendar year of the playoff end, which the series-games
/// endpoint expects in its URL (e.g. `2023` for the 20222023 playoffs).
pub async fn rebackfill_playoff_season_via_carousel(
    db: &FantasyDb,
    nhl: &Arc<NhlClient>,
    season: u32,
    short_year: u32,
) -> Result<usize> {
    let carousel = nhl
        .get_playoff_carousel(season.to_string())
        .await?
        .ok_or_else(|| crate::error::Error::Validation(format!(
            "No playoff carousel for season {season}"
        )))?;

    let mut total: usize = 0;
    for round in &carousel.rounds {
        for series in &round.series {
            let games = match nhl
                .get_playoff_series_games(short_year, &series.series_letter)
                .await
            {
                Ok(g) => g,
                Err(e) => {
                    warn!(
                        season,
                        letter = %series.series_letter,
                        error = %e,
                        "series-games fetch failed; skipping series"
                    );
                    continue;
                }
            };
            for game in games.games {
                if !game.game_state.is_completed() {
                    continue;
                }
                let (Some(home_score), Some(away_score)) =
                    (game.home_team.score, game.away_team.score)
                else {
                    continue;
                };
                let Some(ref start) = game.start_time_utc else {
                    continue;
                };
                // Derive game_date from the start-time ISO string
                // (YYYY-MM-DDThh:mm:ssZ).
                let game_date = &start[..10];
                let winner = if home_score > away_score {
                    &game.home_team.abbrev
                } else {
                    &game.away_team.abbrev
                };
                let round_i16 = round.round_number as i16;

                sqlx::query(
                    r#"
                    INSERT INTO playoff_game_results (
                        season, game_type, game_id, game_date,
                        home_team, away_team, home_score, away_score, winner, round
                    )
                    VALUES ($1, 3, $2, $3::date, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT (game_id) DO UPDATE SET
                        game_date  = EXCLUDED.game_date,
                        home_team  = EXCLUDED.home_team,
                        away_team  = EXCLUDED.away_team,
                        home_score = EXCLUDED.home_score,
                        away_score = EXCLUDED.away_score,
                        winner     = EXCLUDED.winner,
                        round      = EXCLUDED.round
                    "#,
                )
                .bind(season as i32)
                .bind(game.id as i64)
                .bind(game_date)
                .bind(&game.home_team.abbrev)
                .bind(&game.away_team.abbrev)
                .bind(home_score)
                .bind(away_score)
                .bind(winner)
                .bind(round_i16)
                .execute(db.pool())
                .await
                .map_err(crate::error::Error::Database)?;

                total += 1;
            }
        }
    }
    info!(season, rows = total, "carousel-driven backfill complete");
    Ok(total)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

async fn ingest_single_game(
    db: &FantasyDb,
    nhl: &Arc<NhlClient>,
    date: &str,
    game: &TodayGame,
) -> Result<usize> {
    let box_score = nhl.get_game_boxscore(game.id).await?;
    let season_i32 = game.season as i32;
    let game_type_i16 = game.game_type as i16;
    let game_id_i64 = game.id as i64;
    // Pass date through as text and cast to DATE in SQL — sqlx in this
    // crate is compiled without the chrono feature, so binding
    // `NaiveDate` arrays directly isn't available.
    let game_date_str = date.to_string();
    // Fail fast if the caller handed us something that isn't a valid date.
    NaiveDate::parse_from_str(&game_date_str, "%Y-%m-%d")
        .map_err(|e| crate::error::Error::Internal(format!("bad date {}: {}", date, e)))?;
    let away_abbrev = game.away_team.abbrev.clone();
    let home_abbrev = game.home_team.abbrev.clone();

    // Upsert the team-level result row (playoff Elo replays off this).
    // Prefer the schedule's reported score; fall back to summing the
    // boxscore's player goals if the score is missing.
    let (home_score, away_score) = match &game.game_score {
        Some(s) => (s.home, s.away),
        None => {
            let sum_goals = |team: &crate::models::nhl::TeamGameStats| -> i32 {
                team.forwards.iter().chain(team.defense.iter())
                    .map(|p| p.goals.unwrap_or(0))
                    .sum()
            };
            (
                sum_goals(&box_score.player_by_game_stats.home_team),
                sum_goals(&box_score.player_by_game_stats.away_team),
            )
        }
    };
    let winner = if home_score > away_score {
        home_abbrev.clone()
    } else {
        away_abbrev.clone()
    };
    let round_i16: Option<i16> = game.series_status.as_ref().map(|s| s.round as i16);
    sqlx::query(
        r#"
        INSERT INTO playoff_game_results (
            season, game_type, game_id, game_date,
            home_team, away_team, home_score, away_score, winner, round
        )
        VALUES ($1, $2, $3, $4::date, $5, $6, $7, $8, $9, $10)
        ON CONFLICT (game_id) DO UPDATE SET
            home_score = EXCLUDED.home_score,
            away_score = EXCLUDED.away_score,
            winner     = EXCLUDED.winner,
            round      = EXCLUDED.round
        "#,
    )
    .bind(season_i32)
    .bind(game_type_i16)
    .bind(game_id_i64)
    .bind(&game_date_str)
    .bind(&home_abbrev)
    .bind(&away_abbrev)
    .bind(home_score)
    .bind(away_score)
    .bind(&winner)
    .bind(round_i16)
    .execute(db.pool())
    .await
    .map_err(crate::error::Error::Database)?;

    let mut rows: Vec<SkaterRow> = Vec::with_capacity(40);
    for p in &box_score.player_by_game_stats.away_team.forwards {
        rows.push(boxscore_to_row(p, &away_abbrev, &home_abbrev, false));
    }
    for p in &box_score.player_by_game_stats.away_team.defense {
        rows.push(boxscore_to_row(p, &away_abbrev, &home_abbrev, false));
    }
    for p in &box_score.player_by_game_stats.home_team.forwards {
        rows.push(boxscore_to_row(p, &home_abbrev, &away_abbrev, true));
    }
    for p in &box_score.player_by_game_stats.home_team.defense {
        rows.push(boxscore_to_row(p, &home_abbrev, &away_abbrev, true));
    }

    if rows.is_empty() {
        return Ok(0);
    }

    // Bulk UPSERT via UNNEST. Primary key is (game_id, player_id), so a
    // re-ingest refreshes existing rows — useful if a boxscore updated
    // after first fetch (NHL occasionally corrects stats post-game).
    let seasons: Vec<i32> = vec![season_i32; rows.len()];
    let game_types: Vec<i16> = vec![game_type_i16; rows.len()];
    let game_ids: Vec<i64> = vec![game_id_i64; rows.len()];
    let player_ids: Vec<i64> = rows.iter().map(|r| r.player_id).collect();
    let team_abbrevs: Vec<&str> = rows.iter().map(|r| r.team_abbrev.as_str()).collect();
    let opponents: Vec<&str> = rows.iter().map(|r| r.opponent.as_str()).collect();
    let homes: Vec<bool> = rows.iter().map(|r| r.home).collect();
    let goals: Vec<i32> = rows.iter().map(|r| r.goals).collect();
    let assists: Vec<i32> = rows.iter().map(|r| r.assists).collect();
    let points: Vec<i32> = rows.iter().map(|r| r.points).collect();
    let shots: Vec<Option<i32>> = rows.iter().map(|r| r.shots).collect();
    let pp_points: Vec<Option<i32>> = rows.iter().map(|r| r.pp_points).collect();

    // game_date is the same value for every row in this game — bind it
    // as a scalar and use it directly in the SELECT's output list rather
    // than threading a per-row date array through sqlx (which would
    // require the chrono feature).
    let inserted = sqlx::query(
        r#"
        INSERT INTO playoff_skater_game_stats (
            season, game_type, game_id, game_date,
            player_id, team_abbrev, opponent, home,
            goals, assists, points, shots, pp_points
        )
        SELECT
            season, game_type, game_id, $13::date AS game_date,
            player_id, team_abbrev, opponent, home,
            goals, assists, points, shots, pp_points
        FROM UNNEST(
            $1::int[], $2::smallint[], $3::bigint[],
            $4::bigint[], $5::text[], $6::text[], $7::bool[],
            $8::int[], $9::int[], $10::int[], $11::int[], $12::int[]
        ) AS u(
            season, game_type, game_id,
            player_id, team_abbrev, opponent, home,
            goals, assists, points, shots, pp_points
        )
        ON CONFLICT (game_id, player_id) DO UPDATE SET
            team_abbrev = EXCLUDED.team_abbrev,
            opponent    = EXCLUDED.opponent,
            home        = EXCLUDED.home,
            goals       = EXCLUDED.goals,
            assists     = EXCLUDED.assists,
            points      = EXCLUDED.points,
            shots       = EXCLUDED.shots,
            pp_points   = EXCLUDED.pp_points
        "#,
    )
    .bind(&seasons)
    .bind(&game_types)
    .bind(&game_ids)
    .bind(&player_ids)
    .bind(&team_abbrevs)
    .bind(&opponents)
    .bind(&homes)
    .bind(&goals)
    .bind(&assists)
    .bind(&points)
    .bind(&shots)
    .bind(&pp_points)
    .bind(&game_date_str)
    .execute(db.pool())
    .await
    .map_err(crate::error::Error::Database)?
    .rows_affected() as usize;

    Ok(inserted)
}

struct SkaterRow {
    player_id: i64,
    team_abbrev: String,
    opponent: String,
    home: bool,
    goals: i32,
    assists: i32,
    points: i32,
    shots: Option<i32>,
    pp_points: Option<i32>,
}

fn boxscore_to_row(
    player: &crate::models::nhl::BoxscorePlayer,
    team_abbrev: &str,
    opponent: &str,
    home: bool,
) -> SkaterRow {
    let goals = player.goals.unwrap_or(0);
    let assists = player.assists.unwrap_or(0);
    // Some boxscores omit the convenience `points` field; fall back to
    // goals + assists so the table never has a stale NULL for a player
    // who clearly scored.
    let points = player.points.unwrap_or(goals + assists);
    // The box-score model we consume exposes `power_play_goals` but not
    // PP assists; store `pp_goals` as a partial PP-points signal when the
    // full figure isn't available.
    let pp_points = player.power_play_goals;
    SkaterRow {
        player_id: player.player_id as i64,
        team_abbrev: team_abbrev.to_string(),
        opponent: opponent.to_string(),
        home,
        goals,
        assists,
        points,
        shots: player.sog,
        pp_points,
    }
}

fn games_for_logging(schedule: &crate::models::nhl::TodaySchedule, date: &str) -> usize {
    schedule
        .games_for_date(date)
        .iter()
        .filter(|g| g.game_type == 3 && matches!(g.game_state, GameState::Final | GameState::Off))
        .count()
}
