use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use serde_json::{from_value, to_value};

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

    let raw = state
        .nhl_client
        .get_playoff_carousel(season.clone())
        .await
        .map_err(|_| {
            crate::error::Error::NotFound(
                "Cannot find the Playoff Season you are looking for".to_string(),
            )
        })?;

    let val = to_value(raw)
        .map_err(|e| crate::error::Error::Internal(format!("serialization error: {}", e)))?;
    let slim: PlayoffCarouselResponse = from_value(val)
        .map_err(|e| crate::error::Error::Internal(format!("conversion error: {}", e)))?;

    Ok(json_success(slim.with_computed_state()))
}
