//! Names of the environment variables understood by the application and its tools.

pub mod data {
    /// Directory that holds the database, uploaded files and backups.
    pub const DIR: &str = "COOKBOOK_DATA_DIR";
}

pub mod backup {
    /// Whether the in-process backup scheduler runs.
    pub const ENABLED: &str = "COOKBOOK_BACKUP_ENABLED";
    /// Five-field cron expression describing when to back up.
    pub const SCHEDULE: &str = "COOKBOOK_BACKUP_SCHEDULE";
    /// Directory that backup files are written to.
    pub const DIR: &str = "COOKBOOK_BACKUP_DIR";
    /// Number of backup files to retain.
    pub const KEEP: &str = "COOKBOOK_BACKUP_KEEP";
}

pub mod auth {
    /// Whether public user registration is allowed.
    pub const REGISTRATION_ENABLED: &str = "COOKBOOK_REGISTRATION_ENABLED";
    /// Email of the initial admin account.
    pub const FIRST_ADMIN_EMAIL: &str = "COOKBOOK_FIRST_ADMIN_EMAIL";
    /// Password of the initial admin account.
    pub const FIRST_ADMIN_PASSWORD: &str = "COOKBOOK_FIRST_ADMIN_PASSWORD";
    /// Optional display name of the initial admin account.
    pub const FIRST_ADMIN_USERNAME: &str = "COOKBOOK_FIRST_ADMIN_USERNAME";
}

pub mod vision {
    /// Prefix of the primary OpenAI-compatible endpoint variables.
    pub const PRIMARY_PREFIX: &str = "COOKBOOK_VLM_PRIMARY";
    /// Prefix of the fallback OpenAI-compatible endpoint variables.
    pub const BACKUP_PREFIX: &str = "COOKBOOK_VLM_BACKUP";

    /// System prompt used by every endpoint without its own override.
    pub const SYSTEM_PROMPT: &str = "COOKBOOK_VLM_SYSTEM_PROMPT";

    pub const URL_SUFFIX: &str = "URL";
    pub const MODEL_SUFFIX: &str = "MODEL";
    pub const API_KEY_SUFFIX: &str = "API_KEY";
    pub const TIMEOUT_SECONDS_SUFFIX: &str = "TIMEOUT_SECONDS";
    pub const SYSTEM_PROMPT_SUFFIX: &str = "SYSTEM_PROMPT";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_names() -> Vec<String> {
        let mut names = [
            data::DIR,
            backup::ENABLED,
            backup::SCHEDULE,
            backup::DIR,
            backup::KEEP,
            auth::REGISTRATION_ENABLED,
            auth::FIRST_ADMIN_EMAIL,
            auth::FIRST_ADMIN_PASSWORD,
            auth::FIRST_ADMIN_USERNAME,
            vision::SYSTEM_PROMPT,
        ]
        .map(str::to_string)
        .to_vec();

        for prefix in [vision::PRIMARY_PREFIX, vision::BACKUP_PREFIX] {
            for suffix in [
                vision::URL_SUFFIX,
                vision::MODEL_SUFFIX,
                vision::API_KEY_SUFFIX,
                vision::TIMEOUT_SECONDS_SUFFIX,
                vision::SYSTEM_PROMPT_SUFFIX,
            ] {
                names.push(format!("{prefix}_{suffix}"));
            }
        }

        names
    }

    #[test]
    fn names_are_unique_and_prefixed() {
        let names = all_names();

        for name in &names {
            assert!(name.starts_with("COOKBOOK_"), "{name}");
        }

        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();

        assert_eq!(
            sorted.len(),
            names.len(),
            "duplicate environment variable name"
        );
    }
}
