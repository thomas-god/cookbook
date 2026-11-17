//! Default values used when an environment variable is not set.

/// Directory that holds the database, uploaded files and backups.
pub const DATA_DIR: &str = "data";

/// SQLite database file name inside the data directory.
pub const DATABASE_FILE_NAME: &str = "cookbook.sqlite3";

/// Backup output directory name inside the data directory.
pub const BACKUP_DIR_NAME: &str = "backups";

/// Number of backup files to retain when `COOKBOOK_BACKUP_KEEP` is unset.
pub const BACKUP_KEEP: usize = 14;

/// Five-field cron expression used when `COOKBOOK_BACKUP_SCHEDULE` is unset.
pub const BACKUP_SCHEDULE: &str = "0 3 * * *";

/// Request timeout in seconds for the primary vision endpoint.
pub const VISION_PRIMARY_TIMEOUT_SECONDS: u64 = 180;

/// Request timeout in seconds for the backup vision endpoint.
pub const VISION_BACKUP_TIMEOUT_SECONDS: u64 = 120;
