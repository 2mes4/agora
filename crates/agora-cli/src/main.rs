//! Operator CLI for AGORA.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agora", author, version, about = "AGORA operator CLI")]
struct Cli {
    /// Gateway URL
    #[arg(
        long,
        env = "AGORA_GATEWAY_URL",
        default_value = "http://127.0.0.1:7100"
    )]
    gateway: String,

    /// Optional API key for authentication
    #[arg(long, env = "AGORA_API_KEY")]
    api_key: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List registered agents in the directory
    List,
    /// Register an agent card manifest JSON file
    Register {
        /// Path to the agent card JSON file
        file: String,
    },
    /// Send a message to an agent
    Send {
        /// Target agent name or URL
        target: String,
        /// Message text
        message: String,
        /// Optional skill identifier
        #[arg(long)]
        skill: Option<String>,
        /// Stream the response
        #[arg(long)]
        stream: bool,
    },
    /// Manage the dead-letter queue (DDLQ)
    DeadLetters {
        #[command(subcommand)]
        action: DeadLetterAction,
    },
    /// Manage cryptographic keys (M5)
    Keys {
        #[command(subcommand)]
        action: KeysAction,
    },
    /// Send a heartbeat for an agent to update presence
    Heartbeat {
        /// Agent name
        name: String,
        /// Optional status: online, busy, offline
        #[arg(long)]
        status: Option<String>,
    },
    /// Discover and search marketplace services
    Services {
        #[command(subcommand)]
        action: ServicesAction,
    },
}

#[derive(Subcommand)]
enum ServicesAction {
    /// List all marketplace services
    List {
        #[arg(long)]
        online_only: bool,
    },
    /// Find agents offering a specific service ID
    Get {
        id: String,
        #[arg(long)]
        online_only: bool,
    },
    /// Search services via Llull Search Engine bridge
    Search {
        query: String,
        #[arg(long)]
        online_only: bool,
        #[arg(long)]
        max_price: Option<f64>,
        #[arg(long)]
        currency: Option<String>,
    },
}

#[derive(Subcommand)]
enum KeysAction {
    /// Generate a new Ed25519 (signing) and X25519 (encryption) keypair
    Generate,
}

