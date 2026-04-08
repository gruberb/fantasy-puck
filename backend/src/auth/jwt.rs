use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// User ID (UUID string)
    pub sub: String,
    /// Email
    pub email: String,
    /// Admin flag
    pub is_admin: bool,
    /// Expiry (unix timestamp)
    pub exp: usize,
    /// Issued at (unix timestamp)
    pub iat: usize,
}

/// Issue a JWT with a 7-day expiry.
pub fn issue_token(
    user_id: &str,
    email: &str,
    is_admin: bool,
    secret: &str,
) -> Result<String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        is_admin,
        exp: now + 7 * 24 * 60 * 60, // 7 days
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| Error::Internal(format!("Failed to issue JWT: {e}")))
}

/// Validate a JWT and return the claims.
pub fn validate_token(token: &str, secret: &str) -> Result<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| Error::Unauthorized(format!("Invalid token: {e}")))
}
