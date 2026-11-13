use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

/// A cloneable handle to the application's SQLite database.
///
/// Connections are pooled so WAL can serve reads concurrently, including while
/// a write transaction is open. SQLite still allows only one writer at a time,
/// so repository write paths should use `BEGIN IMMEDIATE`.
#[derive(Clone)]
pub struct SqliteDatabase {
    pool: SqlitePool,
}

impl SqliteDatabase {
    /// Opens a file-backed database, creating its parent directory when needed.
    pub async fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create SQLite database directory {}",
                    parent.display()
                )
            })?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        Self::connect(options, 4).await
    }

    #[cfg(test)]
    pub async fn open_in_memory() -> anyhow::Result<Self> {
        use std::str::FromStr;

        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .context("failed to parse in-memory SQLite options")?
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5))
            .foreign_keys(true);

        // A single connection keeps the in-memory schema available to every
        // checkout. Separate in-memory connections would each see their own DB.
        Self::connect(options, 1).await
    }

    async fn connect(options: SqliteConnectOptions, max_connections: u32) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .connect_with(options)
            .await
            .context("failed to connect to SQLite database")?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .context("failed to run SQLite migrations")?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use sqlx::Connection;
    use tempfile::tempdir;

    use super::*;

    async fn count_rows(pool: &SqlitePool, table: &str) -> anyhow::Result<i64> {
        let query = match table {
            "users" => "SELECT COUNT(*) FROM users",
            "sessions" => "SELECT COUNT(*) FROM sessions",
            "recipes" => "SELECT COUNT(*) FROM recipes",
            "recipe_revisions" => "SELECT COUNT(*) FROM recipe_revisions",
            "recipe_images" => "SELECT COUNT(*) FROM recipe_images",
            "recipe_source_documents" => "SELECT COUNT(*) FROM recipe_source_documents",
            _ => anyhow::bail!("unknown table {table}"),
        };

        Ok(sqlx::query_scalar::<_, i64>(query).fetch_one(pool).await?)
    }

    async fn insert_user(pool: &SqlitePool, user_id: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO users (id, email, username, password_hash, role)
             VALUES (?, 'user@example.com', NULL, 'hash', 'user')",
        )
        .bind(user_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    async fn insert_recipe(
        pool: &SqlitePool,
        recipe_id: &str,
        author_id: &str,
    ) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO recipes (id, author_id, scope) VALUES (?, ?, 'editable')")
            .bind(recipe_id)
            .bind(author_id)
            .execute(pool)
            .await?;

        Ok(())
    }

    async fn insert_revision(pool: &SqlitePool, recipe_id: &str) -> anyhow::Result<()> {
        sqlx::query(
            "INSERT INTO recipe_revisions (
                id, recipe_id, editor_id, edited_at_ms, origin, verification,
                content_version, content, title, title_key, category, search_text
            ) VALUES (
                'revision-1', ?, 'user-1', 0, '\"created\"', 'verified',
                1, '{}', 'Soup', 'soup', NULL, 'soup'
            )",
        )
        .bind(recipe_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn opens_a_file_database_and_applies_migrations_once() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("cookbook.sqlite3");

        {
            let database = SqliteDatabase::open(&path).await.unwrap();

            let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
                .fetch_one(database.pool())
                .await
                .unwrap();

            assert_eq!(applied, 1);

            let tables: Vec<String> = sqlx::query_scalar(
                "SELECT name FROM sqlite_master
                 WHERE type = 'table'
                 ORDER BY name",
            )
            .fetch_all(database.pool())
            .await
            .unwrap();

            for expected in [
                "users",
                "sessions",
                "recipes",
                "recipe_revisions",
                "recipe_images",
                "recipe_source_documents",
                "_sqlx_migrations",
            ] {
                assert!(
                    tables.iter().any(|table| table == expected),
                    "missing table {expected}: {tables:?}"
                );
            }
        }

        let database = SqliteDatabase::open(&path).await.unwrap();
        let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_one(database.pool())
            .await
            .unwrap();

        assert_eq!(applied, 1);
    }

    #[tokio::test]
    async fn enables_foreign_key_enforcement() {
        let database = SqliteDatabase::open_in_memory().await.unwrap();

        let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(database.pool())
            .await
            .unwrap();

        assert_eq!(enabled, 1);
    }

    #[tokio::test]
    async fn deleting_a_recipe_cascades_to_owned_rows() {
        let database = SqliteDatabase::open_in_memory().await.unwrap();
        let pool = database.pool();

        insert_user(pool, "user-1").await.unwrap();
        insert_recipe(pool, "recipe-1", "user-1").await.unwrap();
        insert_revision(pool, "recipe-1").await.unwrap();

        sqlx::query(
            "INSERT INTO recipe_images (
                id, recipe_id, content_type, byte_len, added_by, added_at_ms, storage_key
             ) VALUES (
                'image-1', 'recipe-1', 'image/jpeg', 4, 'user-1', 0,
                'recipes/recipe-1/images/image-1'
             )",
        )
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO recipe_source_documents (
                id, recipe_id, content_type, original_name, byte_len, added_by,
                added_at_ms, storage_key
             ) VALUES (
                'document-1', 'recipe-1', 'image/jpeg', 'front.jpg', 4, 'user-1',
                0, 'recipes/recipe-1/documents/document-1'
             )",
        )
        .execute(pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM recipes WHERE id = 'recipe-1'")
            .execute(pool)
            .await
            .unwrap();

        let counts = (
            count_rows(pool, "recipes").await.unwrap(),
            count_rows(pool, "recipe_revisions").await.unwrap(),
            count_rows(pool, "recipe_images").await.unwrap(),
            count_rows(pool, "recipe_source_documents").await.unwrap(),
        );

        assert_eq!(counts, (0, 0, 0, 0));
        assert_eq!(count_rows(pool, "users").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn deleting_a_user_does_not_delete_its_recipes_or_sessions() {
        let database = SqliteDatabase::open_in_memory().await.unwrap();
        let pool = database.pool();

        insert_user(pool, "user-1").await.unwrap();
        insert_recipe(pool, "recipe-1", "user-1").await.unwrap();
        insert_revision(pool, "recipe-1").await.unwrap();

        sqlx::query(
            "INSERT INTO sessions (token_hash, user_id, expires_at_ms)
             VALUES (?, 'user-1', 0)",
        )
        .bind(vec![0_u8; 32])
        .execute(pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM users WHERE id = 'user-1'")
            .execute(pool)
            .await
            .unwrap();

        let counts = (
            count_rows(pool, "recipes").await.unwrap(),
            count_rows(pool, "recipe_revisions").await.unwrap(),
            count_rows(pool, "sessions").await.unwrap(),
        );

        assert_eq!(counts, (1, 1, 1));
    }

    #[tokio::test]
    async fn wal_allows_reads_during_an_open_write_transaction() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("cookbook.sqlite3");
        let database = SqliteDatabase::open(&path).await.unwrap();
        let pool = database.pool();

        let mut writer = pool.acquire().await.unwrap();
        let mut transaction = writer.begin_with("BEGIN IMMEDIATE").await.unwrap();

        sqlx::query(
            "INSERT INTO recipes (id, author_id, scope)
             VALUES ('recipe-1', 'user-1', 'editable')",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();

        let before_commit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recipes")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(before_commit, 0);

        transaction.commit().await.unwrap();

        let after_commit: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recipes")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(after_commit, 1);
    }
}
