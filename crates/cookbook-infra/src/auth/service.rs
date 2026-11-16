use std::time::SystemTime;

use cookbook_domain::user::models::{UserId, UserRole, Username};
use uuid::Uuid;

use crate::auth::models::{
    AdminActor, AuthUser, AuthenticatedUser, IssuedSession, LoginResult, SessionPolicy,
    SessionRecord, SessionTokenHash, UserEmail, UserSummary,
};
use crate::auth::ports::{
    AuthService, AuthenticateSessionError, ChangeEmailError, ChangePasswordError,
    ChangeUserEmailError, ChangeUserUsernameError, ChangeUsernameError, Clock, CreateUserError,
    CreateUserRequest, DeleteSessionError, DeleteUserError, GetUserError, ListUsersError,
    LoginError, PasswordHasher, RegisterError, RegisterRequest, RenewSessionError,
    ResetUserPasswordError, SessionRepository, SessionTokenGenerator, SetUserRoleError,
    StartSessionError, UserAdminService, UserRepository,
};

#[derive(Debug, Clone)]
pub struct AuthServiceImpl<U, P, S, C, G> {
    users: U,
    passwords: P,
    sessions: S,
    clock: C,
    tokens: G,
    policy: SessionPolicy,
}

enum CreateAccountFailure {
    EmailTaken,
    Storage(anyhow::Error),
}

enum EmailChangeFailure {
    EmailTaken,
    Storage(anyhow::Error),
}

impl<U, P, S, C, G> AuthServiceImpl<U, P, S, C, G> {
    pub fn new(
        users: U,
        passwords: P,
        sessions: S,
        clock: C,
        tokens: G,
        policy: SessionPolicy,
    ) -> Self {
        Self {
            users,
            passwords,
            sessions,
            clock,
            tokens,
            policy,
        }
    }
}

