use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::str::FromStr;

use crate::error::{Error, Result};

pub mod cache;
pub mod draft;
pub mod league_stats;
pub mod leagues;
pub mod nhl_mirror;
pub mod players;
mod sleepers;
pub mod teams;
pub mod users;

/// Inclusive date window for queries that aggregate across history.
/// Either bound may be `None` to leave that side unbounded. Built by
/// handlers from `crate::api::playoff_start()` / `season_end()` so
/// aggregations in playoff mode don't see regular-season rows.
#[derive(Clone, Copy, Debug, Default)]
pub struct DateWindow<'a> {
    pub min_date: Option<&'a str>,
    pub max_date: Option<&'a str>,
}

impl<'a> DateWindow<'a> {
    pub fn unbounded() -> Self {
        Self { min_date: None, max_date: None }
    }

    pub fn since(min_date: &'a str) -> Self {
        Self { min_date: Some(min_date), max_date: None }
    }

    pub fn between(min_date: &'a str, max_date: &'a str) -> Self {
        Self { min_date: Some(min_date), max_date: Some(max_date) }
    }
}

/// Database interaction for fantasy teams
#[derive(Clone)]
pub struct FantasyDb {
    pool: PgPool,
}

impl FantasyDb {
    pub async fn new(db_url: &str) -> Result<Self> {
        // Use session pooler (port 5432) which supports prepared statements,
        // but still set cache to 0 for safety with PgBouncer
        let connect_options = PgConnectOptions::from_str(db_url)
            .map_err(|e| Error::Internal(format!("Invalid DATABASE_URL: {}", e)))?
            .statement_cache_capacity(0);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
            .await
            .map_err(Error::Database)?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Lightweight health check — verifies the pool can acquire a connection.
    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(Error::Database)
    }

    // Create a helper method to access the cache service
    pub fn cache(&self) -> cache::CacheService {
        cache::CacheService::new(&self.pool)
    }

    // --- League methods ---

