//! Circuit breaker implementation

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info};
use crate::config::CONFIG;

pub struct CircuitBreaker {
    pub consecutive_errors: Arc<RwLock<u32>>,
    pub is_open: Arc<RwLock<bool>>,
    pub last_error_time: Arc<RwLock<Option<Instant>>>,
    pub cooldown_duration: Duration,
}

impl CircuitBreaker {
    pub fn new(cooldown_secs: u64) -> Self {
        Self {
            consecutive_errors: Arc::new(RwLock::new(0)),
            is_open: Arc::new(RwLock::new(false)),
            last_error_time: Arc::new(RwLock::new(None)),
            cooldown_duration: Duration::from_secs(cooldown_secs),
        }
    }

    pub async fn record_success(&self) {
        *self.consecutive_errors.write().await = 0;
        *self.is_open.write().await = false;
    }

    pub async fn record_error(&self) -> bool {
        let mut errors = self.consecutive_errors.write().await;
        *errors += 1;
        
        if *errors >= CONFIG.max_consecutive_errors {
            *self.is_open.write().await = true;
            *self.last_error_time.write().await = Some(Instant::now());
            error!("Circuit breaker OPEN after {} consecutive errors", *errors);
            return true;
        }
        false
    }

    pub async fn can_proceed(&self) -> bool {
        let is_open = *self.is_open.read().await;
        if !is_open {
            return true;
        }

        if let Some(last_error) = *self.last_error_time.read().await {
            if last_error.elapsed() > self.cooldown_duration {
                info!("Circuit breaker cooldown complete, resetting");
                *self.is_open.write().await = false;
                *self.consecutive_errors.write().await = 0;
                return true;
            }
        }
        false
    }
}
