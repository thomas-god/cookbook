//! Online, transaction-aware SQLite backups.
//!
//! `VACUUM INTO` reads a consistent snapshot of the source database without
//! stopping the application. In WAL mode it can run while writers continue;
//! only committed data is copied, and the result is a single compacted file
//! with no `-wal` or `-shm` companion.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use chrono::Utc;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

const BUSY_TIMEOUT: Duration = Duration::from_secs(30);
const BACKUP_PREFIX: &str = "cookbook-";
const BACKUP_SUFFIX: &str = ".sqlite3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupOutcome {
    pub path: PathBuf,
    /// Number of older backups removed by retention.
    pub removed: usize,
}

/// Creates a consistent backup of `source` in `output_dir` using `VACUUM INTO`.
///
/// The temporary file is written in the output directory, verified with
/// `PRAGMA quick_check`, synced, and atomically renamed into place. Afterwards
/// at most `keep` backups are retained, including the new one.
pub async fn backup_database(
    source: &Path,
    output_dir: &Path,
    keep: usize,
) -> anyhow::Result<BackupOutcome> {
    if keep == 0 {
        bail!("backup retention must be at least 1");
    }
    if !source.is_file() {
        bail!("SQLite database {} does not exist", source.display());
    }

    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create backup directory {}", output_dir.display()))?;

    let file_name = backup_file_name(output_dir);
    let final_path = output_dir.join(&file_name);
    let temp_path = temp_file_path(output_dir, &file_name);

    if let Err(error) = write_snapshot(source, &temp_path).await {
        remove_temp(&temp_path);
        return Err(error);
    }

    if let Err(error) = verify_snapshot(&temp_path).await {
        remove_temp(&temp_path);
        return Err(error);
    }

    if let Err(error) = sync_file(&temp_path) {
        remove_temp(&temp_path);
        return Err(error)
            .with_context(|| format!("failed to sync backup file {}", temp_path.display()));
    }

    fs::rename(&temp_path, &final_path).with_context(|| {
        format!(
            "failed to move backup from {} to {}",
            temp_path.display(),
            final_path.display()
        )
    })?;

    sync_directory(output_dir)
        .with_context(|| format!("failed to sync backup directory {}", output_dir.display()))?;

    let removed = prune_old_backups(output_dir, keep);

    Ok(BackupOutcome {
        path: final_path,
        removed,
    })
}

async fn write_snapshot(source: &Path, destination: &Path) -> anyhow::Result<()> {
    let options = SqliteConnectOptions::new()
        .filename(source)
        .create_if_missing(false)
        .busy_timeout(BUSY_TIMEOUT)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open SQLite database {}", source.display()))?;

    let destination = destination.to_str().with_context(|| {
        format!(
            "backup destination {} is not valid UTF-8",
            destination.display()
        )
    })?;

    let result = sqlx::query("VACUUM INTO ?")
        .bind(destination)
        .execute(&pool)
        .await
        .with_context(|| format!("VACUUM INTO {destination} failed"));

    pool.close().await;
    result.map(|_| ())
}

async fn verify_snapshot(path: &Path) -> anyhow::Result<()> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(true)
        .busy_timeout(BUSY_TIMEOUT);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open backup for verification {}", path.display()))?;

    let result: Result<String, sqlx::Error> = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(&pool)
        .await;

    pool.close().await;

    let result = result.with_context(|| format!("failed to verify backup {}", path.display()))?;

    if result != "ok" {
        bail!("backup {} failed quick_check: {result}", path.display());
    }

    Ok(())
}

fn backup_file_name(output_dir: &Path) -> String {
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();

    for suffix in 0_u32.. {
        let name = if suffix == 0 {
            format!("{BACKUP_PREFIX}{timestamp}{BACKUP_SUFFIX}")
        } else {
            format!("{BACKUP_PREFIX}{timestamp}-{suffix}{BACKUP_SUFFIX}")
        };

        if !output_dir.join(&name).exists() {
            return name;
        }
    }

    unreachable!("u32 suffixes cannot be exhausted in practice")
}

