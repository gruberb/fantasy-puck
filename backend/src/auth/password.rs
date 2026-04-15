use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::error::{Error, Result};

/// Hash a password with argon2.
/// Runs on a blocking thread to avoid stalling the async runtime.
pub async fn hash_password(plain: &str) -> Result<String> {
    let plain = plain.to_string();
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        argon2
            .hash_password(plain.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| Error::Internal(format!("Failed to hash password: {e}")))
    })
    .await
    .map_err(|e| Error::Internal(format!("Password hashing task failed: {e}")))?
}

/// Verify a password against an argon2 hash.
/// Runs on a blocking thread to avoid stalling the async runtime.
pub async fn verify_password(plain: &str, hash: &str) -> Result<bool> {
    let plain = plain.to_string();
    let hash = hash.to_string();
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&hash)
            .map_err(|e| Error::Internal(format!("Invalid password hash: {e}")))?;
        Ok(Argon2::default()
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|e| Error::Internal(format!("Password verification task failed: {e}")))?
}
