use std::future::Future;
use std::{fmt, time::SystemTime};

use cookbook_domain::user::models::{UserId, UserRole, Username};

use crate::auth::models::{
    AuthUser, AuthenticatedUser, GeneratedSessionToken, LoginResult, PasswordHash, SessionRecord,
    SessionTokenHash, UserEmail,
};

pub trait Clock: Clone + Send + Sync + 'static {
    fn now(&self) -> SystemTime;
}

pub trait SessionTokenGenerator: Clone + Send + Sync + 'static {
    fn generate(&self) -> GeneratedSessionToken;
}

/// An inbound (driving) port for authentication.
pub trait AuthService: Clone + Send + Sync + 'static {
    fn register(
        &self,
        request: RegisterRequest,
    ) -> impl Future<Output = Result<LoginResult, RegisterError>> + Send;

    /// Creates the first user as an admin, unless a user already exists.
    fn register_initial_admin(
        &self,
        request: RegisterRequest,
    ) -> impl Future<Output = Result<Option<LoginResult>, RegisterError>> + Send;

    fn login(
        &self,
        email: UserEmail,
        password: String,
    ) -> impl Future<Output = Result<LoginResult, LoginError>> + Send;

    fn authenticate_session(
        &self,
        token_hash: SessionTokenHash,
    ) -> impl Future<
        Output = Result<Option<(AuthenticatedUser, SystemTime)>, AuthenticateSessionError>,
    > + Send;

    fn renew_session(
        &self,
        token_hash: SessionTokenHash,
    ) -> impl Future<Output = Result<Option<SystemTime>, RenewSessionError>> + Send;

    fn session_needs_renewal(&self, expires_at: SystemTime) -> bool;

    fn delete_session(
        &self,
        token_hash: SessionTokenHash,
    ) -> impl Future<Output = Result<(), DeleteSessionError>> + Send;
}

/// An outbound (driven) port for user persistence.
pub trait UserRepository: Clone + Send + Sync + 'static {
    fn find_by_id(
        &self,
        id: &UserId,
    ) -> impl Future<Output = Result<Option<AuthUser>, UserRepositoryError>> + Send;

    fn find_by_email(
        &self,
        email: &UserEmail,
    ) -> impl Future<Output = Result<Option<AuthUser>, UserRepositoryError>> + Send;

    fn save_user(
        &self,
        user: AuthUser,
    ) -> impl Future<Output = Result<(), UserRepositoryError>> + Send;

    fn count_users(&self) -> impl Future<Output = Result<usize, UserRepositoryError>> + Send;
}

/// An outbound (driven) port for session persistence.
pub trait SessionRepository: Clone + Send + Sync + 'static {
    fn find_session(
        &self,
        token_hash: &SessionTokenHash,
    ) -> impl Future<Output = Result<Option<SessionRecord>, SessionRepositoryError>> + Send;

    fn save_session(
        &self,
        session: SessionRecord,
    ) -> impl Future<Output = Result<(), SessionRepositoryError>> + Send;

    fn delete_session(
        &self,
        token_hash: &SessionTokenHash,
    ) -> impl Future<Output = Result<(), SessionRepositoryError>> + Send;
}

/// An outbound (driven) port for password hashing and verification.
pub trait PasswordHasher: Clone + Send + Sync + 'static {
    fn hash_password(
        &self,
        password: &str,
    ) -> impl Future<Output = Result<PasswordHash, PasswordHashError>> + Send;

    fn verify_password(
        &self,
        password: &str,
        hash: &PasswordHash,
    ) -> impl Future<Output = Result<bool, PasswordHashError>> + Send;
}

#[derive(Debug, Clone)]
pub struct RegisterRequest {
    pub email: UserEmail,
    pub username: Option<Username>,
    pub password: String,
    pub role: UserRole,
}

#[derive(Debug)]
pub enum RegisterError {
    InvalidEmail,
    PasswordTooShort { minimum: usize },
    EmailTaken,
    SessionError(StartSessionError),
    Storage(anyhow::Error),
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEmail => f.write_str("invalid email address"),
            Self::PasswordTooShort { minimum } => {
                write!(f, "password must be at least {minimum} characters")
            }
            Self::EmailTaken => f.write_str("an account with this email already exists"),
            Self::SessionError(err) => write!(f, "failed to start a session: {err}"),
            Self::Storage(reason) => write!(f, "failed to register user: {reason}"),
        }
    }
}

impl std::error::Error for RegisterError {}

#[derive(Debug)]
pub enum LoginError {
    InvalidCredentials,
    Storage(anyhow::Error),
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCredentials => f.write_str("invalid email or password"),
            Self::Storage(reason) => write!(f, "failed to log in: {reason}"),
        }
    }
}

