use anyhow::{Context, bail};
use cookbook_config::{env, names, parse_bool};
use cookbook_domain::user::models::{UserRole, Username};
use cookbook_infra::auth::{
    models::UserEmail,
    ports::{AuthService, RegisterRequest},
};

use crate::app::AppAuthService;

/// Authentication settings exposed to the web layer.
#[derive(Debug, Clone, Copy)]
pub struct AuthSettings {
    registration_enabled: bool,
}

impl AuthSettings {
    pub fn registration_enabled(&self) -> bool {
        self.registration_enabled
    }
}

/// Startup configuration for authentication.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    settings: AuthSettings,
    bootstrap_admin: Option<BootstrapAdmin>,
}

#[derive(Debug, Clone)]
struct BootstrapAdmin {
    email: String,
    password: String,
    username: Option<String>,
}

impl AuthConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let registration_enabled = match env::optional(names::auth::REGISTRATION_ENABLED)? {
            Some(value) => parse_bool(names::auth::REGISTRATION_ENABLED, &value)?,
            None => false,
        };

        let email = env::optional(names::auth::FIRST_ADMIN_EMAIL)?;
        let password = env::optional(names::auth::FIRST_ADMIN_PASSWORD)?;
        let username = env::optional(names::auth::FIRST_ADMIN_USERNAME)?;

        let bootstrap_admin = match (email, password) {
            (None, None) => {
                if username.is_some() {
                    bail!(
                        "{} requires {} and {}",
                        names::auth::FIRST_ADMIN_USERNAME,
                        names::auth::FIRST_ADMIN_EMAIL,
                        names::auth::FIRST_ADMIN_PASSWORD
                    );
                }
                None
            }
            (Some(email), Some(password)) => Some(BootstrapAdmin {
                email,
                password,
                username,
            }),
            _ => bail!(
                "{} and {} must be set together to create the initial admin",
                names::auth::FIRST_ADMIN_EMAIL,
                names::auth::FIRST_ADMIN_PASSWORD
            ),
        };

        Ok(Self {
            settings: AuthSettings {
                registration_enabled,
            },
            bootstrap_admin,
        })
    }

    pub fn settings(&self) -> AuthSettings {
        self.settings
    }

    /// Creates the initial admin when no user exists yet.
    pub async fn seed_initial_admin(&self, auth: &AppAuthService) -> anyhow::Result<()> {
        let Some(admin) = &self.bootstrap_admin else {
            return Ok(());
        };

        let email = UserEmail::try_new(admin.email.clone())
            .with_context(|| format!("invalid {} value", names::auth::FIRST_ADMIN_EMAIL))?;
        let request = RegisterRequest {
            email,
            username: admin.username.clone().map(Username::new),
            password: admin.password.clone(),
            role: UserRole::Admin,
        };

        auth.register_initial_admin(request)
            .await
            .context("failed to create the initial admin")?;

        Ok(())
    }
}