#[derive(Subcommand)]
enum DeadLetterAction {
    /// List dead-letter entries
    List {
        #[arg(long, default_value = "50")]
        limit: usize,
    },
    /// Inspect a single dead-letter entry
    Get { id: String },
    /// Replay a dead-letter entry
    Replay { id: String },
    /// Delete a dead-letter entry
    Delete { id: String },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::List => {
            let client = reqwest::Client::new();
            let mut req = client.get(format!("{}/v1/agents", cli.gateway));
            if let Some(key) = &cli.api_key {
                req = req.bearer_auth(key);
            }
            let res = req.send().await?;
            let body: serde_json::Value = res.json().await?;
            println!("{}", serde_json::to_string_pretty(&body)?);
        }
        Commands::Register { file } => {
            let content = tokio::fs::read_to_string(&file).await?;
            let card: agora_core::a2a::AgentCard = serde_json::from_str(&content)?;
            let client = reqwest::Client::new();
            let mut req = client
                .post(format!("{}/v1/agents", cli.gateway))
                .json(&card);
            if let Some(key) = &cli.api_key {
                req = req.bearer_auth(key);
            }
            let res = req.send().await?;
            if res.status().is_success() {
                println!("Agent {} registered successfully", card.name);
            } else {
                eprintln!("Failed to register agent: {}", res.text().await?);
            }
        }
        Commands::Send {
            target,
            message,
            skill,
            stream: _,
        } => {
            let mut client = agora_sdk::AgoraClient::new(&target)?;
            if let Some(key) = &cli.api_key {
                client = client.with_api_key(key);
            }
            let mut builder = client.delegate().text(message);
            if let Some(s) = skill {
                builder = builder.skill(s);
            }
            let task = builder.send().await?;
            println!("{}", serde_json::to_string_pretty(&task)?);
        }
        Commands::DeadLetters { action } => {
            let client = reqwest::Client::new();
            match action {
                DeadLetterAction::List { limit } => {
                    let mut req =
                        client.get(format!("{}/v1/dead-letters?limit={limit}", cli.gateway));
                    if let Some(key) = &cli.api_key {
                        req = req.bearer_auth(key);
                    }
                    let res = req.send().await?;
                    let body: serde_json::Value = res.json().await?;
                    println!("{}", serde_json::to_string_pretty(&body)?);
                }
                DeadLetterAction::Get { id } => {
                    let mut req = client.get(format!("{}/v1/dead-letters/{id}", cli.gateway));
                    if let Some(key) = &cli.api_key {
                        req = req.bearer_auth(key);
                    }
                    let res = req.send().await?;
                    let body: serde_json::Value = res.json().await?;
                    println!("{}", serde_json::to_string_pretty(&body)?);
                }
                DeadLetterAction::Replay { id } => {
                    let mut req =
                        client.post(format!("{}/v1/dead-letters/{id}/replay", cli.gateway));
                    if let Some(key) = &cli.api_key {
                        req = req.bearer_auth(key);
                    }
                    let res = req.send().await?;
                    println!("Replay response: {}", res.status());
                    let body: serde_json::Value = res.json().await?;
                    println!("{}", serde_json::to_string_pretty(&body)?);
                }
                DeadLetterAction::Delete { id } => {
                    let mut req = client.delete(format!("{}/v1/dead-letters/{id}", cli.gateway));
                    if let Some(key) = &cli.api_key {
                        req = req.bearer_auth(key);
                    }
                    let res = req.send().await?;
                    if res.status().is_success() {
                        println!("Dead letter {id} deleted");
                    } else {
                        eprintln!("Failed to delete: {}", res.text().await?);
                    }
                }
            }
        }
        Commands::Keys { action } => match action {
            KeysAction::Generate => {
                let keypair = agora_core::AgentKeypair::generate();
                let output = serde_json::json!({
                    "signingPublicKey": keypair.verifying_key().to_hex(),
                    "signingPrivateKey": keypair.signing_key.to_hex(),
                    "encryptionPublicKey": keypair.encryption_public_key().to_hex(),
                    "encryptionPrivateKey": keypair.encryption_secret.to_hex(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
        },
        Commands::Heartbeat { name, status } => {
            let client = reqwest::Client::new();
            let mut req = client.post(format!("{}/v1/agents/{name}/heartbeat", cli.gateway));
            if let Some(key) = &cli.api_key {
                req = req.bearer_auth(key);
            }
            if let Some(s) = status {
                req = req.json(&serde_json::json!({ "status": s }));
            }
            let res = req.send().await?;
            let body: serde_json::Value = res.json().await?;
            println!("{}", serde_json::to_string_pretty(&body)?);
        }
        Commands::Services { action } => {
            let client = reqwest::Client::new();
            match action {
                ServicesAction::List { online_only } => {
                    let mut req = client.get(format!(
                        "{}/v1/services?online_only={online_only}",
                        cli.gateway
                    ));
                    if let Some(key) = &cli.api_key {
                        req = req.bearer_auth(key);
                    }
                    let res = req.send().await?;
                    let body: serde_json::Value = res.json().await?;
                    println!("{}", serde_json::to_string_pretty(&body)?);
                }
                ServicesAction::Get { id, online_only } => {
                    let mut req = client.get(format!(
                        "{}/v1/services/{id}?online_only={online_only}",
                        cli.gateway
                    ));
                    if let Some(key) = &cli.api_key {
                        req = req.bearer_auth(key);
                    }
                    let res = req.send().await?;
                    let body: serde_json::Value = res.json().await?;
                    println!("{}", serde_json::to_string_pretty(&body)?);
                }
                ServicesAction::Search {
                    query,
                    online_only,
                    max_price,
                    currency,
                } => {
                    let mut query_params: Vec<(&str, String)> =
                        vec![("q", query), ("online_only", online_only.to_string())];
                    if let Some(max_p) = max_price {
                        query_params.push(("max_price", max_p.to_string()));
                    }
                    if let Some(curr) = currency {
                        query_params.push(("currency", curr));
                    }
                    let mut req = client
                        .get(format!("{}/v1/services/search", cli.gateway))
                        .query(&query_params);
                    if let Some(key) = &cli.api_key {
                        req = req.bearer_auth(key);
                    }
                    let res = req.send().await?;
                    let body: serde_json::Value = res.json().await?;
                    println!("{}", serde_json::to_string_pretty(&body)?);
                }
            }
        }
    }

    Ok(())
}
