use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::infra::db::FantasyDb;
use crate::error::Result;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftPickInsert {
    pub draft_session_id: String,
    pub league_member_id: String,
    pub player_pool_id: String,
    pub nhl_id: i64,
    pub player_name: String,
    pub nhl_team: String,
    pub position: String,
    pub round: i32,
    pub pick_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FantasyPlayerInsert {
    pub team_id: i64,
    pub nhl_id: i64,
    pub name: String,
    pub position: String,
    pub nhl_team: String,
}

impl FantasyDb {
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
    pub async fn get_draft_session(
        &self,
        league_id: &str,
    ) -> Result<Option<DraftSessionRow>> {
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
    pub async fn get_draft_session_by_id(
        &self,
        session_id: &str,
    ) -> Result<DraftSessionRow> {
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

    /// Update the current pick index and round for a draft session.
    pub async fn update_draft_pick_index(
        &self,
        session_id: &str,
        pick_index: i32,
        round: i32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE draft_sessions
            SET current_pick_index = $1, current_round = $2
            WHERE id = $3::uuid
            "#,
        )
        .bind(pick_index)
        .bind(round)
        .bind(session_id)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Get all players in the player pool for a draft session.
    pub async fn get_player_pool(
        &self,
        session_id: &str,
    ) -> Result<Vec<PlayerPoolRow>> {
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
    pub async fn get_draft_picks(
        &self,
        session_id: &str,
    ) -> Result<Vec<DraftPickRow>> {
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

    /// Insert a single draft pick and return the created row.
    pub async fn insert_draft_pick(
        &self,
        pick: DraftPickInsert,
    ) -> Result<DraftPickRow> {
        let row = sqlx::query_as::<_, DraftPickRow>(
            r#"
            INSERT INTO draft_picks
                (draft_session_id, league_member_id, player_pool_id, nhl_id,
                 player_name, nhl_team, position, round, pick_number)
            VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9)
            RETURNING
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
            "#,
        )
        .bind(&pick.draft_session_id)
        .bind(&pick.league_member_id)
        .bind(&pick.player_pool_id)
        .bind(pick.nhl_id)
        .bind(&pick.player_name)
        .bind(&pick.nhl_team)
        .bind(&pick.position)
        .bind(pick.round)
        .bind(pick.pick_number)
        .fetch_one(self.pool())
        .await?;

        Ok(row)
    }

    /// Delete a draft session and all associated data (picks, player pool).
    pub async fn delete_draft_session(&self, session_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM draft_sessions WHERE id = $1::uuid")
            .bind(session_id)
            .execute(self.pool())
            .await?;

        Ok(())
    }

    /// Finalize a draft: bulk-insert fantasy players and mark the session as completed.
    pub async fn finalize_draft(
        &self,
        session_id: &str,
        players: Vec<FantasyPlayerInsert>,
    ) -> Result<usize> {
        if players.is_empty() {
            return Ok(0);
        }

        let mut tx = self.pool().begin().await?;

        // Build bulk INSERT for fantasy_players
        let mut values = Vec::with_capacity(players.len());
        let mut param_index = 1u32;

        for _ in &players {
            values.push(format!(
                "(${team}, ${nhl}, ${name}, ${pos}, ${nhl_team})",
                team = param_index,
                nhl = param_index + 1,
                name = param_index + 2,
                pos = param_index + 3,
                nhl_team = param_index + 4,
            ));
            param_index += 5;
        }

        let query_str = format!(
            r#"
            INSERT INTO fantasy_players (team_id, nhl_id, name, position, nhl_team)
            VALUES {}
            "#,
            values.join(", ")
        );

        let mut query = sqlx::query(&query_str);
        for player in &players {
            query = query
                .bind(player.team_id)
                .bind(player.nhl_id)
                .bind(&player.name)
                .bind(&player.position)
                .bind(&player.nhl_team);
        }

        let result = query.execute(&mut *tx).await?;
        let count = result.rows_affected() as usize;

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

        tx.commit().await?;

        Ok(count)
    }

    /// Randomize draft order for all members in a league.
    pub async fn randomize_draft_order(&self, league_id: &str) -> Result<()> {
        use rand::seq::SliceRandom;

        // Fetch all member IDs for this league
        let member_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id::text FROM league_members WHERE league_id = $1::uuid",
        )
        .bind(league_id)
        .fetch_all(self.pool())
        .await?;

        if member_ids.is_empty() {
            return Ok(());
        }

        // Shuffle the order (scope rng so it's dropped before .await)
        let order = {
            let mut rng = rand::thread_rng();
            let mut order: Vec<usize> = (0..member_ids.len()).collect();
            order.shuffle(&mut rng);
            order
        };

        // Update each member's draft_order in a transaction
        let mut tx = self.pool().begin().await?;

        for (new_order, member_id) in order.iter().zip(member_ids.iter()) {
            sqlx::query(
                "UPDATE league_members SET draft_order = $1 WHERE id = $2::uuid",
            )
            .bind(*new_order as i32)
            .bind(member_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

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

    /// Insert a sleeper pick for a fantasy team.
    pub async fn insert_sleeper_pick(
        &self,
        team_id: i64,
        nhl_id: i64,
        name: &str,
        position: &str,
        nhl_team: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO fantasy_sleepers (team_id, nhl_id, name, position, nhl_team)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(team_id)
        .bind(nhl_id)
        .bind(name)
        .bind(position)
        .bind(nhl_team)
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

    /// Get a single player from the pool by pool id and draft session id.
    pub async fn get_draft_pool_player(
        &self,
        pool_player_id: &str,
        session_id: &str,
    ) -> Result<PlayerPoolRow> {
        let player = sqlx::query_as::<_, PlayerPoolRow>(
            r#"
            SELECT
                id::text, draft_session_id::text,
                nhl_id, name, position, nhl_team, headshot_url
            FROM player_pool
            WHERE id = $1::uuid AND draft_session_id = $2::uuid
            "#,
        )
        .bind(pool_player_id)
        .bind(session_id)
        .fetch_one(self.pool())
        .await?;

        Ok(player)
    }

    /// Check if a player (by nhl_id) has already been picked in a draft session.
    pub async fn check_player_already_picked(
        &self,
        session_id: &str,
        nhl_id: i64,
    ) -> Result<bool> {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id::text FROM draft_picks WHERE draft_session_id = $1::uuid AND nhl_id = $2",
        )
        .bind(session_id)
        .bind(nhl_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(existing.is_some())
    }

    /// Get league member IDs ordered by draft_order.
    pub async fn get_league_member_ids_ordered(
        &self,
        league_id: &str,
    ) -> Result<Vec<String>> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id::text FROM league_members WHERE league_id = $1::uuid ORDER BY draft_order",
        )
        .bind(league_id)
        .fetch_all(self.pool())
        .await?;

        Ok(ids)
    }

    /// Advance the draft session pick index and round, returning the updated session.
    pub async fn advance_draft_session(
        &self,
        session_id: &str,
        new_pick_index: i32,
        new_round: i32,
    ) -> Result<DraftSessionRow> {
        let session = sqlx::query_as::<_, DraftSessionRow>(
            r#"
            UPDATE draft_sessions
            SET current_pick_index = $1, current_round = $2
            WHERE id = $3::uuid
            RETURNING
                id::text, league_id::text, status,
                current_round, current_pick_index, total_rounds, snake_draft,
                started_at::text, completed_at::text,
                sleeper_status, sleeper_pick_index
            "#,
        )
        .bind(new_pick_index)
        .bind(new_round)
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

        // Get all picks with the fantasy_team_id from league_members
        let picks: Vec<(i64, i64, String, String, String)> = sqlx::query_as(
            r#"
            SELECT
                lm.fantasy_team_id,
                dp.nhl_id,
                dp.player_name,
                dp.position,
                dp.nhl_team
            FROM draft_picks dp
            JOIN league_members lm ON lm.id = dp.league_member_id
            WHERE dp.draft_session_id = $1::uuid
            "#,
        )
        .bind(session_id)
        .fetch_all(&mut *tx)
        .await?;

        // Insert each pick into fantasy_players
        for (team_id, nhl_id, name, position, nhl_team) in picks {
            sqlx::query(
                r#"
                INSERT INTO fantasy_players (team_id, nhl_id, name, position, nhl_team)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(team_id)
            .bind(nhl_id)
            .bind(&name)
            .bind(&position)
            .bind(&nhl_team)
            .execute(&mut *tx)
            .await?;
        }

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

    /// Insert a single player into the player pool.
    pub async fn insert_single_pool_player(
        &self,
        session_id: &str,
        nhl_id: i64,
        name: &str,
        position: &str,
        nhl_team: &str,
        headshot_url: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO player_pool (draft_session_id, nhl_id, name, position, nhl_team, headshot_url)
            VALUES ($1::uuid, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(session_id)
        .bind(nhl_id)
        .bind(name)
        .bind(position)
        .bind(nhl_team)
        .bind(headshot_url)
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

    /// Insert a sleeper pick and advance the sleeper pick index.
    pub async fn insert_sleeper_and_advance(
        &self,
        session_id: &str,
        team_id: i64,
        nhl_id: i64,
        name: &str,
        position: &str,
        nhl_team: &str,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;

        sqlx::query(
            r#"
            INSERT INTO fantasy_sleepers (team_id, nhl_id, name, position, nhl_team)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(team_id)
        .bind(nhl_id)
        .bind(name)
        .bind(position)
        .bind(nhl_team)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE draft_sessions SET sleeper_pick_index = sleeper_pick_index + 1 WHERE id = $1::uuid",
        )
        .bind(session_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Get undrafted players from the pool (eligible for sleeper picks).
    pub async fn get_undrafted_pool_players(
        &self,
        session_id: &str,
    ) -> Result<Vec<PlayerPoolRow>> {
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

    /// Get eligible sleeper picks: players in the pool who haven't been drafted
    /// or already picked as sleepers by any of the given teams.
    pub async fn get_eligible_sleepers(
        &self,
        session_id: &str,
        team_ids: &[i64],
    ) -> Result<Vec<PlayerPoolRow>> {
        if team_ids.is_empty() {
            return self.get_player_pool(session_id).await;
        }

        // Build placeholders for team_ids: $2, $3, $4, ...
        let placeholders = team_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("${}", i + 2))
            .collect::<Vec<_>>()
            .join(", ");

        let query_str = format!(
            r#"
            SELECT
                pp.id::text,
                pp.draft_session_id::text,
                pp.nhl_id,
                pp.name,
                pp.position,
                pp.nhl_team,
                pp.headshot_url
            FROM player_pool pp
            WHERE pp.draft_session_id = $1::uuid
              AND pp.nhl_id NOT IN (
                  SELECT fp.nhl_id FROM fantasy_players fp
                  WHERE fp.team_id IN ({placeholders})
              )
              AND pp.nhl_id NOT IN (
                  SELECT fs.nhl_id FROM fantasy_sleepers fs
              )
              AND pp.nhl_id NOT IN (
                  SELECT dp.nhl_id FROM draft_picks dp
                  WHERE dp.draft_session_id = $1::uuid
              )
            ORDER BY pp.name
            "#,
        );

        let mut query = sqlx::query_as::<_, PlayerPoolRow>(&query_str);
        query = query.bind(session_id);
        for team_id in team_ids {
            query = query.bind(team_id);
        }

        let players = query.fetch_all(self.pool()).await?;

        Ok(players)
    }
}