    pub async fn get_all_leagues(&self) -> Result<Vec<crate::domain::models::db::League>> {
        let leagues = sqlx::query_as::<_, crate::domain::models::db::League>(
            "SELECT id::text, name, season, visibility, created_by::text FROM leagues ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(leagues)
    }

    pub async fn get_public_leagues(&self) -> Result<Vec<crate::domain::models::db::League>> {
        let leagues = sqlx::query_as::<_, crate::domain::models::db::League>(
            "SELECT id::text, name, season, visibility, created_by::text FROM leagues WHERE visibility = 'public' ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(leagues)
    }

    /// Get all league IDs (used by scheduler to iterate over leagues)
    pub async fn get_all_league_ids(&self) -> Result<Vec<String>> {
        let ids: Vec<String> =
            sqlx::query_scalar("SELECT id::text FROM leagues")
                .fetch_all(&self.pool)
                .await?;

        Ok(ids)
    }

    /// Get the league_id for a draft session. Useful for authorization checks.
    pub async fn get_league_id_for_draft(&self, draft_id: &str) -> Result<String> {
        let league_id: String = sqlx::query_scalar(
            "SELECT league_id::text FROM draft_sessions WHERE id = $1::uuid",
        )
        .bind(draft_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(league_id)
    }

    /// Get the league_id that a fantasy team belongs to.
    pub async fn get_league_id_for_team(&self, team_id: i64) -> Result<String> {
        let league_id: String = sqlx::query_scalar(
            "SELECT league_id::text FROM fantasy_teams WHERE id = $1",
        )
        .bind(team_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound("Team not found".into()))?;

        Ok(league_id)
    }

    /// Get the league_id for the team that owns a given player.
    pub async fn get_league_id_for_player(&self, player_id: i64) -> Result<String> {
        let league_id: String = sqlx::query_scalar(
            r#"
            SELECT ft.league_id::text
            FROM fantasy_players fp
            JOIN fantasy_teams ft ON ft.id = fp.team_id
            WHERE fp.id = $1
            "#,
        )
        .bind(player_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::NotFound("Player not found".into()))?;

        Ok(league_id)
    }

    // --- Team methods (delegate to TeamDbService) ---

    pub async fn get_team(
        &self,
        team_id: i64,
        league_id: &str,
    ) -> Result<crate::domain::models::db::FantasyTeam> {
        teams::TeamDbService::new(&self.pool)
            .get_team(team_id, league_id)
            .await
    }

    pub async fn get_all_teams(
        &self,
        league_id: &str,
    ) -> Result<Vec<crate::domain::models::db::FantasyTeam>> {
        teams::TeamDbService::new(&self.pool)
            .get_all_teams(league_id)
            .await
    }

    pub async fn get_fantasy_bets_by_nhl_team(
        &self,
        league_id: &str,
    ) -> Result<Vec<crate::domain::models::db::FantasyTeamBets>> {
        teams::TeamDbService::new(&self.pool)
            .get_fantasy_bets_by_nhl_team(league_id)
            .await
    }

    pub async fn get_fantasy_teams_for_nhl_teams(
        &self,
        nhl_teams: &[&str],
        league_id: &str,
    ) -> Result<Vec<crate::domain::models::fantasy::FantasyTeamInGame>> {
        teams::TeamDbService::new(&self.pool)
            .get_fantasy_teams_for_nhl_teams(nhl_teams, league_id)
            .await
    }

    pub async fn update_team_name(&self, team_id: i64, name: &str) -> Result<()> {
        teams::TeamDbService::new(&self.pool)
            .update_team_name(team_id, name)
            .await
    }

    pub async fn get_team_sparklines(
        &self,
        league_id: &str,
        days: i32,
        min_date: &str,
    ) -> Result<std::collections::HashMap<i64, Vec<i32>>> {
        teams::TeamDbService::new(&self.pool)
            .get_team_sparklines(league_id, days, min_date)
            .await
    }

    /// Same shape as `get_team_sparklines` but unioned with
    /// `v_daily_fantasy_totals` so today's running total shows up
    /// in the chart before the nightly/afternoon cron rolls it into
    /// `daily_rankings`. Used by Pulse.
    pub async fn get_team_sparklines_with_live(
        &self,
        league_id: &str,
        days: i32,
        min_date: &str,
    ) -> Result<std::collections::HashMap<i64, Vec<i32>>> {
        teams::TeamDbService::new(&self.pool)
            .get_team_sparklines_with_live(league_id, days, min_date)
            .await
    }

    pub async fn get_all_teams_with_players(
        &self,
        league_id: &str,
    ) -> Result<Vec<crate::domain::models::fantasy::FantasyTeamInGame>> {
        teams::TeamDbService::new(&self.pool)
            .get_all_teams_with_players(league_id)
            .await
    }

    // --- Player methods (delegate to PlayerDbService) ---

    pub async fn add_player_to_team(
        &self,
        team_id: i64,
        nhl_id: i64,
        name: &str,
        position: &str,
        nhl_team: &str,
    ) -> Result<crate::domain::models::db::FantasyPlayer> {
        players::PlayerDbService::new(&self.pool)
            .add_player_to_team(team_id, nhl_id, name, position, nhl_team)
            .await
    }

    pub async fn remove_player(&self, player_id: i64) -> Result<()> {
        players::PlayerDbService::new(&self.pool)
            .remove_player(player_id)
            .await
    }

    pub async fn get_team_players(
        &self,
        team_id: i64,
    ) -> Result<Vec<crate::domain::models::db::FantasyPlayer>> {
        players::PlayerDbService::new(&self.pool)
            .get_team_players(team_id)
            .await
    }

    pub async fn get_nhl_teams_and_players(
        &self,
        league_id: &str,
    ) -> Result<Vec<crate::domain::models::db::NhlTeamPlayers>> {
        players::PlayerDbService::new(&self.pool)
            .get_nhl_teams_and_players(league_id)
            .await
    }

    pub async fn get_fantasy_players_for_nhl_teams(
        &self,
        nhl_teams: &[&str],
        league_id: &str,
    ) -> Result<Vec<crate::domain::models::fantasy::FantasyTeamInGame>> {
        players::PlayerDbService::new(&self.pool)
            .get_fantasy_players_for_nhl_teams(nhl_teams, league_id)
            .await
    }

    // --- Sleeper methods (delegate to SleeperDbService) ---

    pub async fn get_all_sleepers(
        &self,
        league_id: &str,
    ) -> Result<Vec<crate::domain::models::db::FantasySleeper>> {
        sleepers::SleeperDbService::new(&self.pool)
            .get_all_sleepers(league_id)
            .await
    }

    pub async fn remove_sleeper(&self, sleeper_id: i64) -> Result<()> {
        sleepers::SleeperDbService::new(&self.pool)
            .remove_sleeper(sleeper_id)
            .await
    }

    pub async fn get_daily_ranking_stats(
        &self,
        league_id: &str,
        window: DateWindow<'_>,
    ) -> Result<Vec<crate::domain::models::db::TeamDailyRankingStats>> {
        teams::TeamDbService::new(&self.pool)
            .get_daily_ranking_stats(league_id, window)
            .await
    }

    /// Read the cached playoff roster pool (16-team blob) if present.
    /// Returns the JSONB payload; callers deserialize into their own shape.
    pub async fn get_playoff_roster_cache(
        &self,
        season: i32,
        game_type: i16,
    ) -> Result<Option<serde_json::Value>> {
        let row: Option<serde_json::Value> = sqlx::query_scalar(
            "SELECT rosters FROM playoff_roster_cache WHERE season = $1 AND game_type = $2",
        )
        .bind(season)
        .bind(game_type)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Upsert the cached playoff roster pool.
    pub async fn upsert_playoff_roster_cache(
        &self,
        season: i32,
        game_type: i16,
        rosters: &serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO playoff_roster_cache (season, game_type, rosters, updated_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (season, game_type) DO UPDATE
              SET rosters = EXCLUDED.rosters,
                  updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(season)
        .bind(game_type)
        .bind(rosters)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
