use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::domain::services::draft::pick_slot;
use crate::error::{Error, Result};
use crate::infra::db::FantasyDb;

// --- Row types (returned from queries) ---

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DraftSessionRow {
    pub id: String,
    pub league_id: String,
    pub status: String,
    pub current_round: i32,
    pub current_pick_index: i32,
    pub total_rounds: i32,
    pub snake_draft: bool,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub sleeper_status: Option<String>,
    pub sleeper_pick_index: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SleeperRow {
    pub id: i64,
    pub team_id: i64,
    pub nhl_id: i64,
    pub name: String,
    pub position: String,
    pub nhl_team: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPoolRow {
    pub id: String,
    pub draft_session_id: String,
    pub nhl_id: i64,
    pub name: String,
    pub position: String,
    pub nhl_team: String,
    pub headshot_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DraftPickRow {
    pub id: String,
    pub draft_session_id: String,
    pub league_member_id: String,
    pub player_pool_id: Option<String>,
    pub nhl_id: i64,
    pub player_name: String,
    pub nhl_team: String,
    pub position: String,
    pub round: i32,
    pub pick_number: i32,
    pub picked_at: String,
}

// --- Insert types (used as input) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPoolInsert {
    pub draft_session_id: String,
    pub nhl_id: i64,
    pub name: String,
    pub position: String,
    pub nhl_team: String,
    pub headshot_url: String,
}

const SESSION_COLUMNS: &str = "id::text, league_id::text, status, current_round, \
    current_pick_index, total_rounds, snake_draft, started_at::text, completed_at::text, \
    sleeper_status, sleeper_pick_index";

/// Locks the session row for the rest of `tx`. Every pick goes through
/// this lock, so concurrent picks for the same draft serialize instead of
/// claiming the same slot or player.
async fn lock_session(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: &str,
) -> Result<DraftSessionRow> {
    let sql =
        format!("SELECT {SESSION_COLUMNS} FROM draft_sessions WHERE id = $1::uuid FOR UPDATE");
    Ok(sqlx::query_as::<_, DraftSessionRow>(&sql)
        .bind(session_id)
        .fetch_one(&mut **tx)
        .await?)
}

async fn member_ids_in_order(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    league_id: &str,
) -> Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT id::text FROM league_members WHERE league_id = $1::uuid ORDER BY draft_order",
    )
    .bind(league_id)
    .fetch_all(&mut **tx)
    .await?)
}

async fn pool_player(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    pool_player_id: &str,
    session_id: &str,
) -> Result<PlayerPoolRow> {
    Ok(sqlx::query_as::<_, PlayerPoolRow>(
        r#"
        SELECT id::text, draft_session_id::text, nhl_id, name, position, nhl_team, headshot_url
          FROM player_pool
         WHERE id = $1::uuid AND draft_session_id = $2::uuid
        "#,
    )
    .bind(pool_player_id)
    .bind(session_id)
    .fetch_one(&mut **tx)
    .await?)
}

