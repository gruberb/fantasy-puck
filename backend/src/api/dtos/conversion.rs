use crate::api::dtos::PlayerHighlightResponse;
use crate::api::dtos::rankings::DailyFantasyRankingResponse;
use crate::domain::models::fantasy::{DailyRanking, PlayerHighlight};

pub trait IntoResponse {
    type Response;
    fn into_response(self) -> Self::Response;
}

impl IntoResponse for PlayerHighlight {
    type Response = PlayerHighlightResponse;

    fn into_response(self) -> PlayerHighlightResponse {
        PlayerHighlightResponse {
            player_name: self.player_name,
            points: self.points,
            nhl_team: self.nhl_team,
            nhl_id: self.nhl_id,
            image_url: crate::infra::nhl::constants::players::player_image(self.nhl_id),
        }
    }
}

impl IntoResponse for DailyRanking {
    type Response = DailyFantasyRankingResponse;

    fn into_response(self) -> DailyFantasyRankingResponse {
        DailyFantasyRankingResponse {
            rank: self.rank,
            team_id: self.team_id,
            team_name: self.team_name,
            daily_points: self.daily_points,
            daily_assists: self.daily_assists,
            daily_goals: self.daily_goals,
            player_highlights: self
                .player_highlights
                .into_iter()
                .map(|p| p.into_response())
                .collect(),
        }
    }
}
