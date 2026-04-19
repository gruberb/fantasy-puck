use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::postgres::PgPool;
use sqlx::Row;

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

            let response: T = serde_json::from_str(&data).map_err(|e| {
                Error::Internal(format!("Failed to deserialize cached data: {}", e))
            })?;

            Ok(Some(response))
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
    /// Delete every row whose `cache_key` begins with `prefix`. Used
    /// by the live poller to invalidate all `pulse_narrative:{league}:*`
    /// entries when a game a rostered player is in transitions to
    /// FINAL.
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
}
