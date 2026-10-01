use std::fmt::Display;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::error;

/// Application error. Client-facing variants carry the message returned in
/// the response body; server-side variants are logged in full but never
/// exposed to the client.
///
/// Server-side variants render their whole cause chain in `Display` (via
/// anyhow's `{:#}`) and deliberately expose no `source()`, so the many
/// `warn!("...: {}", e)` log sites get the full story without a reporter
/// printing it twice.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database query failed: {0}")]
    Database(sqlx::Error),
    #[error("NHL API request failed: {0:#}")]
    NhlApi(anyhow::Error),
    #[error("internal error: {0:#}")]
    Internal(anyhow::Error),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    Conflict(String),
}

impl Error {
    pub fn internal(msg: impl Display + std::fmt::Debug + Send + Sync + 'static) -> Self {
        Self::Internal(anyhow::Error::msg(msg))
    }

    pub fn nhl_api(msg: impl Display + std::fmt::Debug + Send + Sync + 'static) -> Self {
        Self::NhlApi(anyhow::Error::msg(msg))
    }

    /// Adds context to the source chain while keeping the variant, so the
    /// HTTP status stays what the original failure classified it as.
    /// Client-facing and `Database` variants have no chain to extend and
    /// are returned unchanged.
    pub fn context(self, ctx: impl Display + Send + Sync + 'static) -> Self {
        match self {
            Self::NhlApi(e) => Self::NhlApi(e.context(ctx)),
            Self::Internal(e) => Self::Internal(e.context(ctx)),
            other => other,
        }
    }
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match &self {
            Error::Database(_) | Error::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::NhlApi(_) => StatusCode::BAD_GATEWAY,
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::Validation(_) => StatusCode::BAD_REQUEST,
            Error::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Error::Forbidden(_) => StatusCode::FORBIDDEN,
            Error::Conflict(_) => StatusCode::CONFLICT,
        };

        let public_message = match &self {
            Error::Database(_) => Some("Database error occurred"),
            Error::NhlApi(_) => Some("External service error"),
            Error::Internal(_) => Some("Internal server error"),
            _ => None,
        };
        let message = match public_message {
            Some(public) => {
                error!(
                    error = %self,
                    "request failed"
                );
                public.to_string()
            }
            None => self.to_string(),
        };

        let body = Json(ErrorResponse {
            success: false,
            error: message,
        });
        (status, body).into_response()
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => Error::NotFound("Resource not found".into()),
            // invalid_text_representation: a malformed id from the URL hit a
            // `::uuid` cast. That's bad input, not a server fault.
            sqlx::Error::Database(db) if db.code().as_deref() == Some("22P02") => {
                Error::Validation("Invalid identifier".into())
            }
            _ => Error::Database(err),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::NhlApi(err.into())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Internal(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Internal(anyhow::Error::new(err).context("JSON (de)serialization failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_renders_chain_once() {
        let err = Error::from(anyhow::anyhow!("root cause").context("loading x"));
        assert_eq!(err.to_string(), "internal error: loading x: root cause");
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn context_keeps_variant() {
        let err = Error::nhl_api("status 503").context("fetching standings");
        assert!(matches!(err, Error::NhlApi(_)));
        assert_eq!(err.into_response().status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn row_not_found_maps_to_404() {
        let status = Error::from(sqlx::Error::RowNotFound)
            .into_response()
            .status();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn client_errors_expose_their_message() {
        let err = Error::Validation("bad team id".into());
        assert_eq!(err.to_string(), "bad team id");
        assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
    }
}
