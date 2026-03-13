use anyhow::{Context, Result};
use native_tls::TlsConnector;
use sqlx::SqlitePool;
use std::env;

use crate::config::Config;
use crate::db;

/// Validates and persists IMAP credentials for later newsletter polling.
pub async fn add_email_credentials(
    config: &Config,
    server: String,
    port: u16,
    username: String,
    password: String,
) -> Result<()> {
    let pool = db::create_pool(&config.db_path)
        .await
        .with_context(|| format!("failed to connect to sqlite db at {}", config.db_path))?;

    add_email_credentials_with_validator(
        &pool,
        "imap",
        &server,
        port,
        &username,
        &password,
        validate_imap_credentials,
    )
    .await?;

    println!("Email credentials added successfully.");
    Ok(())
}

/// Runs a credential validator and persists the mailbox configuration only on success.
async fn add_email_credentials_with_validator<F>(
    pool: &SqlitePool,
    protocol: &str,
    server: &str,
    port: u16,
    username: &str,
    password: &str,
    validator: F,
) -> Result<()>
where
    F: Fn(&str, u16, &str, &str) -> Result<()>,
{
    validator(server, port, username, password).with_context(|| {
        format!("Failed to connect to mailbox at {server}:{port} for user {username}")
    })?;

    sqlx::query(
        "INSERT INTO email_credentials (protocol, server, port, username, password) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(protocol)
    .bind(server)
    .bind(i64::from(port))
    .bind(username)
    .bind(password)
    .execute(pool)
    .await
    .context("failed to persist email credentials")?;

    Ok(())
}

/// Verifies that the supplied IMAP settings can connect, authenticate, and open the inbox.
fn validate_imap_credentials(
    server: &str,
    port: u16,
    username: &str,
    password: &str,
) -> Result<()> {
    if mock_imap_enabled() {
        return Ok(());
    }

    let tls = TlsConnector::builder()
        .build()
        .context("failed to initialize tls connector")?;

    let client = imap::connect((server, port), server, &tls)
        .with_context(|| format!("failed to connect to mailbox at {server}:{port}"))?;

    let mut session = client
        .login(username, password)
        .map_err(|(error, _client)| error)
        .with_context(|| format!("failed to login to mailbox at {server}:{port}"))?;

    session.select("inbox").context("failed to select inbox")?;
    session.logout().context("failed to logout from mailbox")?;
    Ok(())
}

/// Returns true when test-mode mailbox validation should bypass the real IMAP round trip.
fn mock_imap_enabled() -> bool {
    env::var("TESTING_MODE")
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false)
        && env::var("HEADLESS_RSS_TEST_IMAP_ALLOW")
            .ok()
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use super::add_email_credentials_with_validator;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE email_credentials (id INTEGER PRIMARY KEY AUTOINCREMENT, protocol VARCHAR NOT NULL, server VARCHAR NOT NULL, port INTEGER NOT NULL, username VARCHAR NOT NULL, password VARCHAR NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn persists_credentials_when_validation_succeeds() {
        let pool = setup_pool().await;

        add_email_credentials_with_validator(
            &pool,
            "imap",
            "imap.example.com",
            993,
            "user@example.com",
            "secret",
            |_server, _port, _username, _password| Ok(()),
        )
        .await
        .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_credentials")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn does_not_persist_when_validation_fails() {
        let pool = setup_pool().await;

        let result = add_email_credentials_with_validator(
            &pool,
            "imap",
            "imap.example.com",
            993,
            "user@example.com",
            "secret",
            |_server, _port, _username, _password| anyhow::bail!("invalid credentials"),
        )
        .await;

        assert!(result.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM email_credentials")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
