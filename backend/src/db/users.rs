use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::db::FantasyDb;
use crate::error::Result;

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct MembershipRow {
    pub league_id: String,
    pub league_name: String,
    pub league_season: String,
    pub fantasy_team_id: Option<i64>,
    pub team_name: Option<String>,
    pub draft_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserRow {
    pub id: String,
    pub email: String,
    pub password_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProfileRow {
    pub id: String,
    pub display_name: String,
    pub is_admin: bool,
}

impl FantasyDb {
    /// Look up a user by email address. Returns None if not found.
    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UserRow>> {
        let user = sqlx::query_as::<_, UserRow>(
            "SELECT id::text, email, password_hash FROM users WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(self.pool())
        .await?;

        Ok(user)
    }

    /// Get a user by their UUID id.
    pub async fn get_user_by_id(&self, id: &str) -> Result<UserRow> {
        let user = sqlx::query_as::<_, UserRow>(
            "SELECT id::text, email, password_hash FROM users WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_one(self.pool())
        .await?;

        Ok(user)
    }

    /// Create a new user with email and password hash.
    pub async fn create_user(&self, email: &str, password_hash: &str) -> Result<UserRow> {
        let user = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO users (email, password_hash)
            VALUES ($1, $2)
            RETURNING id::text, email, password_hash
            "#,
        )
        .bind(email)
        .bind(password_hash)
        .fetch_one(self.pool())
        .await?;

        Ok(user)
    }

    /// Update a user's password hash.
    pub async fn update_password_hash(&self, id: &str, new_hash: &str) -> Result<()> {
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2::uuid")
            .bind(new_hash)
            .bind(id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Get a user's profile by user id.
    pub async fn get_profile(&self, user_id: &str) -> Result<ProfileRow> {
        let profile = sqlx::query_as::<_, ProfileRow>(
            "SELECT id::text, display_name, is_admin FROM profiles WHERE id = $1::uuid",
        )
        .bind(user_id)
        .fetch_one(self.pool())
        .await?;

        Ok(profile)
    }

    /// Create a profile for a user.
    pub async fn create_profile(&self, user_id: &str, display_name: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO profiles (id, display_name)
            VALUES ($1::uuid, $2)
            "#,
        )
        .bind(user_id)
        .bind(display_name)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Update a user's display name.
    pub async fn update_profile(&self, user_id: &str, display_name: &str) -> Result<()> {
        sqlx::query("UPDATE profiles SET display_name = $1 WHERE id = $2::uuid")
            .bind(display_name)
            .bind(user_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Delete a user account by calling the delete_user_account SQL function.
    pub async fn delete_user_account(&self, user_id: &str) -> Result<()> {
        sqlx::query("SELECT delete_user_account($1::uuid)")
            .bind(user_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Get all league memberships for a user with league and team info.
    pub async fn get_user_memberships(&self, user_id: &str) -> Result<Vec<MembershipRow>> {
        let memberships = sqlx::query_as::<_, MembershipRow>(
            r#"
            SELECT
                lm.league_id::text AS league_id,
                l.name AS league_name,
                l.season AS league_season,
                lm.fantasy_team_id,
                ft.name AS team_name,
                lm.draft_order
            FROM league_members lm
            JOIN leagues l ON l.id = lm.league_id
            LEFT JOIN fantasy_teams ft ON ft.id = lm.fantasy_team_id
            WHERE lm.user_id = $1::uuid
            ORDER BY l.name
            "#,
        )
        .bind(user_id)
        .fetch_all(self.pool())
        .await?;

        Ok(memberships)
    }
}
