use cookbook_domain::user::models::UserId;
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqliteRow};

use crate::auth::models::{SessionRecord, SessionTokenHash};
use crate::auth::ports::{SessionRepository, SessionRepositoryError};
use crate::storage::time;

#[derive(Debug, Clone)]
pub struct SqliteSessionRepository {
    pool: SqlitePool,
}

impl SqliteSessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl SessionRepository for SqliteSessionRepository {
    async fn find_session(
        &self,
        token_hash: &SessionTokenHash,
    ) -> Result<Option<SessionRecord>, SessionRepositoryError> {
        let row = sqlx::query("SELECT user_id, expires_at_ms FROM sessions WHERE token_hash = ?")
            .bind(&token_hash.as_bytes()[..])
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;

        row.map(|row| decode_session(&row, token_hash))
            .transpose()
            .map_err(SessionRepositoryError::Storage)
    }

    async fn save_session(&self, session: SessionRecord) -> Result<(), SessionRepositoryError> {
        let expires_at_ms =
            time::to_millis(session.expires_at()).map_err(SessionRepositoryError::Storage)?;

        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, expires_at_ms)
             VALUES (?, ?, ?)
             ON CONFLICT(token_hash) DO UPDATE SET
                 user_id = excluded.user_id,
                 expires_at_ms = excluded.expires_at_ms",
        )
        .bind(&session.token_hash().as_bytes()[..])
        .bind(session.user_id().as_str())
        .bind(expires_at_ms)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;

        Ok(())
    }

    async fn delete_session(
        &self,
        token_hash: &SessionTokenHash,
    ) -> Result<(), SessionRepositoryError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&token_hash.as_bytes()[..])
            .execute(&self.pool)
            .await
            .map_err(storage_error)?;

        Ok(())
    }
}

fn decode_session(row: &SqliteRow, token_hash: &SessionTokenHash) -> anyhow::Result<SessionRecord> {
    let user_id: String = row.try_get("user_id")?;
    let expires_at_ms: i64 = row.try_get("expires_at_ms")?;

    Ok(SessionRecord::new(
        token_hash.clone(),
        UserId::new(user_id),
        time::from_millis(expires_at_ms)?,
    ))
}

fn storage_error(error: sqlx::Error) -> SessionRepositoryError {
    SessionRepositoryError::Storage(error.into())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use cookbook_domain::user::models::UserId;

    use super::*;
    use crate::storage::SqliteDatabase;

    async fn repository() -> SqliteSessionRepository {
        let database = SqliteDatabase::open_in_memory().await.unwrap();

        SqliteSessionRepository::new(database.pool().clone())
    }

    #[tokio::test]
    async fn saves_finds_and_deletes_sessions() {
        let repository = repository().await;
        let hash = SessionTokenHash::new([1; 32]);
        let expires_at = SystemTime::UNIX_EPOCH + Duration::from_secs(3_600);
        let record =
            SessionRecord::new(hash.clone(), UserId::new("user-1".to_string()), expires_at);

        repository.save_session(record).await.unwrap();

        let stored = repository.find_session(&hash).await.unwrap().unwrap();
        assert_eq!(stored.user_id().as_str(), "user-1");
        assert_eq!(stored.expires_at(), expires_at);

        repository.delete_session(&hash).await.unwrap();

        assert!(repository.find_session(&hash).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn returns_none_for_an_unknown_session() {
        let repository = repository().await;

        assert!(
            repository
                .find_session(&SessionTokenHash::new([9; 32]))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn saving_an_existing_token_updates_the_session() {
        let repository = repository().await;
        let hash = SessionTokenHash::new([2; 32]);

        repository
            .save_session(SessionRecord::new(
                hash.clone(),
                UserId::new("user-1".to_string()),
                SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            ))
            .await
            .unwrap();

        repository
            .save_session(SessionRecord::new(
                hash.clone(),
                UserId::new("user-2".to_string()),
                SystemTime::UNIX_EPOCH + Duration::from_secs(20),
            ))
            .await
            .unwrap();

        let stored = repository.find_session(&hash).await.unwrap().unwrap();
        assert_eq!(stored.user_id().as_str(), "user-2");
        assert_eq!(
            stored.expires_at(),
            SystemTime::UNIX_EPOCH + Duration::from_secs(20)
        );
    }
}
