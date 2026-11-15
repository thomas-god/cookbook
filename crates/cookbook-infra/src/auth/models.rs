use std::fmt;
use std::time::{Duration, SystemTime};

use cookbook_domain::user::models::{UserId, UserRole, Username};

/// A stored, normalized, and valid email address.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserEmail(String);

impl UserEmail {
    pub fn try_new(value: impl Into<String>) -> Result<Self, InvalidUserEmail> {
        let value = value.into().trim().to_lowercase();
        let Some((local, domain)) = value.split_once('@') else {
            return Err(InvalidUserEmail);
        };

        if local.is_empty()
            || domain.is_empty()
            || domain.contains('@')
            || domain.starts_with('.')
            || domain.ends_with('.')
        {
            return Err(InvalidUserEmail);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidUserEmail;

impl fmt::Display for InvalidUserEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid email address")
    }
}

impl std::error::Error for InvalidUserEmail {}

/// A password hash, normally a PHC string produced by Argon2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The hash of a session token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionTokenHash([u8; 32]);

impl SessionTokenHash {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// A stored user, including the credentials used to authenticate them.
#[derive(Debug, Clone)]
pub struct AuthUser {
    id: UserId,
    email: UserEmail,
    username: Option<Username>,
    password_hash: PasswordHash,
    role: UserRole,
}

impl AuthUser {
    pub fn new(
        id: UserId,
        email: UserEmail,
        username: Option<Username>,
        password_hash: PasswordHash,
        role: UserRole,
    ) -> Self {
        Self {
            id,
            email,
            username,
            password_hash,
            role,
        }
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn email(&self) -> &UserEmail {
        &self.email
    }

    pub fn username(&self) -> Option<&Username> {
        self.username.as_ref()
    }

    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
    }

    pub fn role(&self) -> UserRole {
        self.role
    }

    pub fn display_name(&self) -> &str {
        self.username
            .as_ref()
            .map_or(self.email.as_str(), Username::as_str)
    }

    pub fn with_email(self, email: UserEmail) -> Self {
        Self::new(self.id, email, self.username, self.password_hash, self.role)
    }

    pub fn with_username(self, username: Option<Username>) -> Self {
        Self::new(self.id, self.email, username, self.password_hash, self.role)
    }

    pub fn with_password_hash(self, password_hash: PasswordHash) -> Self {
        Self::new(self.id, self.email, self.username, password_hash, self.role)
    }

    pub fn with_role(self, role: UserRole) -> Self {
        Self::new(self.id, self.email, self.username, self.password_hash, role)
    }
}

/// An administrative view of a user, safe to expose to the application layer.
#[derive(Debug, Clone)]
pub struct UserSummary {
    id: UserId,
    email: UserEmail,
    username: Option<Username>,
    role: UserRole,
}

impl UserSummary {
    pub fn new(id: UserId, email: UserEmail, username: Option<Username>, role: UserRole) -> Self {
        Self {
            id,
            email,
            username,
            role,
        }
    }

    pub fn from_user(user: &AuthUser) -> Self {
        Self::new(
            user.id().clone(),
            user.email().clone(),
            user.username().cloned(),
            user.role(),
        )
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn email(&self) -> &UserEmail {
        &self.email
    }

    pub fn username(&self) -> Option<&Username> {
        self.username.as_ref()
    }

    pub fn role(&self) -> UserRole {
        self.role
    }

    pub fn display_name(&self) -> &str {
        self.username
            .as_ref()
            .map_or(self.email.as_str(), Username::as_str)
    }
}

/// An authenticated user, safe to expose to the application layer.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    id: UserId,
    email: UserEmail,
    username: Option<Username>,
    role: UserRole,
}

impl AuthenticatedUser {
    pub fn new(id: UserId, email: UserEmail, username: Option<Username>, role: UserRole) -> Self {
        Self {
            id,
            email,
            username,
            role,
        }
    }

    pub fn from_user(user: &AuthUser) -> Self {
        Self::new(
            user.id().clone(),
            user.email().clone(),
            user.username().cloned(),
            user.role(),
        )
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn email(&self) -> &UserEmail {
        &self.email
    }

    pub fn username(&self) -> Option<&Username> {
        self.username.as_ref()
    }

    pub fn role(&self) -> UserRole {
        self.role
    }

    pub fn display_name(&self) -> &str {
        self.username
            .as_ref()
            .map_or(self.email.as_str(), Username::as_str)
    }
}

/// The result of a successful login.
#[derive(Debug, Clone)]
pub struct LoginResult {
    user: AuthenticatedUser,
    session: IssuedSession,
}

impl LoginResult {
    pub fn new(user: AuthenticatedUser, session: IssuedSession) -> Self {
        Self { user, session }
    }

    pub fn user(&self) -> &AuthenticatedUser {
        &self.user
    }

    pub fn session(&self) -> &IssuedSession {
        &self.session
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedSessionToken {
    token: String,
    token_hash: SessionTokenHash,
}

impl GeneratedSessionToken {
    pub fn new(token: impl Into<String>, token_hash: SessionTokenHash) -> Self {
        Self {
            token: token.into(),
            token_hash,
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn token_hash(&self) -> &SessionTokenHash {
        &self.token_hash
    }
}

/// A session issued by the application service.
#[derive(Debug, Clone)]
pub struct IssuedSession {
    token: String,
    token_hash: SessionTokenHash,
    expires_at: SystemTime,
}

impl IssuedSession {
    pub fn new(
        token: impl Into<String>,
        token_hash: SessionTokenHash,
        expires_at: SystemTime,
    ) -> Self {
        Self {
            token: token.into(),
            token_hash,
            expires_at,
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn token_hash(&self) -> &SessionTokenHash {
        &self.token_hash
    }

    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }
}

/// The application's session lifetime policy.
#[derive(Debug, Clone, Copy)]
pub struct SessionPolicy {
    lifetime: Duration,
    renewal_threshold: Duration,
}

impl SessionPolicy {
    pub const fn new(lifetime: Duration, renewal_threshold: Duration) -> Self {
        Self {
            lifetime,
            renewal_threshold,
        }
    }

    pub const fn lifetime(self) -> Duration {
        self.lifetime
    }

    pub const fn renewal_threshold(self) -> Duration {
        self.renewal_threshold
    }
}

impl Default for SessionPolicy {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(60 * 60 * 24 * 30),
            Duration::from_secs(60 * 60 * 24 * 7),
        )
    }
}

/// A persisted session, keyed by the hash of its token.
#[derive(Debug, Clone)]
pub struct SessionRecord {
    token_hash: SessionTokenHash,
    user_id: UserId,
    expires_at: SystemTime,
}

impl SessionRecord {
    pub fn new(token_hash: SessionTokenHash, user_id: UserId, expires_at: SystemTime) -> Self {
        Self {
            token_hash,
            user_id,
            expires_at,
        }
    }

    pub fn token_hash(&self) -> &SessionTokenHash {
        &self.token_hash
    }

    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_valid_emails() {
        let email = UserEmail::try_new(" Alice@Example.com ").unwrap();

        assert_eq!(email.as_str(), "alice@example.com");
    }

    #[test]
    fn rejects_invalid_emails() {
        for value in [
            "",
            "alice",
            "@example.com",
            "alice@",
            "alice@example.com@example.com",
            "alice@.example.com",
            "alice@example.com.",
        ] {
            assert!(
                UserEmail::try_new(value).is_err(),
                "{value} should be invalid"
            );
        }
    }
}