impl std::error::Error for LoginError {}

#[derive(Debug)]
pub enum StartSessionError {
    Storage(anyhow::Error),
}

impl fmt::Display for StartSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to start session: {reason}"),
        }
    }
}

impl std::error::Error for StartSessionError {}

#[derive(Debug)]
pub enum AuthenticateSessionError {
    Storage(anyhow::Error),
}

impl fmt::Display for AuthenticateSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to authenticate session: {reason}"),
        }
    }
}

impl std::error::Error for AuthenticateSessionError {}

#[derive(Debug)]
pub enum RenewSessionError {
    Storage(anyhow::Error),
}

impl fmt::Display for RenewSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to renew session: {reason}"),
        }
    }
}

impl std::error::Error for RenewSessionError {}

#[derive(Debug)]
pub enum DeleteSessionError {
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to delete session: {reason}"),
        }
    }
}

impl std::error::Error for DeleteSessionError {}

#[derive(Debug)]
pub enum UserRepositoryError {
    Storage(anyhow::Error),
}

impl fmt::Display for UserRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to access users: {reason}"),
        }
    }
}

impl std::error::Error for UserRepositoryError {}

#[derive(Debug)]
pub enum SessionRepositoryError {
    Storage(anyhow::Error),
}

impl fmt::Display for SessionRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to access sessions: {reason}"),
        }
    }
}

impl std::error::Error for SessionRepositoryError {}

#[derive(Debug)]
pub enum PasswordHashError {
    Hashing(anyhow::Error),
}

impl fmt::Display for PasswordHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hashing(reason) => write!(f, "failed to hash password: {reason}"),
        }
    }
}

impl std::error::Error for PasswordHashError {}

#[cfg(test)]
mockall::mock! {
    pub AuthService {}

    impl Clone for AuthService {
        fn clone(&self) -> Self;
    }

    impl AuthService for AuthService {
        async fn register(&self, request: RegisterRequest) -> Result<LoginResult, RegisterError>;

        async fn register_initial_admin(
            &self,
            request: RegisterRequest,
        ) -> Result<Option<LoginResult>, RegisterError>;

        async fn login(
            &self,
            email: UserEmail,
            password: String,
        ) -> Result<LoginResult, LoginError>;

        async fn authenticate_session(
            &self,
            token_hash: SessionTokenHash,
        ) -> Result<Option<(AuthenticatedUser, SystemTime)>, AuthenticateSessionError>;

        async fn renew_session(
            &self,
            token_hash: SessionTokenHash,
        ) -> Result<Option<SystemTime>, RenewSessionError>;

        fn session_needs_renewal(&self, expires_at: SystemTime) -> bool;

        async fn delete_session(
            &self,
            token_hash: SessionTokenHash,
        ) -> Result<(), DeleteSessionError>;
    }
}

#[cfg(test)]
mockall::mock! {
    pub UserRepository {}

    impl Clone for UserRepository {
        fn clone(&self) -> Self;
    }

    impl UserRepository for UserRepository {
        async fn find_by_id(&self, id: &UserId) -> Result<Option<AuthUser>, UserRepositoryError>;

        async fn find_by_email(
            &self,
            email: &UserEmail,
        ) -> Result<Option<AuthUser>, UserRepositoryError>;

        async fn save_user(&self, user: AuthUser) -> Result<(), UserRepositoryError>;

        async fn count_users(&self) -> Result<usize, UserRepositoryError>;
    }
}

#[cfg(test)]
mockall::mock! {
    pub SessionRepository {}

    impl Clone for SessionRepository {
        fn clone(&self) -> Self;
    }

    impl SessionRepository for SessionRepository {
        async fn find_session(
            &self,
            token_hash: &SessionTokenHash,
        ) -> Result<Option<SessionRecord>, SessionRepositoryError>;

        async fn save_session(&self, session: SessionRecord) -> Result<(), SessionRepositoryError>;

        async fn delete_session(
            &self,
            token_hash: &SessionTokenHash,
        ) -> Result<(), SessionRepositoryError>;
    }
}

#[cfg(test)]
mockall::mock! {
    pub PasswordHasher {}

    impl Clone for PasswordHasher {
        fn clone(&self) -> Self;
    }

    impl PasswordHasher for PasswordHasher {
        async fn hash_password(&self, password: &str) -> Result<PasswordHash, PasswordHashError>;

        async fn verify_password(
            &self,
            password: &str,
            hash: &PasswordHash,
        ) -> Result<bool, PasswordHashError>;
    }
}
