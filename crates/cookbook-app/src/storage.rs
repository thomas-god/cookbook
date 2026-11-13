use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::env;

const DATA_DIR_ENV: &str = "COOKBOOK_DATA_DIR";
const DEFAULT_DATA_DIR: &str = "data";
const DATABASE_FILE_NAME: &str = "cookbook.sqlite3";

/// Startup configuration for the persistent data directory.
#[derive(Debug, Clone)]
pub struct DataConfig {
    data_dir: PathBuf,
}

impl DataConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let data_dir = env::optional(DATA_DIR_ENV)?
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_DIR));

        Ok(Self { data_dir })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join(DATABASE_FILE_NAME)
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
        let config = DataConfig {
            data_dir: PathBuf::from("some/data"),
        };

        assert_eq!(
            config.database_path(),
            PathBuf::from("some/data/cookbook.sqlite3")
        );
    }
}
