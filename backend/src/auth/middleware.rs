use std::sync::Arc;

use axum::{extract::FromRequestParts, http::request::Parts};

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

impl AuthUser {
    fn from_header(header: &str, state: &AppState) -> Result<Self, Error> {
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| Error::Unauthorized("Invalid authorization header format".into()))?;
        Self::from_token(token, state)
    }

    /// Validates a bare JWT. Used directly by the WebSocket upgrade,
    /// where browsers can't set an Authorization header.
    pub fn from_token(token: &str, state: &AppState) -> Result<Self, Error> {
        let claims = jwt::validate_token(token, &state.config.jwt_secret)?;
        Ok(AuthUser {
            id: claims.sub,
            email: claims.email,
            is_admin: claims.is_admin,
        })
    }
}

fn authorization_header(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
}

impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header = authorization_header(parts)
            .ok_or_else(|| Error::Unauthorized("Missing authorization header".into()))?;
        AuthUser::from_header(header, state)
    }
}

/// Optional auth: `None` without a token, error only on an invalid one.
#[derive(Debug, Clone)]
pub struct OptionalAuth(pub Option<AuthUser>);

impl FromRequestParts<Arc<AppState>> for OptionalAuth {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        authorization_header(parts)
            .map(|h| AuthUser::from_header(h, state))
            .transpose()
            .map(OptionalAuth)
    }
}

/// An authenticated user with the global admin flag; rejects everyone else
/// with 403.
#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthUser);

impl FromRequestParts<Arc<AppState>> for AdminUser {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin {
            return Err(Error::Forbidden("Admin access required".into()));
        }
        Ok(AdminUser(user))
    }
}
