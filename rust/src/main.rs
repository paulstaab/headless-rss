mod api;
mod config;
mod db;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use tokio::net::TcpListener;
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
        Commands::Serve { host, port } => serve(config, host, port).await,
        Commands::Update => {
            tracing::warn!("update command is not implemented yet in rust");
            Ok(())
        }
        Commands::AddEmailCredentials {
            server,
            port,
            username,
            ..
        } => {
            tracing::warn!(
                server,
                port,
                username,
                "add-email-credentials is not implemented yet in rust"
            );
            Ok(())
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

    let state = AppState {
        pool,
        config: config.clone(),
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