impl<U, P, S, C, G> AuthService for AuthServiceImpl<U, P, S, C, G>
where
    U: UserRepository,
    P: PasswordHasher,
    S: SessionRepository,
    C: Clock,
    G: SessionTokenGenerator,
{
    async fn register(&self, request: RegisterRequest) -> Result<LoginResult, RegisterError> {
        self.register_user(request).await
    }

    async fn register_initial_admin(
        &self,
        request: RegisterRequest,
    ) -> Result<Option<LoginResult>, RegisterError> {
        let count = self
            .users
            .count_users()
            .await
            .map_err(|error| RegisterError::Storage(error.into()))?;

        if count > 0 {
            return Ok(None);
        }

        self.register_user(request).await.map(Some)
    }

    async fn login(&self, email: UserEmail, password: String) -> Result<LoginResult, LoginError> {
        let user = self.verify_credentials(email, password).await?;
        let session = self
            .start_session(user.id().clone())
            .await
            .map_err(|error| LoginError::Storage(error.into()))?;

        Ok(LoginResult::new(
            AuthenticatedUser::from_user(&user),
            session,
        ))
    }

    async fn change_email(
        &self,
        actor: &AuthenticatedUser,
        current_password: String,
        new_email: UserEmail,
    ) -> Result<AuthenticatedUser, ChangeEmailError> {
        let user = self
            .load_user(actor.id())
            .await
            .map_err(ChangeEmailError::Storage)?;

        let matches = self
            .passwords
            .verify_password(&current_password, user.password_hash())
            .await
            .map_err(|error| ChangeEmailError::Storage(error.into()))?;

        if !matches {
            return Err(ChangeEmailError::InvalidPassword);
        }

        let user = self
            .apply_email(user, new_email)
            .await
            .map_err(|error| match error {
                EmailChangeFailure::EmailTaken => ChangeEmailError::EmailTaken,
                EmailChangeFailure::Storage(error) => ChangeEmailError::Storage(error),
            })?;

        Ok(AuthenticatedUser::from_user(&user))
    }

    async fn change_username(
        &self,
        actor: &AuthenticatedUser,
        username: Option<Username>,
    ) -> Result<AuthenticatedUser, ChangeUsernameError> {
        let user = self
            .load_user(actor.id())
            .await
            .map_err(ChangeUsernameError::Storage)?;

        let user = self
            .apply_username(user, username)
            .await
            .map_err(ChangeUsernameError::Storage)?;

        Ok(AuthenticatedUser::from_user(&user))
    }

    async fn change_password(
        &self,
        actor: &AuthenticatedUser,
        current_password: String,
        new_password: String,
    ) -> Result<(), ChangePasswordError> {
        let user = self
            .load_user(actor.id())
            .await
            .map_err(ChangePasswordError::Storage)?;

        let matches = self
            .passwords
            .verify_password(&current_password, user.password_hash())
            .await
            .map_err(|error| ChangePasswordError::Storage(error.into()))?;

        if !matches {
            return Err(ChangePasswordError::InvalidPassword);
        }

        self.apply_password(user, new_password)
            .await
            .map_err(ChangePasswordError::Storage)
    }

    async fn authenticate_session(
        &self,
        token_hash: SessionTokenHash,
    ) -> Result<Option<(AuthenticatedUser, SystemTime)>, AuthenticateSessionError> {
        let now = self.clock.now();
        let Some(session) = self
            .sessions
            .find_session(&token_hash)
            .await
            .map_err(|error| AuthenticateSessionError::Storage(error.into()))?
        else {
            return Ok(None);
        };

        if session.expires_at() <= now {
            self.sessions
                .delete_session(&token_hash)
                .await
                .map_err(|error| AuthenticateSessionError::Storage(error.into()))?;
            return Ok(None);
        }

        let Some(user) = self
            .users
            .find_by_id(session.user_id())
            .await
            .map_err(|error| AuthenticateSessionError::Storage(error.into()))?
        else {
            self.sessions
                .delete_session(&token_hash)
                .await
                .map_err(|error| AuthenticateSessionError::Storage(error.into()))?;
            return Ok(None);
        };

        Ok(Some((
            AuthenticatedUser::from_user(&user),
            session.expires_at(),
        )))
    }

    async fn renew_session(
        &self,
        token_hash: SessionTokenHash,
    ) -> Result<Option<SystemTime>, RenewSessionError> {
        let Some(session) = self
            .sessions
            .find_session(&token_hash)
            .await
            .map_err(|error| RenewSessionError::Storage(error.into()))?
        else {
            return Ok(None);
        };

        let expires_at = self.clock.now() + self.policy.lifetime();

        self.sessions
            .save_session(SessionRecord::new(
                token_hash,
                session.user_id().clone(),
                expires_at,
            ))
            .await
            .map_err(|error| RenewSessionError::Storage(error.into()))?;

        Ok(Some(expires_at))
    }

    fn session_needs_renewal(&self, expires_at: SystemTime) -> bool {
        expires_at
            .duration_since(self.clock.now())
            .unwrap_or_default()
            < self.policy.renewal_threshold()
    }

    async fn delete_session(&self, token_hash: SessionTokenHash) -> Result<(), DeleteSessionError> {
        self.sessions
            .delete_session(&token_hash)
            .await
            .map_err(|error| DeleteSessionError::Storage(error.into()))
    }

    async fn delete_account(&self, actor: &AuthenticatedUser) -> Result<(), DeleteUserError> {
        self.users
            .delete_user(actor.id())
            .await
            .map_err(|err| DeleteUserError::Storage(err.into()))
    }
}