impl FantasyDb {
    /// Make the next pick of an active draft: validate, insert, and advance
    /// the session, all under the session row lock. Returns the pick and
    /// the updated session.
    pub async fn make_draft_pick(
        &self,
        session_id: &str,
        pool_player_id: &str,
    ) -> Result<(DraftPickRow, DraftSessionRow)> {
        let mut tx = self.pool().begin().await?;
        let session = lock_session(&mut tx, session_id).await?;
        if session.status != "active" {
            return Err(Error::Validation("Draft is not active".into()));
        }
        let members = member_ids_in_order(&mut tx, &session.league_id).await?;
        let num_members = members.len() as i32;
        if num_members == 0 {
            return Err(Error::Validation("No members in league".into()));
        }
        let total_picks = session.total_rounds * num_members;
        let pick_index = session.current_pick_index;
        if pick_index >= total_picks {
            return Err(Error::Validation("All rounds are complete".into()));
        }

        let player = pool_player(&mut tx, pool_player_id, session_id).await?;
        let taken: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM draft_picks WHERE draft_session_id = $1::uuid AND nhl_id = $2)",
        )
        .bind(session_id)
        .bind(player.nhl_id)
        .fetch_one(&mut *tx)
        .await?;
        if taken {
            return Err(Error::Validation("Player already drafted".into()));
        }

        let (round, slot) = pick_slot(pick_index, num_members, session.snake_draft);
        let pick = sqlx::query_as::<_, DraftPickRow>(
            r#"
            INSERT INTO draft_picks
                (draft_session_id, league_member_id, player_pool_id, nhl_id,
                 player_name, nhl_team, position, round, pick_number)
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9)
            RETURNING
                id::text, draft_session_id::text, league_member_id::text,
                player_pool_id::text, nhl_id, player_name, nhl_team, position,
                round, pick_number, picked_at::text
            "#,
        )
        .bind(session_id)
        .bind(&members[slot])
        .bind(pool_player_id)
        .bind(player.nhl_id)
        .bind(&player.name)
        .bind(&player.nhl_team)
        .bind(&player.position)
        // Rounds are 1-based for display; pick_number stays the 0-based
        // global index the frontend uses.
        .bind(round + 1)
        .bind(pick_index)
        .fetch_one(&mut *tx)
        .await?;

        let next = pick_index + 1;
        // `picks_done` waits for the owner to finalize before the sleeper round.
        let status = if next >= total_picks {
            "picks_done"
        } else {
            "active"
        };
        let sql = format!(
            "UPDATE draft_sessions SET current_pick_index = $1, current_round = $2, status = $3 \
             WHERE id = $4::uuid RETURNING {SESSION_COLUMNS}"
        );
        let updated = sqlx::query_as::<_, DraftSessionRow>(&sql)
            .bind(next)
            .bind(next / num_members + 1)
            .bind(status)
            .bind(session_id)
            .fetch_one(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok((pick, updated))
    }

    /// Make a sleeper pick for `team_id`: validate, insert, and advance the
    /// sleeper round under the session row lock. Returns the updated session.
    pub async fn make_sleeper_pick(
        &self,
        session_id: &str,
        team_id: i64,
        pool_player_id: &str,
    ) -> Result<DraftSessionRow> {
        let mut tx = self.pool().begin().await?;
        let session = lock_session(&mut tx, session_id).await?;
        if session.sleeper_status.as_deref() != Some("active") {
            return Err(Error::Validation("Sleeper round is not active".into()));
        }
        // Each member gets exactly one sleeper.
        let num_members = member_ids_in_order(&mut tx, &session.league_id)
            .await?
            .len() as i32;
        if session.sleeper_pick_index >= num_members {
            return Err(Error::Validation("All sleeper picks are done".into()));
        }

        let team_league: Option<String> =
            sqlx::query_scalar("SELECT league_id::text FROM fantasy_teams WHERE id = $1")
                .bind(team_id)
                .fetch_optional(&mut *tx)
                .await?;
        if team_league.as_deref() != Some(session.league_id.as_str()) {
            return Err(Error::Validation(
                "Team is not part of this draft's league".into(),
            ));
        }

        let player = pool_player(&mut tx, pool_player_id, session_id).await?;
        // Sleepers are exclusive per league, not globally: the same NHL
        // player can be a sleeper in two different leagues.
        let (team_has_one, player_taken): (bool, bool) = sqlx::query_as(
            r#"
            SELECT
                EXISTS (SELECT 1 FROM fantasy_sleepers WHERE team_id = $1),
                EXISTS (
                    SELECT 1
                      FROM fantasy_sleepers fs
                      JOIN league_members lm ON lm.fantasy_team_id = fs.team_id
                     WHERE lm.league_id = $2::uuid AND fs.nhl_id = $3
                )
            "#,
        )
        .bind(team_id)
        .bind(&session.league_id)
        .bind(player.nhl_id)
        .fetch_one(&mut *tx)
        .await?;
        if team_has_one {
            return Err(Error::Validation(
                "This team already has a sleeper pick".into(),
            ));
        }
        if player_taken {
            return Err(Error::Validation(
                "This player is already picked as a sleeper by another team".into(),
            ));
        }

        sqlx::query(
            "INSERT INTO fantasy_sleepers (team_id, nhl_id, name, position, nhl_team) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(team_id)
        .bind(player.nhl_id)
        .bind(&player.name)
        .bind(&player.position)
        .bind(&player.nhl_team)
        .execute(&mut *tx)
        .await?;

        let next = session.sleeper_pick_index + 1;
        let status = if next >= num_members {
            "completed"
        } else {
            "active"
        };
        let sql = format!(
            "UPDATE draft_sessions SET sleeper_pick_index = $1, sleeper_status = $2 \
             WHERE id = $3::uuid RETURNING {SESSION_COLUMNS}"
        );
        let updated = sqlx::query_as::<_, DraftSessionRow>(&sql)
            .bind(next)
            .bind(status)
            .bind(session_id)
            .fetch_one(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(updated)
    }

    /// Create a new draft session for a league.
    pub async fn create_draft_session(
        &self,
        league_id: &str,
        total_rounds: i32,
        snake_draft: bool,
    ) -> Result<DraftSessionRow> {
        let session = sqlx::query_as::<_, DraftSessionRow>(
            r#"
            INSERT INTO draft_sessions (league_id, total_rounds, snake_draft)
            VALUES ($1::uuid, $2, $3)
            RETURNING
                id::text,
                league_id::text,
                status,
                current_round,
                current_pick_index,
                total_rounds,
                snake_draft,
                started_at::text,
                completed_at::text,
                sleeper_status,
                sleeper_pick_index
            "#,
        )
        .bind(league_id)
        .bind(total_rounds)
        .bind(snake_draft)
        .fetch_one(self.pool())
        .await?;

        Ok(session)
    }

    /// Get the most recent draft session for a league.
    pub async fn get_draft_session(&self, league_id: &str) -> Result<Option<DraftSessionRow>> {
        let session = sqlx::query_as::<_, DraftSessionRow>(
            r#"
            SELECT
                id::text,
                league_id::text,
                status,
                current_round,
                current_pick_index,
                total_rounds,
                snake_draft,
                started_at::text,
                completed_at::text,
                sleeper_status,
                sleeper_pick_index
            FROM draft_sessions
            WHERE league_id = $1::uuid
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(league_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(session)
    }

    /// Get a draft session by its id.
    pub async fn get_draft_session_by_id(&self, session_id: &str) -> Result<DraftSessionRow> {
        let session = sqlx::query_as::<_, DraftSessionRow>(
            r#"
            SELECT
                id::text,
                league_id::text,
                status,
                current_round,
                current_pick_index,
                total_rounds,
                snake_draft,
                started_at::text,
                completed_at::text,
                sleeper_status,
                sleeper_pick_index
            FROM draft_sessions
            WHERE id = $1::uuid
            "#,
        )
        .bind(session_id)
        .fetch_one(self.pool())
        .await?;

        Ok(session)
    }

    /// Update draft session status and optional timestamps.
    pub async fn update_draft_status(
        &self,
        session_id: &str,
        status: &str,
        started_at: Option<&str>,
        completed_at: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE draft_sessions
            SET status = $1,
                started_at = COALESCE($2::timestamptz, started_at),
                completed_at = COALESCE($3::timestamptz, completed_at)
            WHERE id = $4::uuid
            "#,
        )
        .bind(status)
        .bind(started_at)
        .bind(completed_at)
        .bind(session_id)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Get all players in the player pool for a draft session.
    pub async fn get_player_pool(&self, session_id: &str) -> Result<Vec<PlayerPoolRow>> {
        let players = sqlx::query_as::<_, PlayerPoolRow>(
            r#"
            SELECT
                id::text,
                draft_session_id::text,
                nhl_id,
                name,
                position,
                nhl_team,
                headshot_url
            FROM player_pool
            WHERE draft_session_id = $1::uuid
            ORDER BY name
            "#,
        )
        .bind(session_id)
        .fetch_all(self.pool())
        .await?;

        Ok(players)
    }

    /// Bulk insert players into the player pool. Returns the number of rows inserted.
    pub async fn insert_player_pool(
        &self,
        session_id: &str,
        players: Vec<PlayerPoolInsert>,
    ) -> Result<usize> {
        if players.is_empty() {
            return Ok(0);
        }

        // Build a bulk INSERT with multiple value rows
        let mut values = Vec::with_capacity(players.len());
        let mut param_index = 1u32;

        for _ in &players {
            values.push(format!(
                "(${param}::uuid, ${nhl}, ${name}, ${pos}, ${team}, ${head})",
                param = param_index,
                nhl = param_index + 1,
                name = param_index + 2,
                pos = param_index + 3,
                team = param_index + 4,
                head = param_index + 5,
            ));
            param_index += 6;
        }

        let query_str = format!(
            r#"
            INSERT INTO player_pool (draft_session_id, nhl_id, name, position, nhl_team, headshot_url)
            VALUES {}
            "#,
            values.join(", ")
        );

        let mut query = sqlx::query(&query_str);
        for player in &players {
            query = query
                .bind(session_id)
                .bind(player.nhl_id)
                .bind(&player.name)
                .bind(&player.position)
                .bind(&player.nhl_team)
                .bind(&player.headshot_url);
        }

        let result = query.execute(self.pool()).await?;

        Ok(result.rows_affected() as usize)
    }

    /// Get all draft picks for a session, ordered by pick number.
    pub async fn get_draft_picks(&self, session_id: &str) -> Result<Vec<DraftPickRow>> {
        let picks = sqlx::query_as::<_, DraftPickRow>(
            r#"
            SELECT
                id::text,
                draft_session_id::text,
                league_member_id::text,
                player_pool_id::text,
                nhl_id,
                player_name,
                nhl_team,
                position,
                round,
                pick_number,
                picked_at::text
            FROM draft_picks
            WHERE draft_session_id = $1::uuid
            ORDER BY pick_number
            "#,
        )
        .bind(session_id)
        .fetch_all(self.pool())
        .await?;

        Ok(picks)
    }

    /// Delete a draft session and all associated data (picks, player pool).
    pub async fn delete_draft_session(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM draft_sessions WHERE id = $1::uuid")
            .bind(session_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Randomize draft order for all members in a league.
    pub async fn randomize_draft_order(&self, league_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE league_members lm
               SET draft_order = shuffled.slot
              FROM (
                  SELECT id, (ROW_NUMBER() OVER (ORDER BY random()) - 1)::int AS slot
                    FROM league_members
                   WHERE league_id = $1::uuid
              ) shuffled
             WHERE lm.id = shuffled.id
            "#,
        )
        .bind(league_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Update sleeper draft status and pick index on a draft session.
    pub async fn update_sleeper_status(
        &self,
        session_id: &str,
        status: &str,
        pick_index: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE draft_sessions
            SET sleeper_status = $1, sleeper_pick_index = $2
            WHERE id = $3::uuid
            "#,
        )
        .bind(status)
        .bind(pick_index)
        .bind(session_id)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Start a pending draft session: set status to 'active' and started_at to now.
    pub async fn start_draft_session(&self, session_id: &str) -> Result<DraftSessionRow> {
        let session = sqlx::query_as::<_, DraftSessionRow>(
            r#"
            UPDATE draft_sessions
            SET status = 'active', started_at = now()
            WHERE id = $1::uuid AND status = 'pending'
            RETURNING
                id::text, league_id::text, status,
                current_round, current_pick_index, total_rounds, snake_draft,
                started_at::text, completed_at::text,
                sleeper_status, sleeper_pick_index
            "#,
        )
        .bind(session_id)
        .fetch_one(self.pool())
        .await?;

        Ok(session)
    }

    /// Pause an active draft session.
    pub async fn pause_draft_session(&self, session_id: &str) -> Result<DraftSessionRow> {
        let session = sqlx::query_as::<_, DraftSessionRow>(
            r#"
            UPDATE draft_sessions
            SET status = 'paused'
            WHERE id = $1::uuid AND status = 'active'
            RETURNING
                id::text, league_id::text, status,
                current_round, current_pick_index, total_rounds, snake_draft,
                started_at::text, completed_at::text,
                sleeper_status, sleeper_pick_index
            "#,
        )
        .bind(session_id)
        .fetch_one(self.pool())
        .await?;

        Ok(session)
    }

    /// Resume a paused draft session.
    pub async fn resume_draft_session(&self, session_id: &str) -> Result<DraftSessionRow> {
        let session = sqlx::query_as::<_, DraftSessionRow>(
            r#"
            UPDATE draft_sessions
            SET status = 'active'
            WHERE id = $1::uuid AND status = 'paused'
            RETURNING
                id::text, league_id::text, status,
                current_round, current_pick_index, total_rounds, snake_draft,
                started_at::text, completed_at::text,
                sleeper_status, sleeper_pick_index
            "#,
        )
        .bind(session_id)
        .fetch_one(self.pool())
        .await?;

        Ok(session)
    }

    /// Mark a draft session as completed and copy draft picks to fantasy_players.
    pub async fn finalize_draft_to_players(&self, session_id: &str) -> Result<()> {
        let mut tx = self.pool().begin().await?;

        // Mark session as completed
        sqlx::query(
            r#"
            UPDATE draft_sessions
            SET status = 'completed', completed_at = now()
            WHERE id = $1::uuid
            "#,
        )
        .bind(session_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO fantasy_players (team_id, nhl_id, name, position, nhl_team)
            SELECT lm.fantasy_team_id, dp.nhl_id, dp.player_name, dp.position, dp.nhl_team
              FROM draft_picks dp
              JOIN league_members lm ON lm.id = dp.league_member_id
             WHERE dp.draft_session_id = $1::uuid
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(session_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Delete all players from the pool for a given draft session.
    pub async fn delete_player_pool(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM player_pool WHERE draft_session_id = $1::uuid")
            .bind(session_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Start the sleeper round for a draft session, returning the updated session.
    pub async fn start_sleeper_round(&self, session_id: &str) -> Result<DraftSessionRow> {
        let session = sqlx::query_as::<_, DraftSessionRow>(
            r#"
            UPDATE draft_sessions
            SET sleeper_status = 'active', sleeper_pick_index = 0
            WHERE id = $1::uuid
            RETURNING
                id::text, league_id::text, status,
                current_round, current_pick_index, total_rounds, snake_draft,
                started_at::text, completed_at::text,
                sleeper_status, sleeper_pick_index
            "#,
        )
        .bind(session_id)
        .fetch_one(self.pool())
        .await?;

        Ok(session)
    }

    pub async fn list_league_sleepers(&self, league_id: &str) -> Result<Vec<SleeperRow>> {
        let rows = sqlx::query_as::<_, SleeperRow>(
            r#"
            SELECT fs.id, fs.team_id, fs.nhl_id, fs.name, fs.position, fs.nhl_team
            FROM fantasy_sleepers fs
            JOIN league_members lm ON lm.fantasy_team_id = fs.team_id
            WHERE lm.league_id = $1::uuid
            ORDER BY fs.id
            "#,
        )
        .bind(league_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows)
    }

    /// Get undrafted players from the pool (eligible for sleeper picks).
    pub async fn get_undrafted_pool_players(&self, session_id: &str) -> Result<Vec<PlayerPoolRow>> {
        let players = sqlx::query_as::<_, PlayerPoolRow>(
            r#"
            SELECT
                pp.id::text, pp.draft_session_id::text,
                pp.nhl_id, pp.name, pp.position, pp.nhl_team, pp.headshot_url
            FROM player_pool pp
            WHERE pp.draft_session_id = $1::uuid
              AND pp.nhl_id NOT IN (
                  SELECT dp.nhl_id FROM draft_picks dp WHERE dp.draft_session_id = $1::uuid
              )
              AND pp.nhl_id NOT IN (
                  SELECT fs.nhl_id FROM fantasy_sleepers fs
              )
            ORDER BY pp.name
            "#,
        )
        .bind(session_id)
        .fetch_all(self.pool())
        .await?;

        Ok(players)
    }
}
