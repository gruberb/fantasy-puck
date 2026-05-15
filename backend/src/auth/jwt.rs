use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use secrecy::{ExposeSecret, SecretString};
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
    /// Issued at (unix timestamp)
    pub iat: usize,
}

/// Issue a JWT that remains valid until explicit sign-out or secret rotation.
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
        &session_validation(),
    )
    .map(|data| data.claims)
    .map_err(|e| Error::Unauthorized(format!("Invalid token: {e}")))
}

fn session_validation() -> Validation {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    validation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct LegacyClaims<'a> {
        sub: &'a str,
        email: &'a str,
        is_admin: bool,
        iat: usize,
        exp: usize,
    }

    #[test]
    fn issued_tokens_validate_without_expiry() {
        let secret = SecretString::from("test-secret".to_string());

        let token = issue_token("user-1", "user@example.com", true, &secret).unwrap();
        let claims = validate_token(&token, &secret).unwrap();

        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.email, "user@example.com");
        assert!(claims.is_admin);
    }

    #[test]
    fn legacy_expired_tokens_remain_valid() {
        let secret = SecretString::from("test-secret".to_string());
        let token = encode(
            &Header::default(),
            &LegacyClaims {
                sub: "user-1",
                email: "user@example.com",
                is_admin: false,
                iat: 1,
                exp: 2,
            },
            &EncodingKey::from_secret(secret.expose_secret().as_bytes()),
        )
        .unwrap();

        let claims = validate_token(&token, &secret).unwrap();

        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.email, "user@example.com");
        assert!(!claims.is_admin);
    }
}
