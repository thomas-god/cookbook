use std::time::SystemTime;

use cookbook_domain::user::models::UserId;
use uuid::Uuid;

use crate::auth::models::{
    AuthUser, AuthenticatedUser, IssuedSession, LoginResult, SessionPolicy, SessionRecord,
    SessionTokenHash, UserEmail,
};
use crate::auth::ports::{
    AuthService, AuthenticateSessionError, Clock, DeleteSessionError, LoginError, PasswordHasher,
    RegisterError, RegisterRequest, RenewSessionError, SessionRepository, SessionTokenGenerator,
    StartSessionError, UserRepository,
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
}

impl<U, P, S, C, G> AuthServiceImpl<U, P, S, C, G>
where
    U: UserRepository,
    P: PasswordHasher,
    S: SessionRepository,
    C: Clock,
    G: SessionTokenGenerator,
{
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
        if !valid_email(request.email.as_str()) {
            return Err(RegisterError::InvalidEmail);
        }

        if self
            .users
            .find_by_email(&request.email)
            .await
            .map_err(|error| RegisterError::Storage(error.into()))?
            .is_some()
        {
            return Err(RegisterError::EmailTaken);
        }

        let password_hash = self
            .passwords
            .hash_password(&request.password)
            .await
            .map_err(|error| RegisterError::Storage(error.into()))?;

        let user = AuthUser::new(
            UserId::new(Uuid::new_v4().to_string()),
            request.email,
            request.username,
            password_hash,
            request.role,
        );

        self.users
            .save_user(user.clone())
            .await
            .map_err(|error| RegisterError::Storage(error.into()))?;

        let session = self
            .start_session(user.id().clone())
            .await
            .map_err(|err| RegisterError::SessionError(err))?;

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

fn valid_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };

    !local.is_empty()
        && !domain.is_empty()
        && !domain.contains('@')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use cookbook_domain::user::models::{UserId, UserRole, Username};

    use super::*;
    use crate::auth::models::{GeneratedSessionToken, PasswordHash};
    use crate::auth::ports::{
        LoginError, MockPasswordHasher, MockSessionRepository, MockUserRepository, RegisterError,
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

    fn request(email: &str) -> RegisterRequest {
        RegisterRequest {
            email: UserEmail::new(email),
            username: Some(Username::new("Alice".to_string())),
            password: "correct horse battery".to_string(),
            role: UserRole::User,
        }
    }

    fn user(email: &str) -> AuthUser {
        AuthUser::new(
            UserId::new("user-1".to_string()),
            UserEmail::new(email),
            Some(Username::new("Alice".to_string())),
            PasswordHash::new("stored-hash"),
            UserRole::User,
        )
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
            .login(UserEmail::new("alice@example.com"), "password".to_string())
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
            .login(UserEmail::new("nobody@example.com"), "password".to_string())
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
            .login(UserEmail::new("alice@example.com"), "wrong".to_string())
            .await;

        assert!(matches!(result, Err(LoginError::InvalidCredentials)));
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
}
