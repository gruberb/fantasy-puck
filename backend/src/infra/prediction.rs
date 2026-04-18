//! Database-backed entry points for the pure `domain::prediction`
//! modules. These functions are the only place SQL touches the
//! prediction engine — they load the data the pure model needs, then
//! hand off to [`domain::prediction::playoff_elo`] and
//! [`domain::prediction::player_projection`].
//!
//! Keeping the DB layer here is what lets the prediction engine be
//! extracted into a standalone service later — see
//! `PREDICTION_SERVICE.md` at the repo root.

use std::collections::HashMap;

use tracing::debug;

use crate::db::FantasyDb;
use crate::domain::prediction::{
    playoff_elo::{self, GameResult},
    player_projection::{self, GameStats, PlayerInput, Projection},
};
use crate::error::{Error, Result};

/// Load every completed playoff game for the season, chronologically,
/// and return the current Elo map. Seeded from `standings` via
/// [`playoff_elo::seed_from_standings`] before replaying each game with
/// [`playoff_elo::apply_game`] so teams with no completed games still
/// get their RS-based starting rating.
pub async fn compute_current_elo(
    db: &FantasyDb,
    standings: &serde_json::Value,
    season: u32,
) -> Result<HashMap<String, f32>> {
    let mut ratings = playoff_elo::seed_from_standings(standings);

    let rows: Vec<(String, String, i32, i32)> = sqlx::query_as(
        r#"
        SELECT home_team, away_team, home_score, away_score
        FROM playoff_game_results
        WHERE season = $1
        ORDER BY game_date ASC, game_id ASC
        "#,
    )
    .bind(season as i32)
    .fetch_all(db.pool())
    .await
    .map_err(Error::Database)?;

    let game_count = rows.len();
    for (home_team, away_team, home_score, away_score) in rows {
        playoff_elo::apply_game(
            &mut ratings,
            &GameResult {
                home_team,
                away_team,
                home_score,
                away_score,
            },
        );
    }
    debug!(games = game_count, teams = ratings.len(), "playoff Elo replayed");
    Ok(ratings)
}

/// Project every player in a single DB round-trip.
///
/// Two batched queries (playoff game logs + historical totals), then
/// one synchronous fold through
/// [`player_projection::project_one`] per roster entry.
pub async fn project_players(
    db: &FantasyDb,
    season: u32,
    players: &[PlayerInput],
    team_games_played: &HashMap<String, u32>,
) -> Result<HashMap<i64, Projection>> {
    if players.is_empty() {
        return Ok(HashMap::new());
    }

    let player_ids: Vec<i64> = players.iter().map(|p| p.nhl_id).collect();
    // Order lets us slice per-player sub-ranges without rehashing —
    // game_date DESC so the "last N" window is already at the front.
    //
    // Since v1.14.0 the query pulls the richer columns the projection
    // now uses: goals/assists (separate from total points), shots for
    // volume-stabilised goal rate, pp_points for future PP-weighting,
    // and toi_seconds for the lineup-role multiplier. `shots`,
    // `pp_points`, and `toi_seconds` are nullable at the schema level
    // because older boxscores occasionally omit them.
    let stat_rows: Vec<(i64, i32, i32, Option<i32>, Option<i32>, Option<i32>)> =
        sqlx::query_as(
            r#"
            SELECT player_id, goals, assists, shots, pp_points, toi_seconds
            FROM playoff_skater_game_stats
            WHERE season = $1
              AND player_id = ANY($2::bigint[])
            ORDER BY player_id ASC, game_date DESC, game_id DESC
            "#,
        )
        .bind(season as i32)
        .bind(&player_ids)
        .fetch_all(db.pool())
        .await
        .map_err(Error::Database)?;

    // Bucket stat rows by player for per-player aggregation. Preserves
    // the ORDER BY — game_date DESC — so the first elements are the
    // most recent games.
    let mut by_player: HashMap<i64, Vec<GameStats>> = HashMap::with_capacity(players.len());
    for (pid, goals, assists, shots, pp_points, toi_seconds) in stat_rows {
        by_player.entry(pid).or_default().push(GameStats {
            goals,
            assists,
            shots,
            pp_points,
            toi_seconds,
        });
    }

    let names: Vec<&str> = players.iter().map(|p| p.player_name.as_str()).collect();
    let historical_rows: Vec<(String, i32, i32)> = sqlx::query_as(
        r#"
        SELECT player_name, gp, p
        FROM historical_playoff_skater_totals
        WHERE player_name = ANY($1::text[])
        "#,
    )
    .bind(&names)
    .fetch_all(db.pool())
    .await
    .map_err(Error::Database)?;

    let historical: HashMap<String, (i32, i32)> = historical_rows
        .into_iter()
        .map(|(n, gp, p)| (n, (gp, p)))
        .collect();

    let mut out: HashMap<i64, Projection> = HashMap::with_capacity(players.len());
    for p in players {
        let team_gp = team_games_played.get(&p.nhl_team).copied().unwrap_or(0);
        let game_log = by_player.get(&p.nhl_id).cloned().unwrap_or_default();
        let projection = player_projection::project_one(
            p,
            team_gp,
            &game_log,
            historical.get(&p.player_name),
        );
        out.insert(p.nhl_id, projection);
    }
    debug!(players = players.len(), "player projection batch complete");
    Ok(out)
}
