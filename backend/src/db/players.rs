use std::collections::HashMap;

use crate::error::Result;
use crate::models::db::{FantasyPlayer, NhlTeamPlayers, PlayerWithTeam};
use crate::models::fantasy::{FantasyTeamInGame, PlayerInGame};
use sqlx::postgres::PgPool;
use sqlx::Row;

pub struct PlayerDbService<'a> {
    pool: &'a PgPool,
}

impl<'a> PlayerDbService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

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
        .fetch_one(self.pool)
        .await?;

        Ok(player)
    }

    /// Remove a player from a fantasy team.
    pub async fn remove_player(&self, player_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM fantasy_players WHERE id = $1")
            .bind(player_id)
            .execute(self.pool)
            .await?;

        Ok(())
    }

    /// Get all players in a fantasy team
    pub async fn get_team_players(&self, team_id: i64) -> Result<Vec<FantasyPlayer>> {
        let players = sqlx::query_as::<_, FantasyPlayer>(
            "SELECT id, team_id, nhl_id, name, position, nhl_team FROM fantasy_players WHERE team_id = $1"
        )
            .bind(team_id)
            .fetch_all(self.pool)
            .await?;

        Ok(players)
    }

    /// Returns a list of all NHL teams along with the fantasy players
    /// (and their fantasy-team info) that belong to each NHL team, scoped to a league.
    pub async fn get_nhl_teams_and_players(
        &self,
        league_id: &str,
    ) -> Result<Vec<NhlTeamPlayers>> {
        let rows: Vec<PlayerWithTeam> = sqlx::query_as::<_, PlayerWithTeam>(
            r#"
                SELECT
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
        .fetch_all(self.pool)
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

    pub async fn get_fantasy_players_for_nhl_teams(
        &self,
        nhl_teams: &[&str],
        league_id: &str,
    ) -> Result<Vec<FantasyTeamInGame>> {
        if nhl_teams.is_empty() {
            return Ok(Vec::new());
        }

        // Build parameterized placeholders for the IN clause
        // $1 is league_id, then $2..$N+1 are the nhl teams
        let placeholders = nhl_teams
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 2))
            .collect::<Vec<_>>()
            .join(",");
        let query_str = format!(
            r#"SELECT
              p.id as player_id,
              p.nhl_id as nhl_id,
              p.name as player_name,
              p.team_id as fantasy_team_id,
              t.name as fantasy_team_name,
              p.nhl_team,
              p.position
           FROM fantasy_players p
           JOIN fantasy_teams t ON p.team_id = t.id
           INNER JOIN league_members lm ON lm.fantasy_team_id = t.id
           WHERE lm.league_id = $1::uuid
           AND p.nhl_team IN ({})"#,
            placeholders
        );

        // Bind league_id first, then each team abbreviation
        let mut query = sqlx::query(&query_str);
        query = query.bind(league_id);
        for team in nhl_teams {
            query = query.bind(team);
        }

        let rows = query
            .map(|row: sqlx::postgres::PgRow| {
                let player_id: i64 = row.get("player_id");
                let nhl_id: i64 = row.get("nhl_id");
                let player_name: String = row.get("player_name");
                let fantasy_team_id: i64 = row.get("fantasy_team_id");
                let fantasy_team_name: String = row.get("fantasy_team_name");
                let nhl_team: String = row.get("nhl_team");
                let position: String = row.get("position");

                (
                    fantasy_team_id,
                    fantasy_team_name,
                    player_id,
                    nhl_id,
                    player_name,
                    nhl_team,
                    position,
                )
            })
            .fetch_all(self.pool)
            .await?;

        // Group by fantasy team
        let mut teams_map: HashMap<i64, FantasyTeamInGame> = HashMap::new();

        for (
            fantasy_team_id,
            fantasy_team_name,
            player_id,
            nhl_id,
            player_name,
            nhl_team,
            position,
        ) in rows
        {
            let team_entry =
                teams_map
                    .entry(fantasy_team_id)
                    .or_insert_with(|| FantasyTeamInGame {
                        team_id: fantasy_team_id,
                        team_name: fantasy_team_name.clone(),
                        players: Vec::new(),
                    });

            team_entry.players.push(PlayerInGame {
                player_id,
                nhl_id,
                player_name,
                nhl_team,
                position,
            });
        }

        let result: Vec<FantasyTeamInGame> = teams_map.into_values().collect();
        Ok(result)
    }
}
