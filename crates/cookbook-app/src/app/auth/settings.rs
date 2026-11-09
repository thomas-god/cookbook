use std::env;

use anyhow::{Context, bail};
use cookbook_domain::user::models::{UserRole, Username};
use cookbook_infra::auth::{
    models::UserEmail,
    ports::{AuthService, RegisterRequest},
};

use crate::app::AppAuthService;

const REGISTRATION_ENABLED_ENV: &str = "COOKBOOK_REGISTRATION_ENABLED";
const FIRST_ADMIN_EMAIL_ENV: &str = "COOKBOOK_FIRST_ADMIN_EMAIL";
const FIRST_ADMIN_PASSWORD_ENV: &str = "COOKBOOK_FIRST_ADMIN_PASSWORD";
const FIRST_ADMIN_USERNAME_ENV: &str = "COOKBOOK_FIRST_ADMIN_USERNAME";

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
        let registration_enabled = match env::var(REGISTRATION_ENABLED_ENV) {
            Ok(value) => parse_bool(&value)
                .with_context(|| format!("invalid value for {REGISTRATION_ENABLED_ENV}"))?,
            Err(env::VarError::NotPresent) => false,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read {REGISTRATION_ENABLED_ENV}"));
            }
        };

        let email = optional_env(FIRST_ADMIN_EMAIL_ENV);
        let password = optional_env(FIRST_ADMIN_PASSWORD_ENV);
        let username = optional_env(FIRST_ADMIN_USERNAME_ENV);

        let bootstrap_admin = match (email, password) {
            (None, None) => {
                if username.is_some() {
                    bail!(
                        "{FIRST_ADMIN_USERNAME_ENV} requires {FIRST_ADMIN_EMAIL_ENV} and {FIRST_ADMIN_PASSWORD_ENV}"
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
                "{FIRST_ADMIN_EMAIL_ENV} and {FIRST_ADMIN_PASSWORD_ENV} must be set together to create the initial admin"
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

        let request = RegisterRequest {
            email: UserEmail::new(admin.email.clone()),
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

fn optional_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("expected one of true, false, 1, 0, yes, no, on, off"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_boolean_values() {
        for value in ["1", "true", "TRUE", "yes", "on"] {
            assert!(parse_bool(value).unwrap());
        }

        for value in ["0", "false", "FALSE", "no", "off"] {
            assert!(!parse_bool(value).unwrap());
        }

        assert!(parse_bool("sometimes").is_err());
    }
}
