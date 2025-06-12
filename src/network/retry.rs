//! Retry logic with exponential backoff

use std::time::Duration;
use anyhow::Result;
use tracing::warn;
use crate::errors::{BotError, BotResult};

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub exponential_base: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            exponential_base: 2.0,
        }
    }
}

pub async fn retry_with_backoff<F, Fut, T>(
    operation: F,
    config: &RetryConfig,
    context: &str,
) -> BotResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0;
    let mut delay = config.initial_delay_ms;
    
    loop {
        attempt += 1;
        
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt >= config.max_attempts => {
                return Err(BotError::Network {
                    message: format!("{} failed after {} attempts", context, attempt),
                    source: Some(e),
                    retry_count: attempt,
                });
            }
            Err(e) => {
                warn!(
                    "Attempt {}/{} failed for {}: {}. Retrying in {}ms...",
                    attempt, config.max_attempts, context, e, delay
                );
                
                tokio::time::sleep(Duration::from_millis(delay)).await;
                
                delay = (delay as f64 * config.exponential_base) as u64;
                delay = delay.min(config.max_delay_ms);
                let jitter = (delay as f64 * 0.1 * (rand::random::<f64>() - 0.5)) as u64;
                delay = delay.saturating_add(jitter);
            }
        }
    }
}
