use std::sync::Arc;

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};

use crate::api::routes::AppState;
use crate::auth::jwt;
use crate::error::Error;

/// Authenticated user extracted from the Authorization header.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub is_admin: bool,
}

/// Extract and validate the Bearer token from the request.
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Error::Unauthorized("Missing authorization header".into()))?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| Error::Unauthorized("Invalid authorization header format".into()))?;

        let claims = jwt::validate_token(token, &state.config.jwt_secret)?;

        Ok(AuthUser {
            id: claims.sub,
            email: claims.email,
            is_admin: claims.is_admin,
        })
    }
}

/// Optional auth — returns None if no token is provided, errors only on invalid tokens.
#[derive(Debug, Clone)]
pub struct OptionalAuth(pub Option<AuthUser>);

impl FromRequestParts<Arc<AppState>> for OptionalAuth {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok());

        match header {
            None => Ok(OptionalAuth(None)),
            Some(h) => {
                let token = h
                    .strip_prefix("Bearer ")
                    .ok_or_else(|| Error::Unauthorized("Invalid authorization header".into()))?;
                let claims = jwt::validate_token(token, &state.config.jwt_secret)?;
                Ok(OptionalAuth(Some(AuthUser {
                    id: claims.sub,
                    email: claims.email,
                    is_admin: claims.is_admin,
                })))
            }
        }
    }
}
