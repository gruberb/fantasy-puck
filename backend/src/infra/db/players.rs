use crate::infra::db::FantasyDb;
use std::collections::HashMap;

use crate::domain::models::db::{FantasyPlayer, NhlTeamPlayers, PlayerWithTeam};
use crate::error::Result;

impl FantasyDb {
    /// Add a player to a fantasy team.
    pub async fn add_player_to_team(
        &self,
        team_id: i64,
        nhl_id: i64,
        name: &str,
        position: &str,
        nhl_team: &str,
    ) -> Result<FantasyPlayer> {
        let player = sqlx::query_as::<_, FantasyPlayer>(
            r#"
            INSERT INTO fantasy_players (team_id, nhl_id, name, position, nhl_team)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, team_id, nhl_id, name, position, nhl_team
            "#,
        )
        .bind(team_id)
        .bind(nhl_id)
        .bind(name)
        .bind(position)
        .bind(nhl_team)
        .fetch_one(self.pool())
        .await?;

        Ok(player)
    }

    pub async fn remove_player(&self, player_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM fantasy_players WHERE id = $1")
            .bind(player_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Get all players in a fantasy team
    pub async fn get_team_players(&self, team_id: i64) -> Result<Vec<FantasyPlayer>> {
        let players = sqlx::query_as::<_, FantasyPlayer>(
            "SELECT id, team_id, nhl_id, name, position, nhl_team FROM fantasy_players WHERE team_id = $1"
        )
            .bind(team_id)
            .fetch_all(self.pool())
            .await?;

        Ok(players)
    }

    /// Returns a list of all NHL teams along with the fantasy players
    /// (and their fantasy-team info) that belong to each NHL team, scoped to a league.
    pub async fn get_nhl_teams_and_players(&self, league_id: &str) -> Result<Vec<NhlTeamPlayers>> {
        let rows: Vec<PlayerWithTeam> = sqlx::query_as::<_, PlayerWithTeam>(
            r#"
                SELECT
                    p.id               AS id,
                    p.nhl_id           AS nhl_id,
                    p.name             AS name,
                    p.team_id          AS fantasy_team_id,
                    t.name             AS fantasy_team_name,
                    p.position         AS position,
                    p.nhl_team         AS nhl_team
                FROM fantasy_players p
                INNER JOIN fantasy_teams t
                    ON p.team_id = t.id
                INNER JOIN league_members lm
                    ON lm.fantasy_team_id = t.id
                WHERE lm.league_id = $1::uuid
                ORDER BY p.nhl_team
                "#,
        )
        .bind(league_id)
        .fetch_all(self.pool())
        .await?;

        // Group by nhl_team using a HashMap
        let mut grouping_map: HashMap<String, Vec<PlayerWithTeam>> = HashMap::new();

        for row in rows {
            grouping_map
                .entry(row.nhl_team.clone())
                .or_default()
                .push(row);
        }

        // Convert the grouped map into the final Vec<NhlTeamPlayers>
        let mut result = Vec::with_capacity(grouping_map.len());
        for (nhl_team, players) in grouping_map {
            result.push(NhlTeamPlayers { nhl_team, players });
        }

        Ok(result)
    }
}
