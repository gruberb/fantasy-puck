use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Fantasy team player
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FantasyPlayer {
    pub id: i64,
    pub team_id: i64,
    pub nhl_id: i64,
    pub name: String,
    pub position: String,
    pub nhl_team: String,
}

pub struct NhlTeamPlayers {
    pub nhl_team: String,
    pub players: Vec<PlayerWithTeam>,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct PlayerWithTeam {
    pub nhl_id: i64,
    pub name: String,
    pub fantasy_team_id: i64,
    pub fantasy_team_name: String,
    pub position: String,
    pub nhl_team: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct TeamNhlCount {
    pub team_id: i64,
    pub team_name: String,
    pub nhl_team: String,
    pub num_players: i64,
}

/// Fantasy team
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct FantasyTeam {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FantasyTeamWithPlayers {
    pub id: i64,
    pub name: String,
    pub players: Vec<FantasyPlayer>,
}

pub struct FantasyTeamBets {
    pub team_id: i64,
    pub team_name: String,
    pub bets: Vec<NhlBetCount>,
}

pub struct NhlBetCount {
    pub nhl_team: String,
    pub num_players: i64,
}

/// Fantasy sleeper
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct FantasySleeper {
    pub id: i64,
    pub team_id: Option<i64>,
    pub nhl_id: i64,
    pub name: String,
    pub position: String,
    pub nhl_team: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct TeamDailyRankingStats {
    pub team_id: i64,
    pub wins: i32,
    pub top_three: i32,
    // Array of dates when team ranked #1
    pub win_dates: Vec<String>,
    // Array of dates when team was in top 3
    pub top_three_dates: Vec<String>,
}

/// League
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct League {
    pub id: String,
    pub name: String,
    pub season: String,
    pub visibility: String,
    pub created_by: Option<String>,
}
