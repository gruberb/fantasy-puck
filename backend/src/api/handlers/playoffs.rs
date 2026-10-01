use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use crate::api::dtos::PlayoffCarouselResponse;
use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::error::Result;

pub async fn get_playoff_info(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ApiResponse<PlayoffCarouselResponse>>> {
    let season = match params.get("season") {
        Some(date) => {
            // Validate date format YYYYYYYY (e.g. 20242025)
            if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
                return Err(crate::error::Error::Validation(
                    "Invalid season format. Use YYYYYYYY (20242025)".into(),
                ));
            }
            date
        }
        None => {
            return Err(crate::error::Error::Validation(
                "Season parameter is required (format: season=20242025)".into(),
            ));
        }
    };

    let carousel = state
        .nhl_client
        .get_playoff_carousel(season.clone())
        .await?
        .ok_or_else(|| {
            crate::error::Error::NotFound(
                "Cannot find the Playoff Season you are looking for".to_string(),
            )
        })?;

    Ok(json_success(PlayoffCarouselResponse::from(carousel)))
}
