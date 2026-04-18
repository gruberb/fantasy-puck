use sqlx::postgres::PgPool;

use crate::error::Result;
use crate::domain::models::db::FantasySleeper;

pub struct SleeperDbService<'a> {
    pool: &'a PgPool,
}

impl<'a> SleeperDbService<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Get all sleepers for teams in a league
    pub async fn get_all_sleepers(&self, league_id: &str) -> Result<Vec<FantasySleeper>> {
        let sleepers = sqlx::query_as::<_, FantasySleeper>(
            r#"
            SELECT DISTINCT s.id, s.team_id, s.nhl_id, s.name, s.position, s.nhl_team
            FROM fantasy_sleepers s
            LEFT JOIN league_members lm ON lm.fantasy_team_id = s.team_id
            WHERE lm.league_id = $1::uuid
            "#,
        )
        .bind(league_id)
        .fetch_all(self.pool)
        .await?;

        Ok(sleepers)
    }

    /// Remove a sleeper by ID or NHL ID
    pub async fn remove_sleeper(&self, sleeper_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM fantasy_sleepers WHERE id = $1 OR nhl_id = $1")
            .bind(sleeper_id)
            .execute(self.pool)
            .await?;
        Ok(())
    }
}
