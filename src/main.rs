mod api;
mod article_store;
mod config;
mod content;
mod db;
mod email;
mod email_credentials;
mod http_client;
mod llm;
mod repo;
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
#[command(name = "headless-rss")]
#[command(about = "Self-hosted RSS and newsletter aggregator")]
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
    ReevaluateFeedQuality {
        #[arg(long)]
        feed_id: i64,
    },
    SetFeedQuality {
        #[arg(long)]
        feed_id: i64,
        #[arg(long)]
        use_extracted_fulltext: Option<bool>,
        #[arg(long)]
        use_llm_summary: Option<bool>,
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
            tracing::debug!("cli command invoked: serve");
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
            tracing::debug!("cli command invoked: add-email-credentials");
            email_credentials::add_email_credentials(&config, server, port, username, password)
                .await
        }
        Commands::ReevaluateFeedQuality { feed_id } => {
            tracing::debug!(feed_id, "cli command invoked: reevaluate-feed-quality");
            let result = updater::reevaluate_feed_quality(&config, feed_id).await?;
            println!("Feed quality re-evaluation completed.");
            println!("Feed ID: {}", result.feed_id);
            println!(
                "Feed title: {}",
                result.feed_title.as_deref().unwrap_or("(untitled)")
            );
            println!(
                "Use extracted full text: {}",
                bool_to_enabled_disabled(result.use_extracted_fulltext)
            );
            println!(
                "Use LLM summary: {}",
                bool_to_enabled_disabled(result.use_llm_summary)
            );
            println!(
                "Manual extracted full text override: {}",
                optional_bool_to_manual_state(result.manual_use_extracted_fulltext)
            );
            println!(
                "Manual LLM summary override: {}",
                optional_bool_to_manual_state(result.manual_use_llm_summary)
            );
            println!(
                "Last quality check: {}",
                result
                    .last_quality_check
                    .map(|timestamp| format!("{timestamp} (unix seconds)"))
                    .unwrap_or_else(|| "not updated".to_string())
            );
            println!("Last manual override: not set");
            Ok(())
        }
        Commands::SetFeedQuality {
            feed_id,
            use_extracted_fulltext,
            use_llm_summary,
        } => {
            tracing::debug!(feed_id, "cli command invoked: set-feed-quality");
            let result = updater::set_feed_quality_overrides(
                &config,
                feed_id,
                use_extracted_fulltext,
                use_llm_summary,
            )
            .await?;
            println!("Feed quality overrides updated.");
            println!("Feed ID: {}", result.feed_id);
            println!(
                "Feed title: {}",
                result.feed_title.as_deref().unwrap_or("(untitled)")
            );
            println!(
                "Use extracted full text: {}",
                bool_to_enabled_disabled(result.use_extracted_fulltext)
            );
            println!(
                "Use LLM summary: {}",
                bool_to_enabled_disabled(result.use_llm_summary)
            );
            println!(
                "Manual extracted full text override: {}",
                optional_bool_to_manual_state(result.manual_use_extracted_fulltext)
            );
            println!(
                "Manual LLM summary override: {}",
                optional_bool_to_manual_state(result.manual_use_llm_summary)
            );
            println!(
                "Last quality check: {}",
                result
                    .last_quality_check
                    .map(|timestamp| format!("{timestamp} (unix seconds)"))
                    .unwrap_or_else(|| "not updated".to_string())
            );
            println!(
                "Last manual override: {}",
                result
                    .last_manual_quality_override
                    .map(|timestamp| format!("{timestamp} (unix seconds)"))
                    .unwrap_or_else(|| "not set".to_string())
            );
            Ok(())
        }
    }
}

fn bool_to_enabled_disabled(value: bool) -> &'static str {
    if value { "enabled" } else { "disabled" }
}

fn optional_bool_to_manual_state(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "enabled",
        Some(false) => "disabled",
        None => "automatic",
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
            let _ = err;
            tracing::warn!("startup forced feed update cycle failed");
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
                let _ = err;
                tracing::warn!("scheduled feed update cycle failed");
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

    tracing::info!("starting rust api server");

    axum::serve(listener, app)
        .await
        .context("api server failed")?;
    Ok(())
}