impl<U, P, S, C, G> UserAdminService for AuthServiceImpl<U, P, S, C, G>
where
    U: UserRepository,
    P: PasswordHasher,
    S: SessionRepository,
    C: Clock,
    G: SessionTokenGenerator,
{
    async fn list_users(&self, _actor: &AdminActor) -> Result<Vec<UserSummary>, ListUsersError> {
        let users = self
            .users
            .list_users()
            .await
            .map_err(|error| ListUsersError::Storage(error.into()))?;

        Ok(users.iter().map(UserSummary::from_user).collect())
    }

    async fn get_user(
        &self,
        _actor: &AdminActor,
        target: &UserId,
    ) -> Result<Option<UserSummary>, GetUserError> {
        self.users
            .find_by_id(target)
            .await
            .map(|user| user.as_ref().map(UserSummary::from_user))
            .map_err(|error| GetUserError::Storage(error.into()))
    }

    async fn create_user(
        &self,
        _actor: &AdminActor,
        request: CreateUserRequest,
    ) -> Result<UserSummary, CreateUserError> {
        let user = self
            .insert_account(
                request.email,
                request.username,
                request.password,
                request.role,
            )
            .await
            .map_err(|error| match error {
                CreateAccountFailure::EmailTaken => CreateUserError::EmailTaken,
                CreateAccountFailure::Storage(error) => CreateUserError::Storage(error),
            })?;

        Ok(UserSummary::from_user(&user))
    }

    async fn change_user_email(
        &self,
        _actor: &AdminActor,
        target: &UserId,
        email: UserEmail,
    ) -> Result<UserSummary, ChangeUserEmailError> {
        let user = self
            .users
            .find_by_id(target)
            .await
            .map_err(|error| ChangeUserEmailError::Storage(error.into()))?
            .ok_or(ChangeUserEmailError::NotFound)?;

        let user = self
            .apply_email(user, email)
            .await
            .map_err(|error| match error {
                EmailChangeFailure::EmailTaken => ChangeUserEmailError::EmailTaken,
                EmailChangeFailure::Storage(error) => ChangeUserEmailError::Storage(error),
            })?;

        Ok(UserSummary::from_user(&user))
    }

    async fn change_user_username(
        &self,
        _actor: &AdminActor,
        target: &UserId,
        username: Option<Username>,
    ) -> Result<UserSummary, ChangeUserUsernameError> {
        let user = self
            .users
            .find_by_id(target)
            .await
            .map_err(|error| ChangeUserUsernameError::Storage(error.into()))?
            .ok_or(ChangeUserUsernameError::NotFound)?;

        let user = self
            .apply_username(user, username)
            .await
            .map_err(|error| ChangeUserUsernameError::Storage(error))?;

        Ok(UserSummary::from_user(&user))
    }

    async fn reset_user_password(
        &self,
        _actor: &AdminActor,
        target: &UserId,
        password: String,
    ) -> Result<(), ResetUserPasswordError> {
        let user = self
            .users
            .find_by_id(target)
            .await
            .map_err(|error| ResetUserPasswordError::Storage(error.into()))?
            .ok_or(ResetUserPasswordError::NotFound)?;

        self.apply_password(user, password)
            .await
            .map_err(|error| ResetUserPasswordError::Storage(error))
    }

    async fn set_user_role(
        &self,
        _actor: &AdminActor,
        target: &UserId,
        role: UserRole,
    ) -> Result<UserSummary, SetUserRoleError> {
        let user = self
            .users
            .find_by_id(target)
            .await
            .map_err(|error| SetUserRoleError::Storage(error.into()))?
            .ok_or(SetUserRoleError::NotFound)?;

        if user.role().is_admin() && !role.is_admin() {
            let admins = self
                .users
                .count_admins()
                .await
                .map_err(|error| SetUserRoleError::Storage(error.into()))?;

            if admins <= 1 {
                return Err(SetUserRoleError::LastAdmin);
            }
        }

        let user = user.with_role(role);
        self.users
            .save_user(user.clone())
            .await
            .map_err(|error| SetUserRoleError::Storage(error.into()))?;

        Ok(UserSummary::from_user(&user))
    }

    async fn delete_user(
        &self,
        _actor: &AdminActor,
        target: &UserId,
    ) -> Result<(), DeleteUserError> {
        self.users
            .delete_user(target)
            .await
            .map_err(|err| DeleteUserError::Storage(err.into()))
    }
}

impl<U, P, S, C, G> AuthServiceImpl<U, P, S, C, G>
where
    U: UserRepository,
    P: PasswordHasher,
    S: SessionRepository,
    C: Clock,
    G: SessionTokenGenerator,
{
    async fn load_user(&self, id: &UserId) -> anyhow::Result<AuthUser> {
        self.users
            .find_by_id(id)
            .await
            .map_err(anyhow::Error::from)?
            .ok_or_else(|| anyhow::anyhow!("user `{}` not found", id.as_str()))
    }

    async fn verify_credentials(
        &self,
        email: UserEmail,
        password: String,
    ) -> Result<AuthUser, LoginError> {
        let Some(user) = self
            .users
            .find_by_email(&email)
            .await
            .map_err(|error| LoginError::Storage(error.into()))?
        else {
            return Err(LoginError::InvalidCredentials);
        };

        let matches = self
            .passwords
            .verify_password(&password, user.password_hash())
            .await
            .map_err(|error| LoginError::Storage(error.into()))?;

        if matches {
            Ok(user)
        } else {
            Err(LoginError::InvalidCredentials)
        }
    }

    async fn register_user(&self, request: RegisterRequest) -> Result<LoginResult, RegisterError> {
        let user = self
            .insert_account(
                request.email,
                request.username,
                request.password,
                request.role,
            )
            .await
            .map_err(|error| match error {
                CreateAccountFailure::EmailTaken => RegisterError::EmailTaken,
                CreateAccountFailure::Storage(error) => RegisterError::Storage(error),
            })?;

        let session = self
            .start_session(user.id().clone())
            .await
            .map_err(RegisterError::SessionError)?;

        Ok(LoginResult::new(
            AuthenticatedUser::from_user(&user),
            session,
        ))
    }

    async fn start_session(&self, user_id: UserId) -> Result<IssuedSession, StartSessionError> {
        let generated = self.tokens.generate();
        let expires_at = self.clock.now() + self.policy.lifetime();

        self.sessions
            .save_session(SessionRecord::new(
                generated.token_hash().clone(),
                user_id,
                expires_at,
            ))
            .await
            .map_err(|error| StartSessionError::Storage(error.into()))?;

        Ok(IssuedSession::new(
            generated.token(),
            generated.token_hash().clone(),
            expires_at,
        ))
    }
}

