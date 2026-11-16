use std::future::Future;
use std::{fmt, time::SystemTime};

use cookbook_domain::user::models::{UserId, UserRole, Username};

use crate::auth::models::{
    AuthUser, AuthenticatedUser, GeneratedSessionToken, LoginResult, PasswordHash, SessionRecord,
    SessionTokenHash, UserEmail, UserSummary,
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

    fn change_email(
        &self,
        user_id: &UserId,
        current_password: String,
        new_email: UserEmail,
    ) -> impl Future<Output = Result<AuthenticatedUser, ChangeEmailError>> + Send;

    fn change_username(
        &self,
        user_id: &UserId,
        username: Option<Username>,
    ) -> impl Future<Output = Result<AuthenticatedUser, ChangeUsernameError>> + Send;

    fn change_password(
        &self,
        user_id: &UserId,
        current_password: String,
        new_password: String,
    ) -> impl Future<Output = Result<(), ChangePasswordError>> + Send;

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

    fn delete_account(
        &self,
        user: &UserId,
    ) -> impl Future<Output = Result<(), DeleteUserError>> + Send;
}

/// An inbound (driving) port for administrative user management.
pub trait UserAdminService: Clone + Send + Sync + 'static {
    fn list_users(&self) -> impl Future<Output = Result<Vec<UserSummary>, ListUsersError>> + Send;

    fn get_user(
        &self,
        id: &UserId,
    ) -> impl Future<Output = Result<Option<UserSummary>, GetUserError>> + Send;

    fn create_user(
        &self,
        request: CreateUserRequest,
    ) -> impl Future<Output = Result<UserSummary, CreateUserError>> + Send;

    fn change_user_email(
        &self,
        id: &UserId,
        email: UserEmail,
    ) -> impl Future<Output = Result<UserSummary, ChangeUserEmailError>> + Send;

    fn change_user_username(
        &self,
        id: &UserId,
        username: Option<Username>,
    ) -> impl Future<Output = Result<UserSummary, ChangeUserUsernameError>> + Send;

    fn reset_user_password(
        &self,
        id: &UserId,
        password: String,
    ) -> impl Future<Output = Result<(), ResetUserPasswordError>> + Send;

    fn set_user_role(
        &self,
        actor: &UserId,
        target: &UserId,
        role: UserRole,
    ) -> impl Future<Output = Result<UserSummary, SetUserRoleError>> + Send;

    fn delete_user(
        &self,
        actor: &UserId,
        target: &UserId,
    ) -> impl Future<Output = Result<(), DeleteUserError>> + Send;
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

    fn delete_user(
        &self,
        user: &UserId,
    ) -> impl Future<Output = Result<(), UserRepositoryError>> + Send;

    fn count_users(&self) -> impl Future<Output = Result<usize, UserRepositoryError>> + Send;

    fn list_users(&self)
    -> impl Future<Output = Result<Vec<AuthUser>, UserRepositoryError>> + Send;

    fn count_admins(&self) -> impl Future<Output = Result<usize, UserRepositoryError>> + Send;
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

#[derive(Debug, Clone)]
pub struct CreateUserRequest {
    pub email: UserEmail,
    pub username: Option<Username>,
    pub password: String,
    pub role: UserRole,
}

#[derive(Debug)]
pub enum RegisterError {
    EmailTaken,
    SessionError(StartSessionError),
    Storage(anyhow::Error),
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
pub enum ChangeEmailError {
    InvalidPassword,
    EmailTaken,
    Storage(anyhow::Error),
}

impl fmt::Display for ChangeEmailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPassword => f.write_str("current password is incorrect"),
            Self::EmailTaken => f.write_str("an account with this email already exists"),
            Self::Storage(reason) => write!(f, "failed to change email: {reason}"),
        }
    }
}

impl std::error::Error for ChangeEmailError {}

#[derive(Debug)]
pub enum ChangeUsernameError {
    Storage(anyhow::Error),
}

impl fmt::Display for ChangeUsernameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to change username: {reason}"),
        }
    }
}

impl std::error::Error for ChangeUsernameError {}

#[derive(Debug)]
pub enum ChangePasswordError {
    InvalidPassword,
    Storage(anyhow::Error),
}

impl fmt::Display for ChangePasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPassword => f.write_str("current password is incorrect"),
            Self::Storage(reason) => write!(f, "failed to change password: {reason}"),
        }
    }
}

impl std::error::Error for ChangePasswordError {}

#[derive(Debug)]
pub enum ListUsersError {
    Storage(anyhow::Error),
}

impl fmt::Display for ListUsersError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to list users: {reason}"),
        }
    }
}

impl std::error::Error for ListUsersError {}

