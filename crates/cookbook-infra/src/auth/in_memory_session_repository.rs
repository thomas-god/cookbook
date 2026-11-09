use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::anyhow;

use crate::auth::models::{SessionRecord, SessionTokenHash};
use crate::auth::ports::{SessionRepository, SessionRepositoryError};

#[derive(Debug, Clone, Default)]
pub struct InMemorySessionRepository {
    sessions: Arc<RwLock<HashMap<SessionTokenHash, SessionRecord>>>,
}

impl InMemorySessionRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

fn storage_error(reason: &'static str) -> SessionRepositoryError {
    SessionRepositoryError::Storage(anyhow!(reason))
}

impl SessionRepository for InMemorySessionRepository {
    async fn find_session(
        &self,
        token_hash: &SessionTokenHash,
    ) -> Result<Option<SessionRecord>, SessionRepositoryError> {
        let sessions = self
            .sessions
            .read()
            .map_err(|_| storage_error("session store lock poisoned"))?;

        Ok(sessions.get(token_hash).cloned())
    }

    async fn save_session(&self, session: SessionRecord) -> Result<(), SessionRepositoryError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| storage_error("session store lock poisoned"))?;

        sessions.insert(session.token_hash().clone(), session);

        Ok(())
    }

    async fn delete_session(
        &self,
        token_hash: &SessionTokenHash,
    ) -> Result<(), SessionRepositoryError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| storage_error("session store lock poisoned"))?;

        sessions.remove(token_hash);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use cookbook_domain::user::models::UserId;

    use super::*;

    #[tokio::test]
    async fn saves_finds_and_deletes_sessions() {
        let repository = InMemorySessionRepository::new();
        let hash = SessionTokenHash::new([1; 32]);
        let record = SessionRecord::new(
            hash.clone(),
            UserId::new("user-1".to_string()),
            SystemTime::now() + Duration::from_hours(1),
        );

        repository.save_session(record).await.unwrap();

        let stored = repository.find_session(&hash).await.unwrap().unwrap();
        assert_eq!(stored.user_id().as_str(), "user-1");

        repository.delete_session(&hash).await.unwrap();

        assert!(repository.find_session(&hash).await.unwrap().is_none());
    }
}
