use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde_json::Value;
use tokio::sync::{RwLock, Semaphore};
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::models::nhl::{
    GameBoxscore, GameData, GameState, Player, PlayerGameLog, PlayoffCarousel, StatsLeaders,
    TodaySchedule,
};
use crate::nhl_api::nhl_constants as endpoints;
use crate::utils::nhl::calculate_totals_from_game_log;

/// A cached HTTP response with its expiration time.
struct CacheEntry {
    body: String,
    inserted_at: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.inserted_at.elapsed() > self.ttl
    }
}

// TTL constants for different NHL API endpoint types
mod ttl {
    use std::time::Duration;

    pub const SKATER_STATS: Duration = Duration::from_secs(300); // 5 min
    pub const SCHEDULE: Duration = Duration::from_secs(120); // 2 min
    pub const GAME_CENTER: Duration = Duration::from_secs(120); // 2 min
    pub const BOXSCORE_LIVE: Duration = Duration::from_secs(60); // 1 min
    pub const BOXSCORE_FINAL: Duration = Duration::from_secs(86400); // 24 hr
    pub const PLAYOFF_CAROUSEL: Duration = Duration::from_secs(900); // 15 min
    pub const PLAYER_GAME_LOG: Duration = Duration::from_secs(600); // 10 min
    pub const PLAYER_DETAILS: Duration = Duration::from_secs(1800); // 30 min
    pub const STANDINGS: Duration = Duration::from_secs(1800); // 30 min
    pub const ROSTER: Duration = Duration::from_secs(1800); // 30 min
    pub const EDGE: Duration = Duration::from_secs(1800); // 30 min
    pub const SCORES: Duration = Duration::from_secs(120); // 2 min
}

