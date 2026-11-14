//! Scheduled online SQLite backups.
//!
//! The scheduler runs in-process using standard five-field cron expressions
//! evaluated in UTC. A failed backup is logged and does not stop the server.

use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, bail};
use chrono::Utc;
use cookbook_infra::storage::backup_database;
use cron::Schedule;

use crate::{env, storage::DataConfig};

const ENABLED_ENV: &str = "COOKBOOK_BACKUP_ENABLED";
const SCHEDULE_ENV: &str = "COOKBOOK_BACKUP_SCHEDULE";
const OUTPUT_DIR_ENV: &str = "COOKBOOK_BACKUP_DIR";
const KEEP_ENV: &str = "COOKBOOK_BACKUP_KEEP";

const DEFAULT_SCHEDULE: &str = "0 3 * * *";
const DEFAULT_KEEP: usize = 14;

/// Configuration for automatic database backups.
#[derive(Debug, Clone)]
pub struct BackupSettings {
    enabled: bool,
    schedule_text: String,
    schedule: Schedule,
    source: PathBuf,
    output_dir: PathBuf,
    keep: usize,
}

impl BackupSettings {
    pub fn from_env(data: &DataConfig) -> anyhow::Result<Self> {
        let enabled = match env::optional(ENABLED_ENV)? {
            Some(value) => parse_bool(ENABLED_ENV, &value)?,
            None => true,
        };

        let schedule_text =
            env::optional(SCHEDULE_ENV)?.unwrap_or_else(|| DEFAULT_SCHEDULE.to_string());
        let schedule = parse_schedule(&schedule_text)?;

        let keep = match env::optional(KEEP_ENV)? {
            Some(value) => value
                .parse()
                .with_context(|| format!("invalid {KEEP_ENV} value `{value}`"))?,
            None => DEFAULT_KEEP,
        };

        if keep == 0 {
            bail!("{KEEP_ENV} must be at least 1");
        }

        let output_dir = match env::optional(OUTPUT_DIR_ENV)? {
            Some(value) => PathBuf::from(value),
            None => data.data_dir().join("backups"),
        };

        Ok(Self {
            enabled,
            schedule_text,
            schedule,
            source: data.database_path(),
            output_dir,
            keep,
        })
    }
}

/// Starts the background backup task when automatic backups are enabled.
pub fn spawn(settings: BackupSettings) {
    if !settings.enabled {
        tracing::info!("automatic database backups are disabled");
        return;
    }

    tracing::info!(
        schedule = %settings.schedule_text,
        output_dir = %settings.output_dir.display(),
        keep = settings.keep,
        "automatic database backups enabled"
    );

    tokio::spawn(run(settings));
}

async fn run(settings: BackupSettings) {
    loop {
        let Some(next) = settings.schedule.upcoming(Utc).next() else {
            tracing::warn!(
                schedule = %settings.schedule_text,
                "backup schedule has no future occurrences"
            );
            return;
        };

        tracing::info!(next = %next, "next automatic database backup scheduled");

        let delay = (next - Utc::now()).to_std().unwrap_or_default();
        tokio::time::sleep(delay).await;

        match backup_database(&settings.source, &settings.output_dir, settings.keep).await {
            Ok(outcome) => tracing::info!(
                path = %outcome.path.display(),
                removed = outcome.removed,
                "automatic database backup complete"
            ),
            Err(error) => tracing::warn!(
                source = %settings.source.display(),
                output_dir = %settings.output_dir.display(),
                %error,
                "automatic database backup failed"
            ),
        }
    }
}

fn parse_schedule(value: &str) -> anyhow::Result<Schedule> {
    let fields = value.split_whitespace().count();

    if fields != 5 {
        bail!(
            "invalid {SCHEDULE_ENV} value `{value}`: expected 5 fields \
             (minute hour day-of-month month day-of-week), got {fields}"
        );
    }

    // The cron crate includes a seconds field; a five-field expression always
    // runs at second zero.
    Schedule::from_str(&format!("0 {value}"))
        .with_context(|| format!("invalid {SCHEDULE_ENV} value `{value}`"))
}

fn parse_bool(name: &str, value: &str) -> anyhow::Result<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => bail!("invalid {name} value `{value}`: expected true or false"),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;

    #[test]
    fn accepts_five_field_cron_expressions() {
        let schedule = parse_schedule("0 3 * * *").unwrap();
        assert!(schedule.upcoming(Utc).next().is_some());
    }

    #[test]
    fn rejects_expressions_without_five_fields() {
        for value in ["", "0 3 * *", "0 0 3 * * *", "not a schedule"] {
            let error = parse_schedule(value).unwrap_err();
            assert!(error.to_string().contains("5 fields"), "{value}: {error}");
        }
    }

    #[test]
    fn parses_common_boolean_spellings() {
        for value in ["1", "true", "TRUE", "yes", "on"] {
            assert!(parse_bool("FLAG", value).unwrap(), "{value}");
        }

        for value in ["0", "false", "FALSE", "no", "off"] {
            assert!(!parse_bool("FLAG", value).unwrap(), "{value}");
        }

        assert!(parse_bool("FLAG", "maybe").is_err());
    }
}