fn temp_file_path(output_dir: &Path, file_name: &str) -> PathBuf {
    let pid = std::process::id();

    for suffix in 0_u32.. {
        let path = output_dir.join(format!(".{file_name}.{pid}.{suffix}.tmp"));

        if !path.exists() {
            return path;
        }
    }

    unreachable!("u32 suffixes cannot be exhausted in practice")
}

fn remove_temp(path: &Path) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            path = %path.display(),
            %error,
            "failed to remove incomplete backup file"
        );
    }
}

fn sync_file(path: &Path) -> std::io::Result<()> {
    fs::File::open(path)?.sync_all()
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn prune_old_backups(output_dir: &Path, keep: usize) -> usize {
    let entries = match fs::read_dir(output_dir) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::warn!(
                directory = %output_dir.display(),
                %error,
                "failed to list backups for retention"
            );
            return 0;
        }
    };

    let mut backups: Vec<(std::time::SystemTime, String, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let file_name = entry.file_name();
            let file_name = file_name.to_str()?;

            if !file_name.starts_with(BACKUP_PREFIX) || !file_name.ends_with(BACKUP_SUFFIX) {
                return None;
            }

            let path = entry.path();
            if !path.is_file() {
                return None;
            }

            // Modification time orders backups taken within the same second,
            // where the collision suffix does not sort chronologically.
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(std::time::UNIX_EPOCH);

            Some((modified, file_name.to_string(), path))
        })
        .collect();

    backups.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));

    let excess = backups.len().saturating_sub(keep);
    let mut removed = 0;

    for (_, file_name, path) in backups.into_iter().take(excess) {
        match fs::remove_file(&path) {
            Ok(()) => {
                tracing::info!(backup = %file_name, "removed old backup");
                removed += 1;
            }
            Err(error) => tracing::warn!(
                backup = %file_name,
                %error,
                "failed to remove old backup"
            ),
        }
    }

    removed
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use sqlx::Connection;
    use tempfile::tempdir;

    use super::*;
    use crate::storage::SqliteDatabase;

    async fn create_database(path: &Path) -> SqliteDatabase {
        SqliteDatabase::open(path).await.unwrap()
    }

    async fn count_rows(database_path: &Path, table: &str) -> i64 {
        let database = SqliteDatabase::open(database_path).await.unwrap();
        let query = match table {
            "users" => "SELECT COUNT(*) FROM users",
            "recipes" => "SELECT COUNT(*) FROM recipes",
            _ => panic!("unknown table {table}"),
        };
        let count = sqlx::query_scalar(query)
            .fetch_one(database.pool())
            .await
            .unwrap();
        database.pool().close().await;
        count
    }

    #[tokio::test]
    async fn creates_a_valid_backup_with_committed_data() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("cookbook.sqlite3");
        let output = directory.path().join("backups");

        let database = create_database(&source).await;
        sqlx::query(
            "INSERT INTO users (id, email, username, password_hash, role)
             VALUES ('user-1', 'user@example.com', NULL, 'hash', 'user')",
        )
        .execute(database.pool())
        .await
        .unwrap();
        database.pool().close().await;

        let outcome = backup_database(&source, &output, 14).await.unwrap();

        assert!(outcome.path.exists());
        assert_eq!(outcome.removed, 0);
        assert!(
            outcome
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(".sqlite3")
        );
        assert_eq!(count_rows(&outcome.path, "users").await, 1);
    }

    #[tokio::test]
    async fn excludes_uncommitted_data_from_the_snapshot() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("cookbook.sqlite3");
        let output = directory.path().join("backups");

        let database = create_database(&source).await;
        let mut writer = database.pool().acquire().await.unwrap();
        let mut transaction = writer.begin_with("BEGIN IMMEDIATE").await.unwrap();

        sqlx::query(
            "INSERT INTO users (id, email, username, password_hash, role)
             VALUES ('user-1', 'user@example.com', NULL, 'hash', 'user')",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();

        let outcome = backup_database(&source, &output, 14).await.unwrap();

        // The backup ran while the write transaction was still open.
        assert_eq!(count_rows(&outcome.path, "users").await, 0);

        transaction.commit().await.unwrap();

        let outcome = backup_database(&source, &output, 14).await.unwrap();
        assert_eq!(count_rows(&outcome.path, "users").await, 1);
    }

    #[tokio::test]
    async fn keeps_only_the_newest_backups() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("cookbook.sqlite3");
        let output = directory.path().join("backups");

        let database = create_database(&source).await;
        database.pool().close().await;
        fs::create_dir_all(&output).unwrap();

        for name in [
            "cookbook-20200101T000000Z.sqlite3",
            "cookbook-20200102T000000Z.sqlite3",
            "cookbook-20200103T000000Z.sqlite3",
        ] {
            fs::write(output.join(name), b"old backup").unwrap();
        }
        fs::write(output.join("unrelated.sqlite3"), b"keep me").unwrap();

        let outcome = backup_database(&source, &output, 3).await.unwrap();

        assert_eq!(outcome.removed, 1);
        assert!(!output.join("cookbook-20200101T000000Z.sqlite3").exists());
        assert!(output.join("cookbook-20200102T000000Z.sqlite3").exists());
        assert!(output.join("cookbook-20200103T000000Z.sqlite3").exists());
        assert!(outcome.path.exists());
        assert!(output.join("unrelated.sqlite3").exists());

        let backups = fs::read_dir(&output)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_str().unwrap_or_default();
                name.starts_with(BACKUP_PREFIX) && name.ends_with(BACKUP_SUFFIX)
            })
            .count();
        assert_eq!(backups, 3);
    }

    #[tokio::test]
    async fn rejects_a_missing_source_database() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("missing.sqlite3");
        let output = directory.path().join("backups");

        let error = backup_database(&source, &output, 14).await.unwrap_err();

        assert!(error.to_string().contains("does not exist"));
    }

    #[tokio::test]
    async fn rejects_zero_retention() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("cookbook.sqlite3");
        let output = directory.path().join("backups");

        let database = create_database(&source).await;
        database.pool().close().await;

        let error = backup_database(&source, &output, 0).await.unwrap_err();

        assert!(error.to_string().contains("retention"));
    }

    #[tokio::test]
    async fn removes_the_temporary_file_when_backup_fails() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("cookbook.sqlite3");
        let output = directory.path().join("backups");

        let database = create_database(&source).await;
        database.pool().close().await;
        fs::create_dir_all(&output).unwrap();

        // Make the destination directory read-only so VACUUM INTO cannot
        // create its temporary file.
        let mut permissions = fs::metadata(&output).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o500);
        }
        fs::set_permissions(&output, permissions).unwrap();

        let result = backup_database(&source, &output, 14).await;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&output).unwrap().permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&output, permissions).unwrap();

            assert!(result.is_err());
            let remaining = fs::read_dir(&output).unwrap().count();
            assert_eq!(remaining, 0);
        }
    }

    #[tokio::test]
    async fn backup_uses_a_single_file_without_wal_or_shm() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("cookbook.sqlite3");
        let output = directory.path().join("backups");

        let database = create_database(&source).await;
        database.pool().close().await;

        let outcome = backup_database(&source, &output, 14).await.unwrap();

        assert!(!outcome.path.with_extension("sqlite3-wal").exists());
        assert!(!outcome.path.with_extension("sqlite3-shm").exists());
    }

    #[tokio::test]
    async fn a_writer_can_continue_while_the_backup_runs() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("cookbook.sqlite3");
        let output = directory.path().join("backups");

        let database = create_database(&source).await;
        let pool = database.pool().clone();

        let writer = tokio::spawn(async move {
            for index in 0..20 {
                sqlx::query(
                    "INSERT INTO users (id, email, username, password_hash, role)
                     VALUES (?, ?, NULL, 'hash', 'user')",
                )
                .bind(format!("user-{index}"))
                .bind(format!("user-{index}@example.com"))
                .execute(&pool)
                .await
                .unwrap();

                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        // Give the writer a head start, then back up while it is still running.
        tokio::time::sleep(Duration::from_millis(10)).await;
        let outcome = backup_database(&source, &output, 14).await.unwrap();
        writer.await.unwrap();

        assert!(count_rows(&outcome.path, "users").await <= 20);
        assert_eq!(count_rows(&source, "users").await, 20);
    }
}