impl<U, P, S, C, G> AuthServiceImpl<U, P, S, C, G>
where
    U: UserRepository,
    P: PasswordHasher,
{
    async fn insert_account(
        &self,
        email: UserEmail,
        username: Option<Username>,
        password: String,
        role: UserRole,
    ) -> Result<AuthUser, CreateAccountFailure> {
        if self
            .users
            .find_by_email(&email)
            .await
            .map_err(|error| CreateAccountFailure::Storage(error.into()))?
            .is_some()
        {
            return Err(CreateAccountFailure::EmailTaken);
        }

        let password_hash = self
            .passwords
            .hash_password(&password)
            .await
            .map_err(|error| CreateAccountFailure::Storage(error.into()))?;

        let user = AuthUser::new(
            UserId::new(Uuid::new_v4().to_string()),
            email,
            username,
            password_hash,
            role,
        );

        self.users
            .save_user(user.clone())
            .await
            .map_err(|error| CreateAccountFailure::Storage(error.into()))?;

        Ok(user)
    }

    async fn apply_email(
        &self,
        user: AuthUser,
        new_email: UserEmail,
    ) -> Result<AuthUser, EmailChangeFailure> {
        if let Some(existing) = self
            .users
            .find_by_email(&new_email)
            .await
            .map_err(|error| EmailChangeFailure::Storage(error.into()))?
            && existing.id() != user.id()
        {
            return Err(EmailChangeFailure::EmailTaken);
        }

        let user = user.with_email(new_email);
        self.users
            .save_user(user.clone())
            .await
            .map_err(|error| EmailChangeFailure::Storage(error.into()))?;

        Ok(user)
    }

    async fn apply_username(
        &self,
        user: AuthUser,
        username: Option<Username>,
    ) -> anyhow::Result<AuthUser> {
        let user = user.with_username(username);
        self.users
            .save_user(user.clone())
            .await
            .map_err(anyhow::Error::from)?;

        Ok(user)
    }

    async fn apply_password(&self, user: AuthUser, password: String) -> anyhow::Result<()> {
        let password_hash = self
            .passwords
            .hash_password(&password)
            .await
            .map_err(anyhow::Error::from)?;

        self.users
            .save_user(user.with_password_hash(password_hash))
            .await
            .map_err(anyhow::Error::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use cookbook_domain::user::models::{UserId, UserRole, Username};

    use super::*;
    use crate::auth::models::{GeneratedSessionToken, PasswordHash};
    use crate::auth::ports::{
        LoginError, MockPasswordHasher, MockSessionRepository, MockUserRepository, RegisterError,
        UserRepositoryError,
    };

    #[derive(Clone)]
    struct FixedClock(SystemTime);

    impl Clock for FixedClock {
        fn now(&self) -> SystemTime {
            self.0
        }
    }

    #[derive(Clone)]
    struct FixedTokenGenerator(GeneratedSessionToken);

    impl SessionTokenGenerator for FixedTokenGenerator {
        fn generate(&self) -> GeneratedSessionToken {
            self.0.clone()
        }
    }

    fn policy() -> SessionPolicy {
        SessionPolicy::new(Duration::from_hours(1), Duration::from_mins(10))
    }

    fn token_generator() -> FixedTokenGenerator {
        FixedTokenGenerator(GeneratedSessionToken::new(
            "fixed-token",
            SessionTokenHash::new([9; 32]),
        ))
    }

    fn test_service<U, P, S>(
        users: U,
        passwords: P,
        sessions: S,
        now: SystemTime,
    ) -> AuthServiceImpl<U, P, S, FixedClock, FixedTokenGenerator>
    where
        U: UserRepository,
        P: PasswordHasher,
        S: SessionRepository,
    {
        AuthServiceImpl::new(
            users,
            passwords,
            sessions,
            FixedClock(now),
            token_generator(),
            policy(),
        )
    }

    fn user_email(value: &str) -> UserEmail {
        UserEmail::try_new(value).unwrap()
    }

    fn request(email: &str) -> RegisterRequest {
        RegisterRequest {
            email: user_email(email),
            username: Some(Username::new("Alice".to_string())),
            password: "correct horse battery".to_string(),
            role: UserRole::User,
        }
    }

    fn user(email: &str) -> AuthUser {
        AuthUser::new(
            UserId::new("user-1".to_string()),
            user_email(email),
            Some(Username::new("Alice".to_string())),
            PasswordHash::new("stored-hash"),
            UserRole::User,
        )
    }

    fn authenticated(email: &str) -> AuthenticatedUser {
        AuthenticatedUser::from_user(&user(email))
    }

    fn admin_actor() -> AdminActor {
        AdminActor::try_from(&AuthenticatedUser::new(
            UserId::new("admin-1".to_string()),
            user_email("admin@example.com"),
            None,
            UserRole::Admin,
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn register_hashes_the_password_and_saves_the_user() {
        let mut users = MockUserRepository::new();
        users.expect_find_by_email().returning(|_| Ok(None));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.email().as_str(), "alice@example.com");
            assert!(!user.role().is_admin());
            Ok(())
        });

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_hash_password()
            .returning(|password| Ok(PasswordHash::new(format!("hash:{password}"))));

        let mut sessions = MockSessionRepository::new();
        sessions.expect_save_session().returning(|_| Ok(()));

        let service = test_service(users, passwords, sessions, SystemTime::now());

        let res = service
            .register(request("  Alice@Example.com "))
            .await
            .unwrap();

        assert_eq!(res.user().email().as_str(), "alice@example.com");
    }

    #[tokio::test]
    async fn register_rejects_duplicate_emails() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_email()
            .returning(|_| Ok(Some(user("alice@example.com"))));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service.register(request("alice@example.com")).await;

        assert!(matches!(result, Err(RegisterError::EmailTaken)));
    }

    #[tokio::test]
    async fn register_initial_admin_is_a_no_op_when_users_exist() {
        let mut users = MockUserRepository::new();
        users.expect_count_users().returning(|| Ok(1));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .register_initial_admin(request("admin@example.com"))
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn login_returns_the_user_for_valid_credentials() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_email()
            .returning(|_| Ok(Some(user("alice@example.com"))));

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_verify_password()
            .returning(|_, _| Ok(true));

        let mut sessions = MockSessionRepository::new();
        sessions.expect_save_session().returning(|_| Ok(()));

        let service = test_service(users, passwords, sessions, SystemTime::now());

        let result = service
            .login(user_email("alice@example.com"), "password".to_string())
            .await
            .unwrap();

        assert_eq!(result.user().id().as_str(), "user-1");
        assert_eq!(result.session().token(), "fixed-token");
    }

    #[tokio::test]
    async fn login_rejects_unknown_emails_and_wrong_passwords() {
        let mut users = MockUserRepository::new();
        users.expect_find_by_email().returning(|_| Ok(None));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .login(user_email("nobody@example.com"), "password".to_string())
            .await;

        assert!(matches!(result, Err(LoginError::InvalidCredentials)));

        let mut users = MockUserRepository::new();
        users
            .expect_find_by_email()
            .returning(|_| Ok(Some(user("alice@example.com"))));

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_verify_password()
            .returning(|_, _| Ok(false));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .login(user_email("alice@example.com"), "wrong".to_string())
            .await;

        assert!(matches!(result, Err(LoginError::InvalidCredentials)));
    }

    #[tokio::test]
    async fn change_email_verifies_the_password_and_saves_a_normalized_email() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_find_by_email().returning(|_| Ok(None));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.email().as_str(), "new@example.com");
            Ok(())
        });

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_verify_password()
            .returning(|_, _| Ok(true));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let updated = service
            .change_email(
                &authenticated("alice@example.com"),
                "correct horse battery".to_string(),
                user_email(" New@Example.com "),
            )
            .await
            .unwrap();

        assert_eq!(updated.email().as_str(), "new@example.com");
    }

    #[tokio::test]
    async fn change_email_rejects_an_incorrect_current_password() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_verify_password()
            .returning(|_, _| Ok(false));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .change_email(
                &authenticated("alice@example.com"),
                "wrong".to_string(),
                user_email("new@example.com"),
            )
            .await;

        assert!(matches!(result, Err(ChangeEmailError::InvalidPassword)));
    }

    #[tokio::test]
    async fn change_email_rejects_an_email_used_by_another_user() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_find_by_email().returning(|_| {
            Ok(Some(AuthUser::new(
                UserId::new("user-2".to_string()),
                user_email("taken@example.com"),
                None,
                PasswordHash::new("stored-hash"),
                UserRole::User,
            )))
        });

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_verify_password()
            .returning(|_, _| Ok(true));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .change_email(
                &authenticated("alice@example.com"),
                "correct horse battery".to_string(),
                user_email("taken@example.com"),
            )
            .await;

        assert!(matches!(result, Err(ChangeEmailError::EmailTaken)));
    }

    #[tokio::test]
    async fn change_email_allows_keeping_the_same_email() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users
            .expect_find_by_email()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_save_user().returning(|_| Ok(()));

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_verify_password()
            .returning(|_, _| Ok(true));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let updated = service
            .change_email(
                &authenticated("alice@example.com"),
                "correct horse battery".to_string(),
                user_email("alice@example.com"),
            )
            .await
            .unwrap();

        assert_eq!(updated.email().as_str(), "alice@example.com");
    }

    #[tokio::test]
    async fn change_username_updates_and_clears_the_username() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .times(2)
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_save_user().returning(|_| Ok(()));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let actor = authenticated("alice@example.com");

        let updated = service
            .change_username(&actor, Some(Username::new("New Name".to_string())))
            .await
            .unwrap();

        assert_eq!(updated.username().map(Username::as_str), Some("New Name"));

        let cleared = service.change_username(&actor, None).await.unwrap();

        assert_eq!(cleared.username(), None);
        assert_eq!(cleared.display_name(), "alice@example.com");
    }

    #[tokio::test]
    async fn change_password_verifies_and_hashes_the_new_password() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.password_hash().as_str(), "hash:new password");
            Ok(())
        });

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_verify_password()
            .returning(|_, _| Ok(true));
        passwords
            .expect_hash_password()
            .returning(|password| Ok(PasswordHash::new(format!("hash:{password}"))));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        service
            .change_password(
                &authenticated("alice@example.com"),
                "correct horse battery".to_string(),
                "new password".to_string(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn change_password_rejects_an_incorrect_current_password() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_verify_password()
            .returning(|_, _| Ok(false));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .change_password(
                &authenticated("alice@example.com"),
                "wrong".to_string(),
                "new password".to_string(),
            )
            .await;

        assert!(matches!(result, Err(ChangePasswordError::InvalidPassword)));
    }

    #[tokio::test]
    async fn start_session_creates_the_token_and_expiry() {
        let now = SystemTime::now();

        let mut sessions = MockSessionRepository::new();
        sessions.expect_save_session().returning(move |session| {
            assert_eq!(session.token_hash(), &SessionTokenHash::new([9; 32]));
            assert_eq!(session.user_id().as_str(), "user-1");
            assert_eq!(session.expires_at(), now + Duration::from_hours(1));
            Ok(())
        });

        let service = test_service(
            MockUserRepository::new(),
            MockPasswordHasher::new(),
            sessions,
            now,
        );

        let issued = service
            .start_session(UserId::new("user-1".to_string()))
            .await
            .unwrap();

        assert_eq!(issued.token(), "fixed-token");
        assert_eq!(issued.token_hash(), &SessionTokenHash::new([9; 32]));
        assert_eq!(issued.expires_at(), now + Duration::from_hours(1));
    }

    #[tokio::test]
    async fn authenticate_session_rejects_and_deletes_expired_sessions() {
        let now = SystemTime::now();
        let hash = SessionTokenHash::new([7; 32]);
        let expected_hash = hash.clone();

        let mut sessions = MockSessionRepository::new();
        sessions.expect_find_session().returning(move |_| {
            Ok(Some(SessionRecord::new(
                expected_hash.clone(),
                UserId::new("user-1".to_string()),
                now - Duration::from_secs(1),
            )))
        });
        sessions.expect_delete_session().returning(|_| Ok(()));

        let service = test_service(
            MockUserRepository::new(),
            MockPasswordHasher::new(),
            sessions,
            now,
        );

        let result = service.authenticate_session(hash).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn authenticate_session_returns_the_user_for_a_valid_session() {
        let now = SystemTime::now();
        let hash = SessionTokenHash::new([7; 32]);
        let expires_at = now + Duration::from_hours(1);
        let expected_hash = hash.clone();

        let mut sessions = MockSessionRepository::new();
        sessions.expect_find_session().returning(move |_| {
            Ok(Some(SessionRecord::new(
                expected_hash.clone(),
                UserId::new("user-1".to_string()),
                expires_at,
            )))
        });

        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));

        let service = test_service(users, MockPasswordHasher::new(), sessions, now);

        let (authenticated, authenticated_expires_at) = service
            .authenticate_session(hash)
            .await
            .unwrap()
            .expect("session should authenticate");

        assert_eq!(authenticated.display_name(), "Alice");
        assert_eq!(authenticated_expires_at, expires_at);
    }

    #[tokio::test]
    async fn renew_session_extends_the_expiry() {
        let now = SystemTime::now();
        let hash = SessionTokenHash::new([7; 32]);
        let expected_hash = hash.clone();

        let mut sessions = MockSessionRepository::new();
        sessions.expect_find_session().returning(move |_| {
            Ok(Some(SessionRecord::new(
                expected_hash.clone(),
                UserId::new("user-1".to_string()),
                now - Duration::from_secs(1),
            )))
        });
        sessions.expect_save_session().returning(move |session| {
            assert_eq!(session.expires_at(), now + Duration::from_hours(1));
            Ok(())
        });

        let service = test_service(
            MockUserRepository::new(),
            MockPasswordHasher::new(),
            sessions,
            now,
        );

        let renewed = service.renew_session(hash).await.unwrap();

        assert_eq!(renewed, Some(now + Duration::from_hours(1)));
    }

    #[test]
    fn session_needs_renewal_uses_the_policy_threshold() {
        let now = SystemTime::now();
        let service = test_service(
            MockUserRepository::new(),
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            now,
        );

        assert!(service.session_needs_renewal(now + Duration::from_mins(5)));
        assert!(!service.session_needs_renewal(now + Duration::from_mins(30)));
    }

    fn admin_user(email: &str) -> AuthUser {
        user(email).with_role(UserRole::Admin)
    }

    fn create_user_request(email: &str, role: UserRole) -> CreateUserRequest {
        CreateUserRequest {
            email: user_email(email),
            username: Some(Username::new("Alice".to_string())),
            password: "correct horse battery".to_string(),
            role,
        }
    }

    #[tokio::test]
    async fn create_user_hashes_the_password_and_saves_the_requested_role() {
        let mut users = MockUserRepository::new();
        users.expect_find_by_email().returning(|_| Ok(None));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.email().as_str(), "new@example.com");
            assert_eq!(user.role(), UserRole::Admin);
            assert_eq!(user.password_hash().as_str(), "hash:secret");
            Ok(())
        });

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_hash_password()
            .returning(|password| Ok(PasswordHash::new(format!("hash:{password}"))));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let mut request = create_user_request(" New@Example.com ", UserRole::Admin);
        request.password = "secret".to_string();

        let created = service.create_user(&admin_actor(), request).await.unwrap();

        assert_eq!(created.email().as_str(), "new@example.com");
        assert_eq!(created.role(), UserRole::Admin);
    }

    #[tokio::test]
    async fn create_user_rejects_duplicate_emails() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_email()
            .returning(|_| Ok(Some(user("taken@example.com"))));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .create_user(
                &admin_actor(),
                create_user_request("taken@example.com", UserRole::User),
            )
            .await;

        assert!(matches!(result, Err(CreateUserError::EmailTaken)));
    }

    #[tokio::test]
    async fn change_user_email_does_not_require_the_current_password() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_find_by_email().returning(|_| Ok(None));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.email().as_str(), "new@example.com");
            Ok(())
        });

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let updated = service
            .change_user_email(
                &admin_actor(),
                &UserId::new("user-1".to_string()),
                user_email(" New@Example.com "),
            )
            .await
            .unwrap();

        assert_eq!(updated.email().as_str(), "new@example.com");
    }

    #[tokio::test]
    async fn change_user_email_rejects_a_taken_email_and_missing_users() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_find_by_email().returning(|_| {
            Ok(Some(AuthUser::new(
                UserId::new("user-2".to_string()),
                user_email("taken@example.com"),
                None,
                PasswordHash::new("stored-hash"),
                UserRole::User,
            )))
        });

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .change_user_email(
                &admin_actor(),
                &UserId::new("user-1".to_string()),
                user_email("taken@example.com"),
            )
            .await;

        assert!(matches!(result, Err(ChangeUserEmailError::EmailTaken)));

        let mut users = MockUserRepository::new();
        users.expect_find_by_id().returning(|_| Ok(None));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .change_user_email(
                &admin_actor(),
                &UserId::new("missing".to_string()),
                user_email("new@example.com"),
            )
            .await;

        assert!(matches!(result, Err(ChangeUserEmailError::NotFound)));
    }

    #[tokio::test]
    async fn change_user_username_updates_and_clears_the_username() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.username().map(Username::as_str), None);
            Ok(())
        });

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let updated = service
            .change_user_username(&admin_actor(), &UserId::new("user-1".to_string()), None)
            .await
            .unwrap();

        assert!(updated.username().is_none());
    }

    #[tokio::test]
    async fn reset_user_password_hashes_without_verifying_the_current_password() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.password_hash().as_str(), "hash:new-password");
            Ok(())
        });

        let mut passwords = MockPasswordHasher::new();
        passwords
            .expect_hash_password()
            .returning(|password| Ok(PasswordHash::new(format!("hash:{password}"))));

        let service = test_service(
            users,
            passwords,
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        service
            .reset_user_password(
                &admin_actor(),
                &UserId::new("user-1".to_string()),
                "new-password".to_string(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn delete_account_deletes_the_user() {
        let mut users = MockUserRepository::new();
        users
            .expect_delete_user()
            .withf(|id| id.as_str() == "user-1")
            .returning(|_| Ok(()));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        service
            .delete_account(&authenticated("alice@example.com"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn delete_account_maps_storage_errors() {
        let mut users = MockUserRepository::new();
        users.expect_delete_user().returning(|_| {
            Err(UserRepositoryError::Storage(anyhow::anyhow!(
                "database unavailable"
            )))
        });

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .delete_account(&authenticated("alice@example.com"))
            .await;

        assert!(matches!(result, Err(DeleteUserError::Storage(_))));
    }

    #[tokio::test]
    async fn set_user_role_promotes_and_demotes_with_another_admin_present() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(user("alice@example.com"))));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.role(), UserRole::Admin);
            Ok(())
        });

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let updated = service
            .set_user_role(
                &admin_actor(),
                &UserId::new("user-1".to_string()),
                UserRole::Admin,
            )
            .await
            .unwrap();

        assert_eq!(updated.role(), UserRole::Admin);

        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(admin_user("alice@example.com"))));
        users.expect_count_admins().returning(|| Ok(2));
        users.expect_save_user().returning(|user| {
            assert_eq!(user.role(), UserRole::User);
            Ok(())
        });

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let updated = service
            .set_user_role(
                &admin_actor(),
                &UserId::new("user-1".to_string()),
                UserRole::User,
            )
            .await
            .unwrap();

        assert_eq!(updated.role(), UserRole::User);
    }

    #[tokio::test]
    async fn set_user_role_rejects_demoting_the_last_admin() {
        let mut users = MockUserRepository::new();
        users
            .expect_find_by_id()
            .returning(|_| Ok(Some(admin_user("alice@example.com"))));
        users.expect_count_admins().returning(|| Ok(1));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .set_user_role(
                &admin_actor(),
                &UserId::new("user-1".to_string()),
                UserRole::User,
            )
            .await;

        assert!(matches!(result, Err(SetUserRoleError::LastAdmin)));
    }

    #[tokio::test]
    async fn delete_user_deletes_the_target_user() {
        let mut users = MockUserRepository::new();
        users
            .expect_delete_user()
            .withf(|id| id.as_str() == "target-user")
            .returning(|_| Ok(()));

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        service
            .delete_user(&admin_actor(), &UserId::new("target-user".to_string()))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn delete_user_maps_storage_errors() {
        let mut users = MockUserRepository::new();
        users.expect_delete_user().returning(|_| {
            Err(UserRepositoryError::Storage(anyhow::anyhow!(
                "database unavailable"
            )))
        });

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let result = service
            .delete_user(&admin_actor(), &UserId::new("target-user".to_string()))
            .await;

        assert!(matches!(result, Err(DeleteUserError::Storage(_))));
    }

    #[tokio::test]
    async fn list_users_returns_summaries_without_password_hashes() {
        let mut users = MockUserRepository::new();
        users.expect_list_users().returning(|| {
            Ok(vec![
                user("alice@example.com"),
                admin_user("admin@example.com"),
            ])
        });

        let service = test_service(
            users,
            MockPasswordHasher::new(),
            MockSessionRepository::new(),
            SystemTime::now(),
        );

        let summaries = service.list_users(&admin_actor()).await.unwrap();

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].display_name(), "Alice");
        assert_eq!(summaries[1].role(), UserRole::Admin);
    }
}
