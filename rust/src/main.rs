mod api;
mod config;
mod content;
mod db;
mod email_credentials;
mod http_client;
mod ssrf;
mod updater;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
use tokio::time::{Duration, sleep};
use tracing_subscriber::EnvFilter;

use api::AppState;
use config::Config;

#[derive(Parser)]
#[command(name = "headless-rss-rs")]
#[command(about = "Rust reimplementation of headless-rss")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Serve {
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long, default_value_t = 8000)]
        port: u16,
    },
    Update,
    AddEmailCredentials {
        #[arg(long)]
        server: String,
        #[arg(long)]
        port: u16,
        #[arg(long)]
        username: String,
        #[arg(long)]
        password: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let config = Arc::new(Config::from_env());

    match cli.command.unwrap_or(Commands::Serve {
        host: "0.0.0.0".to_string(),
        port: 8000,
    }) {
        Commands::Serve { host, port } => {
            tracing::debug!(host = %host, port, "cli command invoked: serve");
            serve(config, host, port).await
        }
        Commands::Update => {
            tracing::debug!("cli command invoked: update");
            updater::update_all(&config).await
        }
        Commands::AddEmailCredentials {
            server,
            port,
            username,
            password,
        } => {
            tracing::debug!(
                server = %server,
                port,
                username = %username,
                "cli command invoked: add-email-credentials"
            );
            email_credentials::add_email_credentials(&config, server, port, username, password)
                .await
        }
    }
}

fn init_tracing() {
    let default_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(default_filter)
        .init();
}

async fn serve(config: Arc<Config>, host: String, port: u16) -> anyhow::Result<()> {
    let pool = db::create_pool(&config.db_path)
        .await
        .with_context(|| format!("failed to connect to sqlite db at {}", config.db_path))?;
    let feed_http_client = http_client::build_feed_http_client()?;
    let article_http_client = http_client::build_article_http_client()?;

    let scheduler_pool = pool.clone();
    let scheduler_config = config.clone();
    let scheduler_testing_mode = config.testing_mode;
    let scheduler_interval = Duration::from_secs((config.feed_update_frequency_min as u64) * 60);
    tokio::spawn(async move {
        if let Err(err) = updater::update_all_regular_feeds(
            &scheduler_pool,
            &scheduler_config,
            scheduler_testing_mode,
        )
        .await
        {
            tracing::warn!(error = %err, "startup forced feed update cycle failed");
        }

        loop {
            sleep(scheduler_interval).await;
            if let Err(err) = updater::update_due_feeds(
                &scheduler_pool,
                &scheduler_config,
                scheduler_testing_mode,
            )
            .await
            {
                tracing::warn!(error = %err, "scheduled feed update cycle failed");
            }
        }
    });

    let state = AppState {
        pool,
        config: config.clone(),
        feed_http_client,
        article_http_client,
    };

    let app = api::app(state);
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .context("invalid host/port")?;
    let listener = TcpListener::bind(addr)
        .await
        .context("failed to bind tcp listener")?;

    tracing::info!(%addr, "starting rust api server");

    axum::serve(listener, app)
        .await
        .context("api server failed")?;
    Ok(())
}
