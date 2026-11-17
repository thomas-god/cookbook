use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use cookbook_config::{defaults, env, names};

/// Startup configuration for the persistent data directory.
#[derive(Debug, Clone)]
pub struct DataConfig {
    data_dir: PathBuf,
}

impl DataConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = env::optional(names::data::DIR)?
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(defaults::DATA_DIR));

        Ok(Self { data_dir })
    }

    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join(defaults::DATABASE_FILE_NAME)
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.data_dir.join(defaults::BACKUP_DIR_NAME)
    }

    pub fn ensure_directory(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.data_dir).with_context(|| {
            format!(
                "failed to create data directory {}",
                self.data_dir.display()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_path_lives_inside_the_data_directory() {
        let config = DataConfig::new("some/data");

        assert_eq!(
            config.database_path(),
            PathBuf::from("some/data/cookbook.sqlite3")
        );
    }

    #[test]
    fn backup_dir_lives_inside_the_data_directory() {
        let config = DataConfig::new("some/data");

        assert_eq!(config.backup_dir(), PathBuf::from("some/data/backups"));
    }
}