#[derive(Debug)]
pub enum GetUserError {
    Storage(anyhow::Error),
}

impl fmt::Display for GetUserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to get user: {reason}"),
        }
    }
}

impl std::error::Error for GetUserError {}

#[derive(Debug)]
pub enum CreateUserError {
    EmailTaken,
    Storage(anyhow::Error),
}

impl fmt::Display for CreateUserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmailTaken => f.write_str("an account with this email already exists"),
            Self::Storage(reason) => write!(f, "failed to create user: {reason}"),
        }
    }
}

impl std::error::Error for CreateUserError {}

#[derive(Debug)]
pub enum ChangeUserEmailError {
    NotFound,
    EmailTaken,
    Storage(anyhow::Error),
}

impl fmt::Display for ChangeUserEmailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("user not found"),
            Self::EmailTaken => f.write_str("an account with this email already exists"),
            Self::Storage(reason) => write!(f, "failed to change user email: {reason}"),
        }
    }
}

impl std::error::Error for ChangeUserEmailError {}

#[derive(Debug)]
pub enum ChangeUserUsernameError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for ChangeUserUsernameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("user not found"),
            Self::Storage(reason) => write!(f, "failed to change user username: {reason}"),
        }
    }
}

impl std::error::Error for ChangeUserUsernameError {}

#[derive(Debug)]
pub enum ResetUserPasswordError {
    NotFound,
    Storage(anyhow::Error),
}

impl fmt::Display for ResetUserPasswordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("user not found"),
            Self::Storage(reason) => write!(f, "failed to reset user password: {reason}"),
        }
    }
}

impl std::error::Error for ResetUserPasswordError {}

#[derive(Debug)]
pub enum DeleteUserError {
    Storage(anyhow::Error),
}

impl fmt::Display for DeleteUserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(reason) => write!(f, "failed to delete user: {reason}"),
        }
    }
}

impl std::error::Error for DeleteUserError {}

#[derive(Debug)]
pub enum SetUserRoleError {
    NotFound,
    LastAdmin,
    Storage(anyhow::Error),
}

impl fmt::Display for SetUserRoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("user not found"),
            Self::LastAdmin => f.write_str("cannot remove the last administrator"),
            Self::Storage(reason) => write!(f, "failed to change user role: {reason}"),
        }
    }
}

impl std::error::Error for SetUserRoleError {}

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

        async fn change_email(
            &self,
            user_id: &UserId,
            current_password: String,
            new_email: UserEmail,
        ) -> Result<AuthenticatedUser, ChangeEmailError>;

        async fn change_username(
            &self,
            user_id: &UserId,
            username: Option<Username>,
        ) -> Result<AuthenticatedUser, ChangeUsernameError>;

        async fn change_password(
            &self,
            user_id: &UserId,
            current_password: String,
            new_password: String,
        ) -> Result<(), ChangePasswordError>;

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

        async fn delete_account(
            &self,
            user: &UserId,
        ) -> Result<(), DeleteUserError>;
    }
}

#[cfg(test)]
mockall::mock! {
    pub UserAdminService {}

    impl Clone for UserAdminService {
        fn clone(&self) -> Self;
    }

    impl UserAdminService for UserAdminService {
        async fn list_users(&self) -> Result<Vec<UserSummary>, ListUsersError>;

        async fn get_user(
            &self,
            id: &UserId,
        ) -> Result<Option<UserSummary>, GetUserError>;

        async fn create_user(
            &self,
            request: CreateUserRequest,
        ) -> Result<UserSummary, CreateUserError>;

        async fn change_user_email(
            &self,
            id: &UserId,
            email: UserEmail,
        ) -> Result<UserSummary, ChangeUserEmailError>;

        async fn change_user_username(
            &self,
            id: &UserId,
            username: Option<Username>,
        ) -> Result<UserSummary, ChangeUserUsernameError>;

        async fn reset_user_password(
            &self,
            id: &UserId,
            password: String,
        ) -> Result<(), ResetUserPasswordError>;

        async fn set_user_role(
            &self,
            actor: &UserId,
            target: &UserId,
            role: UserRole,
        ) -> Result<UserSummary, SetUserRoleError>;

        async fn delete_user(
            &self,
            actor: &UserId,
            target: &UserId,
        ) -> Result<(), DeleteUserError>;
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

        async fn delete_user(&self, user: &UserId) -> Result<(), UserRepositoryError>;

        async fn count_users(&self) -> Result<usize, UserRepositoryError>;

        async fn list_users(&self) -> Result<Vec<AuthUser>, UserRepositoryError>;

        async fn count_admins(&self) -> Result<usize, UserRepositoryError>;
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
