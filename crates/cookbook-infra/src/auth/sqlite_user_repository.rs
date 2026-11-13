use anyhow::{Context, anyhow};
use cookbook_domain::user::models::{UserId, UserRole, Username};
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqliteRow};

use crate::auth::models::{AuthUser, PasswordHash, UserEmail};
use crate::auth::ports::{UserRepository, UserRepositoryError};

#[derive(Debug, Clone)]
pub struct SqliteUserRepository {
    pool: SqlitePool,
}

impl SqliteUserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    async fn find_user(
        &self,
        query: &'static str,
        value: &str,
    ) -> Result<Option<AuthUser>, UserRepositoryError> {
        let row = sqlx::query(query)
            .bind(value)
            .fetch_optional(&self.pool)
            .await
            .map_err(storage_error)?;

        row.map(|row| decode_user(&row))
            .transpose()
            .map_err(UserRepositoryError::Storage)
    }
}

impl UserRepository for SqliteUserRepository {
    async fn find_by_id(&self, id: &UserId) -> Result<Option<AuthUser>, UserRepositoryError> {
        self.find_user(
            concat!(
                "SELECT id, email, username, password_hash, role ",
                "FROM users WHERE id = ?"
            ),
            id.as_str(),
        )
        .await
    }

    async fn find_by_email(
        &self,
        email: &UserEmail,
    ) -> Result<Option<AuthUser>, UserRepositoryError> {
        self.find_user(
            concat!(
                "SELECT id, email, username, password_hash, role ",
                "FROM users WHERE email = ? LIMIT 1"
            ),
            email.as_str(),
        )
        .await
    }

    async fn save_user(&self, user: AuthUser) -> Result<(), UserRepositoryError> {
        sqlx::query(
            "INSERT INTO users (id, email, username, password_hash, role)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                 email = excluded.email,
                 username = excluded.username,
                 password_hash = excluded.password_hash,
                 role = excluded.role",
        )
        .bind(user.id().as_str())
        .bind(user.email().as_str())
        .bind(user.username().map(Username::as_str))
        .bind(user.password_hash().as_str())
        .bind(user.role().as_str())
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;

        Ok(())
    }

    async fn count_users(&self) -> Result<usize, UserRepositoryError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(storage_error)?;

        usize::try_from(count)
            .context("user count does not fit in usize")
            .map_err(UserRepositoryError::Storage)
    }
}

fn decode_user(row: &SqliteRow) -> anyhow::Result<AuthUser> {
    let id: String = row.try_get("id")?;
    let email: String = row.try_get("email")?;
    let username: Option<String> = row.try_get("username")?;
    let password_hash: String = row.try_get("password_hash")?;
    let role: String = row.try_get("role")?;
    let role = UserRole::parse(&role).ok_or_else(|| anyhow!("unknown user role `{role}`"))?;

    Ok(AuthUser::new(
        UserId::new(id),
        UserEmail::new(email),
        username.map(Username::new),
        PasswordHash::new(password_hash),
        role,
    ))
}

fn storage_error(error: sqlx::Error) -> UserRepositoryError {
    UserRepositoryError::Storage(error.into())
}

#[cfg(test)]
mod tests {
    use cookbook_domain::user::models::{UserId, UserRole, Username};

    use super::*;
    use crate::storage::SqliteDatabase;

    async fn repository() -> SqliteUserRepository {
        let database = SqliteDatabase::open_in_memory().await.unwrap();

        SqliteUserRepository::new(database.pool().clone())
    }

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
        let repository = repository().await;
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

    #[tokio::test]
    async fn returns_none_for_an_unknown_user_and_email() {
        let repository = repository().await;

        assert!(
            repository
                .find_by_id(&UserId::new("missing".to_string()))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            repository
                .find_by_email(&UserEmail::new("missing@example.com"))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn saving_an_existing_id_updates_the_user() {
        let repository = repository().await;
        repository.save_user(user()).await.unwrap();

        repository
            .save_user(AuthUser::new(
                UserId::new("user-1".to_string()),
                UserEmail::new("new@example.com"),
                Some(Username::new("New Name".to_string())),
                PasswordHash::new("new-hash"),
                UserRole::Admin,
            ))
            .await
            .unwrap();

        let stored = repository
            .find_by_id(&UserId::new("user-1".to_string()))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(stored.email().as_str(), "new@example.com");
        assert_eq!(stored.username().map(Username::as_str), Some("New Name"));
        assert_eq!(stored.password_hash().as_str(), "new-hash");
        assert_eq!(stored.role(), UserRole::Admin);
        assert_eq!(repository.count_users().await.unwrap(), 1);
    }
}
