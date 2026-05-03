use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const TOKEN_TTL_DAYS: usize = 90;

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

/// Issue a JWT with a 90-day expiry.
pub fn issue_token(
    user_id: &str,
    email: &str,
    is_admin: bool,
    secret: &SecretString,
) -> Result<String> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        is_admin,
        exp: now + TOKEN_TTL_DAYS * 24 * 60 * 60,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.expose_secret().as_bytes()),
    )
    .map_err(|e| Error::Internal(format!("Failed to issue JWT: {e}")))
}

/// Validate a JWT and return the claims.
pub fn validate_token(token: &str, secret: &SecretString) -> Result<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.expose_secret().as_bytes()),
        &Validation::new(jsonwebtoken::Algorithm::HS256),
    )
    .map(|data| data.claims)
    .map_err(|e| Error::Unauthorized(format!("Invalid token: {e}")))
}
