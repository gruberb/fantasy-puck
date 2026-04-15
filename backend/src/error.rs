use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::{error::Error as StdError, fmt};
use tracing::error;

#[derive(Debug)]
pub enum Error {
    Database(sqlx::Error),
    NhlApi(String),
    NotFound(String),
    Validation(String),
    Internal(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
}

// Implement std::error::Error
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Database(err) => Some(err),
            _ => None,
        }
    }
}

// Custom Display implementation
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Database(err) => write!(f, "Database error: {}", err),
            Error::NhlApi(msg) => write!(f, "NHL API error: {}", msg),
            Error::NotFound(msg) => write!(f, "Not found: {}", msg),
            Error::Validation(msg) => write!(f, "Validation error: {}", msg),
            Error::Internal(msg) => write!(f, "Internal error: {}", msg),
            Error::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
            Error::Forbidden(msg) => write!(f, "Forbidden: {}", msg),
            Error::Conflict(msg) => write!(f, "Conflict: {}", msg),
        }
    }
}

// For client responses
#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

// Convert our Error into axum HTTP responses
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            Error::Database(err) => {
                error!("Database error: {}", err);
                (StatusCode::INTERNAL_SERVER_ERROR, "Database error occurred")
            }
            Error::NhlApi(msg) => {
                error!("NHL API error: {}", msg);
                (StatusCode::BAD_GATEWAY, "External service error")
            }
            Error::NotFound(msg) => (StatusCode::NOT_FOUND, msg.as_str()),
            Error::Validation(msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
            Error::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.as_str()),
            Error::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.as_str()),
            Error::Conflict(msg) => (StatusCode::CONFLICT, msg.as_str()),
            Error::Internal(msg) => {
                error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };

        // Create a proper error response
        let body = Json(ErrorResponse {
            success: false,
            error: error_message.to_string(),
        });

        (status, body).into_response()
    }
}

// Convenient type alias
pub type Result<T> = std::result::Result<T, Error>;

// From implementations for easy conversion
impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => Error::NotFound("Resource not found".into()),
            _ => Error::Database(err),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::NhlApi(err.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Internal(format!("JSON parsing error: {}", err))
    }
}
