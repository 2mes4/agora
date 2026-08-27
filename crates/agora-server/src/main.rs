//! AGORA gateway node — binary entry point.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use agora_server::{echo_card, EchoAgent, Gateway, ServerConfig};
use anyhow::Context;
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

/// Command-line interface for the gateway node.
#[derive(Debug, Parser)]
#[command(
    name = "agora-server",
    version,
    about = "AGORA gateway node: hosts agents, serves the directory, routes A2A"
)]
struct Cli {
    /// Bind address.
    #[arg(long, env = "AGORA_BIND", default_value = "0.0.0.0:7100")]
    bind: String,

    /// Host the built-in demo echo agent.
    #[arg(long, env = "AGORA_DEMO_AGENT")]
    demo_agent: bool,

    /// Public base URL advertised in hosted agents' cards.
    #[arg(long, env = "AGORA_ADVERTISE")]
    advertise: Option<String>,

    /// PostgreSQL connection URL for the persistence backend
    /// (postgres://user:password@host:5432/dbname). When unset, all storage
    /// is in-memory.
    #[arg(long, env = "AGORA_DATABASE_URL")]
    database_url: Option<String>,

    /// NATS message bus URL (e.g. nats://127.0.0.1:4222). When unset,
    /// in-process bus is used.
    #[arg(long, env = "AGORA_NATS_URL")]
    nats_url: Option<String>,

    /// Optional API key for authenticating incoming requests.
    #[arg(long, env = "AGORA_API_KEY")]
    api_key: Option<String>,

    /// Path to a TOML configuration file (see config/server.example.toml).
    #[arg(long)]
    config: Option<PathBuf>,

    /// Log format: `text` or `json`.
    #[arg(long, env = "AGORA_LOG_FORMAT", default_value = "text")]
    log_format: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let file_config = load_config(cli.config.as_deref())?;
    let effective = resolve(cli, file_config);

    init_logging(&effective.log_format);

    let backend = match effective.database_url.as_deref() {
        Some(url) => {
            let store = agora_store::PostgresStore::connect(url).await?;
            info!("connected to PostgreSQL persistence backend");
            agora_store::StoreBackend::postgres(std::sync::Arc::new(store))
        }
        None => {
            info!("no database configured; using in-memory storage");
            agora_store::StoreBackend::memory()
        }
    };

    let bus: Arc<dyn agora_bus::MessageBus> = match effective.nats_url.as_deref() {
        Some(url) => {
            let nats = agora_bus_nats::NatsBus::connect(url).await?;
            info!(url, "connected to NATS message bus");
            Arc::new(nats)
        }
        None => Arc::new(agora_bus::InProcessBus::new()),
    };

    let gateway = Gateway::with_backend(bus, backend);

    let port = effective
        .bind
        .rsplit_once(':')
        .map(|(_, port)| port.to_string())
        .unwrap_or_else(|| "7100".to_string());
    let base = effective
        .advertise
        .clone()
        .unwrap_or_else(|| format!("http://127.0.0.1:{port}"));

    if effective.demo_agent {
        gateway.mount(echo_card(&base), Arc::new(EchoAgent)).await;
        info!(url = %format!("{base}/a2a/echo"), "demo echo agent hosted");
    }

    let addr: SocketAddr = effective
        .bind
        .parse()
        .with_context(|| format!("invalid bind address: {}", effective.bind))?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "AGORA gateway listening");

    axum::serve(listener, gateway.router())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Effective configuration: CLI overrides file overrides defaults.
fn resolve(cli: Cli, file: ServerConfig) -> EffectiveConfig {
    EffectiveConfig {
        bind: cli.bind,
        demo_agent: cli.demo_agent || file.demo_agent.unwrap_or(false),
        advertise: cli.advertise.or(file.advertise),
        database_url: cli.database_url.or(file.database_url),
        nats_url: cli.nats_url.or(file.nats_url),
        api_key: cli.api_key.or(file.api_key),
        log_format: cli.log_format,
    }
}

struct EffectiveConfig {
    bind: String,
    demo_agent: bool,
    advertise: Option<String>,
    database_url: Option<String>,
    nats_url: Option<String>,
    #[allow(dead_code)]
    api_key: Option<String>,
    log_format: String,
}

fn load_config(path: Option<&std::path::Path>) -> anyhow::Result<ServerConfig> {
    match path {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("cannot read config: {}", path.display()))?;
            let config: ServerConfig = toml::from_str(&raw)
                .with_context(|| format!("invalid config: {}", path.display()))?;
            info!(path = %path.display(), "configuration loaded");
            Ok(config)
        }
        None => Ok(ServerConfig::default()),
    }
}

fn init_logging(format: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    match format {
        "json" => builder.json().init(),
        "text" => builder.init(),
        other => {
            warn!(format = other, "unrecognized log format, using default");
            builder.init();
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("shutting down");
}
