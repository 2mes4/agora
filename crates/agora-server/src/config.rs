//! Server configuration (TOML file + CLI overrides).

use serde::Deserialize;

/// Configuration loaded from a TOML file (`--config`). Every field is
/// optional; CLI flags override file values.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerConfig {
    /// Bind address, e.g. `0.0.0.0:7100`.
    pub bind: Option<String>,
    /// Host the built-in demo echo agent.
    pub demo_agent: Option<bool>,
    /// Public base URL advertised in hosted agents' cards.
    pub advertise: Option<String>,
    /// Log format: `text` or `json`.
    pub log_format: Option<String>,
}