/// NHL API client with built-in rate limiting and in-memory response caching
#[derive(Clone)]
pub struct NhlClient {
    client: Client,
    semaphore: Arc<Semaphore>,
    cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl Default for NhlClient {
    fn default() -> Self {
        Self::new()
    }
}

impl NhlClient {
    /// Create a new NHL API client (max 5 concurrent requests, retry on 429)
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self {
            client,
            semaphore: Arc::new(Semaphore::new(5)),
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Spawn a background task that periodically removes expired cache entries
    pub fn start_cache_cleanup(&self, interval: Duration) {
        let cache = Arc::clone(&self.cache);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let mut cache = cache.write().await;
                let before = cache.len();
                cache.retain(|_, entry| !entry.is_expired());
                let after = cache.len();
                if before != after {
                    info!(
                        "NHL cache cleanup: removed {} expired entries ({} remaining)",
                        before - after,
                        after
                    );
                }
            }
        });
    }

    /// Clear all entries from the in-memory NHL API cache
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        let count = cache.len();
        cache.clear();
        info!("NHL API cache cleared ({} entries removed)", count);
    }

    // Fetch raw response body from NHL API with semaphore + retry logic
    async fn fetch_raw(&self, url: &str) -> Result<String> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| Error::NhlApi(format!("Semaphore error: {}", e)))?;

        let mut retries = 0;
        let max_retries = 3;

        loop {
            info!("Fetching from NHL API: {}", url);

            let response = self
                .client
                .get(url)
                .send()
                .await
                .map_err(|e| Error::NhlApi(format!("Request failed: {}", e)))?;

            if response.status() == 429 {
                retries += 1;
                if retries > max_retries {
                    return Err(Error::NhlApi(
                        "NHL API rate limit exceeded after retries".to_string(),
                    ));
                }
                let delay = Duration::from_millis(500 * retries);
                warn!("NHL API rate limited (429), retrying in {:?}...", delay);
                tokio::time::sleep(delay).await;
                continue;
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());

                return Err(Error::NhlApi(format!(
                    "NHL API returned status {}: {}",
                    status, error_text
                )));
            }

            return response
                .text()
                .await
                .map_err(|e| Error::NhlApi(format!("Failed to get response text: {}", e)));
        }
    }

    // Make a cached request: check in-memory cache first, fetch on miss
    async fn make_request_cached<T>(&self, url: &str, cache_ttl: Duration) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        // Check cache (read lock)
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(url) {
                if !entry.is_expired() {
                    return serde_json::from_str(&entry.body).map_err(|e| {
                        Error::NhlApi(format!("Cache deserialization error: {}", e))
                    });
                }
            }
        }

        // Cache miss or expired — fetch from NHL API
        let body = self.fetch_raw(url).await?;

        // Store in cache (write lock)
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                url.to_string(),
                CacheEntry {
                    body: body.clone(),
                    inserted_at: Instant::now(),
                    ttl: cache_ttl,
                },
            );
        }

        serde_json::from_str(&body)
            .map_err(|e| Error::NhlApi(format!("Failed to parse NHL API response: {}", e)))
    }

    /// Fetch skater stats leaders for a specific season and game type
    pub async fn get_skater_stats(&self, season: &u32, game_type: u8) -> Result<StatsLeaders> {
        let url = endpoints::stats::skater_stats_leaders(season, game_type);
        self.make_request_cached(&url, ttl::SKATER_STATS).await
    }

    /// Search for players by name, using team rosters
    pub async fn search_players(&self, query: &str) -> Result<Vec<Player>> {
        info!("Searching for players matching '{}'...", query);

        // Get all teams first
        let teams = self.get_all_teams().await?;
        let mut matching_players = Vec::new();

        let query_lower = query.to_lowercase();

        for team_abbrev in teams {
            match self.get_team_roster(&team_abbrev).await {
                Ok(players) => {
                    let team_matches: Vec<Player> = players
                        .into_iter()
                        .filter(|player| {
                            let first = player.first_name.get("default").cloned().unwrap_or_default();
                            let last = player.last_name.get("default").cloned().unwrap_or_default();
                            let full_name = format!("{} {}", first, last).to_lowercase();
                            full_name.contains(&query_lower)
                        })
                        .collect();

                    if !team_matches.is_empty() {
                        matching_players.extend(team_matches);
                    }
                }
                Err(e) => {
                    warn!("Could not fetch roster for {}: {}", team_abbrev, e);
                    continue;
                }
            }
        }

        Ok(matching_players)
    }

    /// Get all NHL teams
    pub async fn get_all_teams(&self) -> Result<Vec<String>> {
        let url = endpoints::teams::standings_url();
        let json: Value = self.make_request_cached(&url, ttl::STANDINGS).await?;

        let mut teams = Vec::new();

        // Extract team abbreviations from standings
        if let Some(standings) = json.get("standings").and_then(|s| s.as_array()) {
            for team in standings {
                if let Some(team_abbrev) = team
                    .get("teamAbbrev")
                    .and_then(|a| a.get("default"))
                    .and_then(|a| a.as_str())
                {
                    teams.push(team_abbrev.to_string());
                }
            }
        }

        Ok(teams)
    }

    /// Get team roster
    pub async fn get_team_roster(&self, team_abbrev: &str) -> Result<Vec<Player>> {
        let url = endpoints::players::team_roster(team_abbrev);
        let json: Value = self.make_request_cached(&url, ttl::ROSTER).await?;

        let mut players = Vec::new();

        // Process different player types (forwards, defensemen, goalies)
        let player_types = ["forwards", "defensemen", "goalies"];

        for player_type in &player_types {
            if let Some(roster_players) = json.get(player_type).and_then(|r| r.as_array()) {
                for player_json in roster_players {
                    let id = player_json
                        .get("id")
                        .and_then(|id| id.as_u64())
                        .unwrap_or(0) as u32;

                    // Skip if player ID is 0 (invalid)
                    if id == 0 {
                        continue;
                    }

                    // Extract firstName as HashMap
                    let mut first_name_map = std::collections::HashMap::new();
                    if let Some(first_name_obj) = player_json.get("firstName") {
                        if first_name_obj.is_object() {
                            for (key, value) in first_name_obj.as_object().unwrap() {
                                if let Some(name_str) = value.as_str() {
                                    first_name_map.insert(key.clone(), name_str.to_string());
                                }
                            }
                        }
                    }

                    // Extract lastName as HashMap
                    let mut last_name_map = std::collections::HashMap::new();
                    if let Some(last_name_obj) = player_json.get("lastName") {
                        if last_name_obj.is_object() {
                            for (key, value) in last_name_obj.as_object().unwrap() {
                                if let Some(name_str) = value.as_str() {
                                    last_name_map.insert(key.clone(), name_str.to_string());
                                }
                            }
                        }
                    }

                    let position = player_json
                        .get("positionCode")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string();

                    let jersey_number = player_json
                        .get("sweaterNumber")
                        .and_then(|n| n.as_u64())
                        .map(|n| n as u32);

                    players.push(Player {
                        id,
                        first_name: first_name_map,
                        last_name: last_name_map,
                        sweater_number: jersey_number,
                        team_abbrev: team_abbrev.to_string(),
                        position,
                        value: 0.0, // No stats value from roster endpoint
                    });
                }
            }
        }

        Ok(players)
    }

    /// Get today's NHL schedule
    pub async fn get_today_schedule(&self) -> Result<TodaySchedule> {
        let url = endpoints::games::today_schedule_url();
        self.make_request_cached(&url, ttl::SCHEDULE).await
    }

    /// Get schedule for a specific date
    pub async fn get_schedule_by_date(&self, date: &str) -> Result<TodaySchedule> {
        let url = endpoints::games::schedule_by_date(date);
        self.make_request_cached(&url, ttl::SCHEDULE).await
    }

    /// Get player image URL
    pub fn get_player_image_url(&self, player_id: i64) -> String {
        endpoints::players::player_image(player_id)
    }

    /// Get team logo URL
    pub fn get_team_logo_url(&self, team_abbrev: &str) -> String {
        endpoints::teams::team_logo(team_abbrev)
    }

    /// Gets player stats including a headshot image
    pub async fn get_player_details(&self, player_id: i64) -> Result<Player> {
        let url = endpoints::players::player_details(player_id);
        self.make_request_cached(&url, ttl::PLAYER_DETAILS).await
    }

    /// Get game scores
    pub async fn get_game_scores(&self, game_id: u32) -> Result<(Option<i32>, Option<i32>)> {
        let url = endpoints::games::game_center(game_id);
        let json: Value = self.make_request_cached(&url, ttl::GAME_CENTER).await?;

        // Extract scores - based on actual response structure
        let home_score = json
            .get("homeTeam")
            .and_then(|team| team.get("score"))
            .and_then(|score| score.as_i64())
            .map(|s| s as i32);

        let away_score = json
            .get("awayTeam")
            .and_then(|team| team.get("score"))
            .and_then(|score| score.as_i64())
            .map(|s| s as i32);

        Ok((home_score, away_score))
    }

    /// Get period information for a game
    pub async fn get_period_info(&self, game_id: u32) -> Result<Option<String>> {
        let url = endpoints::games::game_center(game_id);
        let json: Value = self.make_request_cached(&url, ttl::GAME_CENTER).await?;

        // Extract period information
        let period_descriptor = json.get("periodDescriptor");

        if let Some(period_data) = period_descriptor {
            let number = period_data
                .get("number")
                .and_then(|n| n.as_i64())
                .unwrap_or(0);
            let period_type = period_data.get("periodType").and_then(|t| t.as_str());

            let period_type_text = match period_type {
                Some("REG") => "Period",
                Some("OT") => "OT",
                Some("SO") => "Shootout",
                Some(other) => other,
                None => "",
            };

            return Ok(Some(format!("{} {}", number, period_type_text)));
        }

        Ok(None)
    }

    pub async fn get_game_data(&self, game_id: u32) -> Result<Option<GameData>> {
        let url = endpoints::games::game_center(game_id);
        let json: Value = self.make_request_cached(&url, ttl::GAME_CENTER).await?;

        // Extract game state
        let game_state_str = json
            .get("gameState")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN");

        // Parse game state
        let game_state: GameState = match game_state_str {
            "LIVE" => GameState::Live,
            "FINAL" => GameState::Final,
            "OFF" => GameState::Off,
            "CRIT" => GameState::Crit,
            "PRE" => GameState::Preview,
            "FUT" => GameState::Fut,
            _ => GameState::Unknown,
        };

        // Extract scores
        let home_score = json
            .get("homeTeam")
            .and_then(|team| team.get("score"))
            .and_then(|score| score.as_i64())
            .map(|s| s as i32);

        let away_score = json
            .get("awayTeam")
            .and_then(|team| team.get("score"))
            .and_then(|score| score.as_i64())
            .map(|s| s as i32);

        // Extract period information
        let period = if let Some(period_data) = json.get("periodDescriptor") {
            let number = period_data
                .get("number")
                .and_then(|n| n.as_i64())
                .unwrap_or(0);
            let period_type = period_data.get("periodType").and_then(|t| t.as_str());

            let period_type_text = match period_type {
                Some("REG") => "Period",
                Some("OT") => "OT",
                Some("SO") => "Shootout",
                Some(other) => other,
                None => "",
            };

            Some(format!("{} {}", number, period_type_text))
        } else {
            None
        };

        Ok(Some(GameData {
            game_state,
            home_score,
            away_score,
            period,
        }))
    }

    pub async fn get_playoff_carousel(&self, season: String) -> Result<Option<PlayoffCarousel>> {
        let url = endpoints::playoffs::carousel_for_season(season);
        self.make_request_cached(&url, ttl::PLAYOFF_CAROUSEL).await
    }

    /// Get the full landing/preview data for a game (matchup stats, goalies, etc.)
    pub async fn get_game_landing_raw(&self, game_id: u32) -> Result<serde_json::Value> {
        let url = endpoints::games::game_center(game_id);
        self.make_request_cached(&url, ttl::GAME_CENTER).await
    }

    /// Get boxscore with player stats for a game.
    /// Uses a short TTL for live games and a long TTL for finished games.
    pub async fn get_game_boxscore(&self, game_id: u32) -> Result<GameBoxscore> {
        let url = endpoints::games::game_boxscore(game_id);

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&url) {
                if !entry.is_expired() {
                    return serde_json::from_str(&entry.body).map_err(|e| {
                        Error::NhlApi(format!("Cache deserialization error: {}", e))
                    });
                }
            }
        }

        // Cache miss — fetch from NHL API
        let body = self.fetch_raw(&url).await?;

        // Determine TTL by inspecting game state in the response
        let cache_ttl = {
            let json: Value = serde_json::from_str(&body).unwrap_or_default();
            let game_state = json
                .get("gameState")
                .and_then(Value::as_str)
                .unwrap_or("");
            match game_state {
                "FINAL" | "OFF" => ttl::BOXSCORE_FINAL,
                _ => ttl::BOXSCORE_LIVE,
            }
        };

        // Store in cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                url.to_string(),
                CacheEntry {
                    body: body.clone(),
                    inserted_at: Instant::now(),
                    ttl: cache_ttl,
                },
            );
        }

        serde_json::from_str(&body)
            .map_err(|e| Error::NhlApi(format!("Failed to parse boxscore: {}", e)))
    }

    /// Get full standings data (raw JSON) -- includes streak, L10, conference rank, etc.
    pub async fn get_standings_raw(&self) -> Result<serde_json::Value> {
        let url = endpoints::teams::standings_url();
        self.make_request_cached(&url, ttl::STANDINGS).await
    }

    /// Get scores/results for a specific date (raw JSON)
    pub async fn get_scores_by_date(&self, date: &str) -> Result<serde_json::Value> {
        let url = endpoints::scores::scores_by_date(date);
        self.make_request_cached(&url, ttl::SCORES).await
    }

    /// Get NHL Edge analytics for a skater (skating speed, shot speed, etc.)
    pub async fn get_skater_edge_detail(&self, player_id: i64) -> Result<serde_json::Value> {
        let url = endpoints::edge::skater_detail(player_id);
        self.make_request_cached(&url, ttl::EDGE).await
    }

    pub fn get_team_name(&self, team_abbrev: &str) -> String {
        crate::nhl_api::nhl_constants::team_names::get_team_name(team_abbrev).to_string()
    }

    /// Get a player's game log for a specific season and game type
    pub async fn get_player_game_log(
        &self,
        player_id: i64,
        season: &u32,
        game_type: u8,
    ) -> Result<PlayerGameLog> {
        let url = endpoints::players::player_game_log(player_id, season, game_type);
        self.make_request_cached(&url, ttl::PLAYER_GAME_LOG).await
    }

    /// Helper method to calculate a player's form based on recent games
    /// Returns (goals, assists, points) in last n games
    pub async fn get_player_form(
        &self,
        player_id: i64,
        season: &u32,
        game_type: u8,
        num_games: usize,
    ) -> Result<(i32, i32, i32)> {
        let game_log = self
            .get_player_game_log(player_id, season, game_type)
            .await?;

        // Take last n games (or fewer if not enough games)
        let recent_games = game_log
            .game_log
            .iter()
            .rev() // Most recent games first
            .take(num_games)
            .collect::<Vec<_>>();

        if recent_games.is_empty() {
            return Ok((0, 0, 0));
        }

        // Calculate totals
        let recent_goals = recent_games.iter().map(|g| g.goals).sum();
        let recent_assists = recent_games.iter().map(|g| g.assists).sum();
        let recent_points = recent_games.iter().map(|g| g.points).sum();

        Ok((recent_goals, recent_assists, recent_points))
    }

    /// Check if a player is participating in the playoffs
    /// Returns (participating, goals, assists, points, games) tuple
    pub async fn check_player_in_playoffs(
        &self,
        player_id: i64,
        season: &u32,
        game_type: u8,
    ) -> Result<(bool, i32, i32, i32, i32)> {
        // Try to get player game log
        match self.get_player_game_log(player_id, season, game_type).await {
            Ok(game_log) => {
                // Is the player in the playoffs?
                let is_in_playoffs = !game_log.game_log.is_empty();

                // If player has playoff games
                if is_in_playoffs {
                    // Calculate totals from game log entries
                    let (goals, assists, points, games) =
                        calculate_totals_from_game_log(&game_log.game_log);
                    return Ok((true, goals, assists, points, games));
                }

                // Player not in playoffs
                Ok((false, 0, 0, 0, 0))
            }
            Err(e) => {
                // Log the error but don't fail the entire operation
                tracing::warn!("Error fetching game log for player {}: {}", player_id, e);
                Ok((false, 0, 0, 0, 0))
            }
        }
    }
}
