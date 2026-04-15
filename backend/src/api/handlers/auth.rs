use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::api::response::{json_success, ApiResponse};
use crate::api::routes::AppState;
use crate::auth::jwt::issue_token;
use crate::auth::middleware::AuthUser;
use crate::auth::password::{hash_password, verify_password};
use crate::db::users::MembershipRow;
use crate::error::{Error, Result};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
    pub profile: ProfileResponse,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: String,
    pub email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResponse {
    pub display_name: String,
    pub is_admin: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeResponse {
    pub user: UserResponse,
    pub profile: ProfileResponse,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    pub display_name: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /api/auth/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>> {
    let user = state
        .db
        .get_user_by_email(&body.email)
        .await?
        .ok_or_else(|| Error::Unauthorized("Invalid email or password".into()))?;

    let valid = verify_password(&body.password, &user.password_hash).await?;
    if !valid {
        return Err(Error::Unauthorized("Invalid email or password".into()));
    }

    let profile = state.db.get_profile(&user.id).await?;
    let token = issue_token(&user.id, &user.email, profile.is_admin, &state.config.jwt_secret)?;

    Ok(json_success(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            email: user.email,
        },
        profile: ProfileResponse {
            display_name: profile.display_name,
            is_admin: profile.is_admin,
        },
    }))
}

/// POST /api/auth/register
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>> {
    // Check if user already exists
    if state.db.get_user_by_email(&body.email).await?.is_some() {
        return Err(Error::Conflict("Email already registered".into()));
    }

    let hashed = hash_password(&body.password).await?;
    let user = state.db.create_user(&body.email, &hashed).await?;
    state
        .db
        .create_profile(&user.id, &body.display_name)
        .await?;

    let profile = state.db.get_profile(&user.id).await?;
    let token = issue_token(&user.id, &user.email, profile.is_admin, &state.config.jwt_secret)?;

    Ok(json_success(AuthResponse {
        token,
        user: UserResponse {
            id: user.id,
            email: user.email,
        },
        profile: ProfileResponse {
            display_name: profile.display_name,
            is_admin: profile.is_admin,
        },
    }))
}

/// GET /api/auth/me
pub async fn get_me(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<MeResponse>>> {
    let user = state.db.get_user_by_id(&auth_user.id).await?;
    let profile = state.db.get_profile(&auth_user.id).await?;

    Ok(json_success(MeResponse {
        user: UserResponse {
            id: user.id,
            email: user.email,
        },
        profile: ProfileResponse {
            display_name: profile.display_name,
            is_admin: profile.is_admin,
        },
    }))
}

/// PUT /api/auth/profile
pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<Json<ApiResponse<()>>> {
    state
        .db
        .update_profile(&auth_user.id, &body.display_name)
        .await?;
    Ok(json_success(()))
}

/// DELETE /api/auth/account
pub async fn delete_account(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<()>>> {
    state.db.delete_user_account(&auth_user.id).await?;
    Ok(json_success(()))
}

/// GET /api/auth/memberships
pub async fn get_memberships(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<Vec<MembershipRow>>>> {
    let memberships = state.db.get_user_memberships(&auth_user.id).await?;
    Ok(json_success(memberships))
}
