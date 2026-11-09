use anyhow::anyhow;
use argon2::{
    Argon2,
    password_hash::{
        PasswordHasher as _, PasswordVerifier, phc::PasswordHash as ParsedPasswordHash,
    },
};

use crate::auth::models::PasswordHash;
use crate::auth::ports::{PasswordHashError, PasswordHasher};

#[derive(Debug, Clone, Copy, Default)]
pub struct Argon2PasswordHasher;

impl Argon2PasswordHasher {
    pub fn new() -> Self {
        Self
    }
}

impl PasswordHasher for Argon2PasswordHasher {
    async fn hash_password(&self, password: &str) -> Result<PasswordHash, PasswordHashError> {
        let password = password.to_owned();

        let hash = tokio::task::spawn_blocking(move || {
            Argon2::default()
                .hash_password(password.as_bytes())
                .map(|hash| hash.to_string())
                .map_err(|error| anyhow!("{error}"))
        })
        .await
        .map_err(|error| PasswordHashError::Hashing(anyhow!("hash task failed: {error}")))?
        .map_err(PasswordHashError::Hashing)?;

        Ok(PasswordHash::new(hash))
    }

    async fn verify_password(
        &self,
        password: &str,
        hash: &PasswordHash,
    ) -> Result<bool, PasswordHashError> {
        let password = password.to_owned();
        let hash = hash.as_str().to_owned();

        tokio::task::spawn_blocking(move || {
            let parsed = ParsedPasswordHash::new(&hash).map_err(|error| anyhow!("{error}"))?;

            Ok(Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok())
        })
        .await
        .map_err(|error| PasswordHashError::Hashing(anyhow!("verify task failed: {error}")))?
        .map_err(PasswordHashError::Hashing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_and_verifies_a_password() {
        let hasher = Argon2PasswordHasher::new();

        let hash = hasher.hash_password("correct horse battery").await.unwrap();

        assert!(hash.as_str().starts_with("$argon2id$"));
        assert!(
            hasher
                .verify_password("correct horse battery", &hash)
                .await
                .unwrap()
        );
        assert!(!hasher.verify_password("wrong", &hash).await.unwrap());
    }
}
