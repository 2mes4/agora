//! Retry and fallback policy configuration for task execution.
//!
//! Controls retry attempts, exponential backoff with jitter limits, and
//! fallback target agents when execution fails.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Policy controlling task retry behavior and fallback agents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    /// Maximum number of retry attempts (0 means no retries).
    pub max_retries: u32,
    /// Initial backoff delay in milliseconds.
    pub initial_backoff_ms: u64,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: f64,
    /// Maximum backoff delay in milliseconds.
    pub max_backoff_ms: u64,
    /// Ordered list of fallback agents to attempt if all retries fail.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallback_agents: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff_ms: 200,
            backoff_multiplier: 2.0,
            max_backoff_ms: 5000,
            fallback_agents: Vec::new(),
        }
    }
}

impl RetryPolicy {
    /// Create a policy with no retries.
    pub fn none() -> Self {
        Self {
            max_retries: 0,
            initial_backoff_ms: 0,
            backoff_multiplier: 1.0,
            max_backoff_ms: 0,
            fallback_agents: Vec::new(),
        }
    }

    /// Builder for custom retries.
    pub fn new(max_retries: u32, initial_backoff_ms: u64) -> Self {
        Self {
            max_retries,
            initial_backoff_ms,
            ..Self::default()
        }
    }

    /// Add a fallback agent.
    pub fn with_fallback(mut self, agent: impl Into<String>) -> Self {
        self.fallback_agents.push(agent.into());
        self
    }

    /// Add multiple fallback agents.
    pub fn with_fallbacks(mut self, agents: Vec<String>) -> Self {
        self.fallback_agents = agents;
        self
    }

    /// Compute backoff duration for a given retry attempt (1-based index).
    pub fn compute_backoff(&self, attempt: u32) -> Duration {
        if attempt == 0 || self.max_retries == 0 {
            return Duration::from_millis(0);
        }
        let mult = self.backoff_multiplier.powi((attempt - 1) as i32);
        let delay_ms = (self.initial_backoff_ms as f64 * mult) as u64;
        let clamped_ms = delay_ms.min(self.max_backoff_ms);
        Duration::from_millis(clamped_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_backoff_exponential() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_backoff_ms: 100,
            backoff_multiplier: 2.0,
            max_backoff_ms: 1000,
            fallback_agents: vec!["fallback-agent".into()],
        };

        assert_eq!(policy.compute_backoff(1), Duration::from_millis(100));
        assert_eq!(policy.compute_backoff(2), Duration::from_millis(200));
        assert_eq!(policy.compute_backoff(3), Duration::from_millis(400));
        assert_eq!(policy.compute_backoff(10), Duration::from_millis(1000));
    }
}
