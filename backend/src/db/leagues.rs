use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::FantasyDb;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LeagueRow {
    pub id: String,
    pub name: String,
    pub season: String,
    pub visibility: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LeagueMemberRow {
    pub id: String,
    pub user_id: String,
    pub draft_order: i32,
    pub display_name: String,
    pub team_name: String,
    pub fantasy_team_id: i64,
}

impl FantasyDb {
    /// Create a new league.
    pub async fn create_league(
        &self,
        name: &str,
        season: &str,
        created_by: &str,
    ) -> Result<LeagueRow> {
        let league = sqlx::query_as::<_, LeagueRow>(
            r#"
            INSERT INTO leagues (name, season, created_by)
            VALUES ($1, $2, $3::uuid)
            RETURNING id::text, name, season, visibility, created_by::text
            "#,
        )
        .bind(name)
        .bind(season)
        .bind(created_by)
        .fetch_one(self.pool())
        .await?;

        Ok(league)
    }

    /// Delete a league by id. Cascading foreign keys handle cleanup of members, teams, etc.
    pub async fn delete_league(&self, league_id: &str) -> Result<()> {
        // Deleting the league cascades to:
        // - league_members (ON DELETE CASCADE)
        // - draft_sessions → draft_picks, player_pool (ON DELETE CASCADE)
        // - fantasy_teams → fantasy_players, fantasy_sleepers (ON DELETE CASCADE)
        sqlx::query("DELETE FROM leagues WHERE id = $1::uuid")
            .bind(league_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Get all members of a league, joined with their profiles and fantasy teams.
    pub async fn get_league_members(
        &self,
        league_id: &str,
    ) -> Result<Vec<LeagueMemberRow>> {
        let members = sqlx::query_as::<_, LeagueMemberRow>(
            r#"
            SELECT
                lm.id::text,
                lm.user_id::text AS user_id,
                lm.draft_order,
                p.display_name,
                ft.name AS team_name,
                lm.fantasy_team_id
            FROM league_members lm
            JOIN profiles p ON p.id = lm.user_id
            JOIN fantasy_teams ft ON ft.id = lm.fantasy_team_id
            WHERE lm.league_id = $1::uuid
            ORDER BY lm.draft_order
            "#,
        )
        .bind(league_id)
        .fetch_all(self.pool())
        .await?;

        Ok(members)
    }

    /// Join a league: create a fantasy team and add a league membership in a transaction.
    pub async fn join_league(
        &self,
        league_id: &str,
        user_id: &str,
        team_name: &str,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;

        let team_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO fantasy_teams (name, user_id, league_id)
            VALUES ($1, $2::uuid, $3::uuid)
            RETURNING id
            "#,
        )
        .bind(team_name)
        .bind(user_id)
        .bind(league_id)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO league_members (league_id, user_id, fantasy_team_id)
            VALUES ($1::uuid, $2::uuid, $3)
            "#,
        )
        .bind(league_id)
        .bind(user_id)
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Remove a member from a league.
    pub async fn remove_league_member(&self, member_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM league_members WHERE id = $1::uuid")
            .bind(member_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Validate that a member belongs to a league. Returns the member id.
    pub async fn validate_league_member(
        &self,
        member_id: &str,
        league_id: &str,
    ) -> Result<String> {
        let id: String = sqlx::query_scalar(
            "SELECT id::text FROM league_members WHERE id = $1::uuid AND league_id = $2::uuid",
        )
        .bind(member_id)
        .bind(league_id)
        .fetch_one(self.pool())
        .await?;

        Ok(id)
    }
}
