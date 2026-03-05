use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

pub async fn create_pool(db_path: &str) -> Result<SqlitePool, sqlx::Error> {
    let db_url = format!("sqlite://{db_path}");
    let options = SqliteConnectOptions::from_str(&db_url)?.create_if_missing(true);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
}
