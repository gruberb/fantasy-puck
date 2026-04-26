use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::postgres::PgPool;
use sqlx::Row;
use tracing::warn;

use crate::error::{Error, Result};

pub struct CacheService {
    pool: PgPool,
}

impl CacheService {
    pub fn new(pool: &PgPool) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn get_cached_response<T: DeserializeOwned>(
        &self,
        cache_key: &str,
    ) -> Result<Option<T>> {
        let query = r#"
            SELECT data FROM response_cache
            WHERE cache_key = $1
        "#;

        let row = sqlx::query(query)
            .bind(cache_key)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::Database)?;

        if let Some(row) = row {
            let data: String = row
                .try_get(0)
                .map_err(|e| Error::Internal(format!("Failed to read data from cache: {}", e)))?;

            // Treat deserialize failures as a cache miss. This self-heals
            // schema drift: when a DTO gains a new field (or changes shape),
            // stale rows from before the deploy can't match the new type —
            // returning None lets the caller regenerate and overwrite the
            // row with a fresh payload instead of 500ing every request.
            match serde_json::from_str::<T>(&data) {
                Ok(response) => Ok(Some(response)),
                Err(e) => {
                    warn!(
                        cache_key = %cache_key,
                        error = %e,
                        "Cached payload failed to deserialize; treating as miss"
                    );
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    pub async fn store_response<T: Serialize>(
        &self,
        cache_key: &str,
        date: &str,
        response: &T,
    ) -> Result<()> {
        let data = serde_json::to_string(response)
            .map_err(|e| Error::Internal(format!("Failed to serialize response: {}", e)))?;

        let now = Utc::now().to_rfc3339();

        let query = r#"
            INSERT INTO response_cache (cache_key, date, data, created_at, last_updated)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (cache_key) DO UPDATE SET
                date = EXCLUDED.date,
                data = EXCLUDED.data,
                last_updated = EXCLUDED.last_updated
        "#;

        sqlx::query(query)
            .bind(cache_key)
            .bind(date)
            .bind(&data)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(())
    }

    pub async fn update_last_updated(&self, cache_key: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();

        let query = r#"
            UPDATE response_cache
            SET last_updated = $1
            WHERE cache_key = $2
        "#;

        sqlx::query(query)
            .bind(&now)
            .bind(cache_key)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(())
    }

    pub async fn invalidate_cache(&self, cache_key: &str) -> Result<()> {
        let query = r#"
            DELETE FROM response_cache
            WHERE cache_key = $1
        "#;

        sqlx::query(query)
            .bind(cache_key)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(())
    }

    pub async fn invalidate_by_date(&self, date: &str) -> Result<()> {
        let query = r#"
            DELETE FROM response_cache
            WHERE date = $1
        "#;

        sqlx::query(query)
            .bind(date)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(())
    }

    pub async fn invalidate_all(&self) -> Result<()> {
        let query = r#"
            DELETE FROM response_cache
        "#;

        sqlx::query(query)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(())
    }
}

impl CacheService {
    /// Delete every row whose `cache_key` begins with `prefix`.
    pub async fn invalidate_by_prefix(&self, prefix: &str) -> Result<u64> {
        let query = "DELETE FROM response_cache WHERE cache_key LIKE $1";
        let pattern = format!("{}%", prefix);
        let result = sqlx::query(query)
            .bind(pattern)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;
        Ok(result.rows_affected())
    }

    /// Delete every row whose `cache_key` matches the SQL LIKE
    /// `pattern`. Used to target a narrow slice of the keyspace where a
    /// pure prefix would also wipe sibling keys the caller wants to
    /// keep — for example, the live poller wipes only the `:v2`
    /// narrative tail of the team-diagnosis family on game-end while
    /// leaving the `:bundle:v1` payload intact for the rest of the day.
    pub async fn invalidate_by_like(&self, pattern: &str) -> Result<u64> {
        let query = "DELETE FROM response_cache WHERE cache_key LIKE $1";
        let result = sqlx::query(query)
            .bind(pattern)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;
        Ok(result.rows_affected())
    }
}
