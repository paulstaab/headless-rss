//! Database pool initialization and migration helpers.

use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::Path;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Creates a SQLite pool backed by a file path and applies all migrations.
pub async fn create_pool(db_path: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(Path::new(db_path))
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    initialize_pool(&pool, true).await?;
    Ok(pool)
}

#[cfg(test)]
/// Creates an in-memory SQLite pool and applies the production migrations.
pub async fn create_memory_pool() -> Result<SqlitePool, sqlx::Error> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;

    initialize_pool(&pool, false).await?;
    Ok(pool)
}

/// Applies migrations to a newly created pool and enables WAL when requested.
async fn initialize_pool(pool: &SqlitePool, use_wal: bool) -> Result<(), sqlx::Error> {
    MIGRATOR.run(pool).await?;
    if use_wal {
        sqlx::query("PRAGMA journal_mode=WAL").execute(pool).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use sqlx::Row;

    use super::create_pool;

    fn temp_db_path() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        std::env::temp_dir().join(format!("headless-rss-{nonce}.sqlite3"))
    }

    #[tokio::test]
    async fn create_pool_runs_migrations_and_bootstraps_root_folder() {
        let db_path = temp_db_path();
        let pool = create_pool(&db_path.to_string_lossy()).await.unwrap();

        let folder_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='folder'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let feed_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='feed'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let article_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='article'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let email_exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='email_credentials'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(folder_exists, 1);
        assert_eq!(feed_exists, 1);
        assert_eq!(article_exists, 1);
        assert_eq!(email_exists, 1);

        let row = sqlx::query("SELECT name, is_root FROM folder WHERE id = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
        let name: String = row.try_get("name").unwrap();
        let is_root: i64 = row.try_get("is_root").unwrap();
        assert_eq!(name, "");
        assert_eq!(is_root, 1);

        pool.close().await;
        let _ = std::fs::remove_file(db_path);
    }
}
