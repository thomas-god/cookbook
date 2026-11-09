use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use anyhow::anyhow;

use cookbook_domain::user::models::UserId;

use crate::auth::models::{AuthUser, UserEmail};
use crate::auth::ports::{UserRepository, UserRepositoryError};

#[derive(Debug, Clone, Default)]
pub struct InMemoryUserRepository {
    users: Arc<RwLock<HashMap<UserId, AuthUser>>>,
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

fn storage_error(reason: &'static str) -> UserRepositoryError {
    UserRepositoryError::Storage(anyhow!(reason))
}

impl UserRepository for InMemoryUserRepository {
    async fn find_by_id(&self, id: &UserId) -> Result<Option<AuthUser>, UserRepositoryError> {
        let users = self
            .users
            .read()
            .map_err(|_| storage_error("user store lock poisoned"))?;

        Ok(users.get(id).cloned())
    }

    async fn find_by_email(
        &self,
        email: &UserEmail,
    ) -> Result<Option<AuthUser>, UserRepositoryError> {
        let users = self
            .users
            .read()
            .map_err(|_| storage_error("user store lock poisoned"))?;

        Ok(users.values().find(|user| user.email() == email).cloned())
    }

    async fn save_user(&self, user: AuthUser) -> Result<(), UserRepositoryError> {
        let mut users = self
            .users
            .write()
            .map_err(|_| storage_error("user store lock poisoned"))?;

        users.insert(user.id().clone(), user);

        Ok(())
    }

    async fn count_users(&self) -> Result<usize, UserRepositoryError> {
        let users = self
            .users
            .read()
            .map_err(|_| storage_error("user store lock poisoned"))?;

        Ok(users.len())
    }
}

#[cfg(test)]
mod tests {
    use cookbook_domain::user::models::{UserId, UserRole};

    use super::*;
    use crate::auth::models::PasswordHash;

    fn user() -> AuthUser {
        AuthUser::new(
            UserId::new("user-1".to_string()),
            UserEmail::new("Alice@Example.com"),
            None,
            PasswordHash::new("hash"),
            UserRole::User,
        )
    }

    #[tokio::test]
    async fn saves_and_finds_users_by_id_and_email() {
        let repository = InMemoryUserRepository::new();
        repository.save_user(user()).await.unwrap();

        let by_id = repository
            .find_by_id(&UserId::new("user-1".to_string()))
            .await
            .unwrap();
        let by_email = repository
            .find_by_email(&UserEmail::new("alice@example.com"))
            .await
            .unwrap();

        assert_eq!(by_id.unwrap().id().as_str(), "user-1");
        assert_eq!(by_email.unwrap().email().as_str(), "alice@example.com");
        assert_eq!(repository.count_users().await.unwrap(), 1);
    }
}
