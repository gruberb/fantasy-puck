use sqlx::postgres::PgPool;
use sqlx::FromRow;

use crate::error::{Error, Result};

#[derive(Debug, FromRow)]
pub struct NhlTeamRosterCountRow {
    pub nhl_team: String,
    pub rostered_count: i64,
}

#[derive(Debug, FromRow)]
pub struct NhlTeamPlayoffPointsRow {
    pub team_abbrev: String,
    pub playoff_points: i64,
}

#[derive(Debug, FromRow)]
pub struct NhlTeamTopSkaterRow {
    pub team_abbrev: String,
    pub player_id: i64,
    pub name: String,
    pub points: i64,
}

#[derive(Debug, FromRow)]
pub struct RosteredSkaterPointsRow {
    pub nhl_id: i64,
    pub name: String,
    pub nhl_team: String,
    pub fantasy_team_id: i64,
    pub fantasy_team_name: String,
    pub playoff_points: i64,
}

/// Count rostered players per NHL team within a single fantasy league.
/// Empty-string `nhl_team` rows (undrafted placeholders) are filtered
/// out by the caller; the handler decides what to show.
pub async fn list_nhl_team_roster_counts(
    pool: &PgPool,
    league_id: &str,
) -> Result<Vec<NhlTeamRosterCountRow>> {
    sqlx::query_as::<_, NhlTeamRosterCountRow>(
        r#"
        SELECT fp.nhl_team,
               COUNT(*)::bigint AS rostered_count
          FROM fantasy_players fp
          JOIN fantasy_teams ft ON ft.id = fp.team_id
         WHERE ft.league_id = $1::uuid
         GROUP BY fp.nhl_team
        "#,
    )
    .bind(league_id)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

/// Total playoff fantasy points per NHL team — every skater on that
/// team, not only those rostered in the league. Completed games only
/// (`game_state IN ('OFF','FINAL')`) so in-progress games don't inflate
/// totals mid-period.
pub async fn list_nhl_team_playoff_points(
    pool: &PgPool,
    season: i32,
) -> Result<Vec<NhlTeamPlayoffPointsRow>> {
    sqlx::query_as::<_, NhlTeamPlayoffPointsRow>(
        r#"
        SELECT pgs.team_abbrev,
               COALESCE(SUM(pgs.points), 0)::bigint AS playoff_points
          FROM nhl_player_game_stats pgs
          JOIN nhl_games g ON g.game_id = pgs.game_id
         WHERE g.season = $1
           AND g.game_type = 3
           AND g.game_state IN ('OFF', 'FINAL')
         GROUP BY pgs.team_abbrev
        "#,
    )
    .bind(season)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

/// Top playoff scorer per NHL team (one row per team_abbrev). Ties
/// broken by goals DESC, then name ASC for determinism.
pub async fn list_nhl_team_top_skaters(
    pool: &PgPool,
    season: i32,
) -> Result<Vec<NhlTeamTopSkaterRow>> {
    sqlx::query_as::<_, NhlTeamTopSkaterRow>(
        r#"
        WITH totals AS (
            SELECT pgs.team_abbrev,
                   pgs.player_id,
                   MAX(pgs.name)                        AS name,
                   COALESCE(SUM(pgs.points), 0)::bigint AS points,
                   COALESCE(SUM(pgs.goals), 0)::bigint  AS goals
              FROM nhl_player_game_stats pgs
              JOIN nhl_games g ON g.game_id = pgs.game_id
             WHERE g.season = $1
               AND g.game_type = 3
               AND g.game_state IN ('OFF', 'FINAL')
             GROUP BY pgs.team_abbrev, pgs.player_id
        ),
        ranked AS (
            SELECT team_abbrev,
                   player_id,
                   name,
                   points,
                   ROW_NUMBER() OVER (
                       PARTITION BY team_abbrev
                       ORDER BY points DESC, goals DESC, name ASC
                   ) AS rn
              FROM totals
        )
        SELECT team_abbrev, player_id, name, points
          FROM ranked
         WHERE rn = 1
        "#,
    )
    .bind(season)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}

/// Top-N rostered skaters in a league by playoff points. LEFT JOIN so
/// rostered players who haven't scored yet (or whose team hasn't
/// played) still appear with 0 points — useful once the playoff bracket
/// is fuller but the query result still needs to be deterministic.
pub async fn list_top_rostered_skaters(
    pool: &PgPool,
    league_id: &str,
    season: i32,
    limit: i32,
) -> Result<Vec<RosteredSkaterPointsRow>> {
    sqlx::query_as::<_, RosteredSkaterPointsRow>(
        r#"
        SELECT fp.nhl_id,
               fp.name,
               fp.nhl_team,
               ft.id                              AS fantasy_team_id,
               ft.name                            AS fantasy_team_name,
               COALESCE(totals.points, 0)::bigint AS playoff_points
          FROM fantasy_players fp
          JOIN fantasy_teams ft ON ft.id = fp.team_id
          LEFT JOIN (
              SELECT pgs.player_id,
                     COALESCE(SUM(pgs.points), 0) AS points
                FROM nhl_player_game_stats pgs
                JOIN nhl_games g ON g.game_id = pgs.game_id
               WHERE g.season = $1
                 AND g.game_type = 3
                 AND g.game_state IN ('OFF', 'FINAL')
               GROUP BY pgs.player_id
          ) totals ON totals.player_id = fp.nhl_id
         WHERE ft.league_id = $2::uuid
         ORDER BY playoff_points DESC, fp.name ASC
         LIMIT $3
        "#,
    )
    .bind(season)
    .bind(league_id)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(Error::Database)
}
