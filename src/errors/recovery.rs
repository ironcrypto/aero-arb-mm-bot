//! Error recovery strategies

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::Level;
use super::BotError;

pub struct ErrorRecovery {
    pub error_counts: Arc<RwLock<HashMap<String, u32>>>,
    pub recovery_strategies: HashMap<String, RecoveryStrategy>,
}

#[derive(Clone)]
pub enum RecoveryStrategy {
    Retry { max_attempts: u32, delay_ms: u64 },
    Fallback { alternative_source: String },
    Skip { log_level: Level },
    #[allow(dead_code)]
    Shutdown { reason: String },
}

#[derive(Debug)]
pub enum RecoveryAction {
    Retry { delay: Duration },
    Fallback { source: String },
    Skip { log_level: Level },
    Escalate,
    Shutdown { reason: String },
}

impl ErrorRecovery {
    pub fn new() -> Self {
        let mut strategies = HashMap::new();
        
        strategies.insert(
            "network_timeout".to_string(),
            RecoveryStrategy::Retry {
                max_attempts: 5,
                delay_ms: 1000,
            },
        );
        
        strategies.insert(
            "invalid_price".to_string(),
            RecoveryStrategy::Skip {
                log_level: Level::WARN,
            },
        );
        
        strategies.insert(
            "contract_error".to_string(),
            RecoveryStrategy::Fallback {
                alternative_source: "backup_pool".to_string(),
            },
        );
        
        Self {
            error_counts: Arc::new(RwLock::new(HashMap::new())),
            recovery_strategies: strategies,
        }
    }
    
    pub async fn handle_error(&self, error: &BotError, _context: &str) -> RecoveryAction {
        let error_type = self.classify_error(error);
        let mut counts = self.error_counts.write().await;
        let count = counts.entry(error_type.clone()).or_insert(0);
        *count += 1;
        
        match self.recovery_strategies.get(&error_type) {
            Some(RecoveryStrategy::Retry { max_attempts, delay_ms }) => {
                if *count <= *max_attempts {
                    RecoveryAction::Retry {
                        delay: Duration::from_millis(*delay_ms),
                    }
                } else {
                    RecoveryAction::Escalate
                }
            }
            Some(RecoveryStrategy::Fallback { alternative_source }) => {
                RecoveryAction::Fallback {
                    source: alternative_source.clone(),
                }
            }
            Some(RecoveryStrategy::Skip { log_level }) => {
                RecoveryAction::Skip {
                    log_level: *log_level,
                }
            }
            Some(RecoveryStrategy::Shutdown { reason }) => {
                RecoveryAction::Shutdown {
                    reason: reason.clone(),
                }
            }
            None => RecoveryAction::Escalate,
        }
    }
    
    fn classify_error(&self, error: &BotError) -> String {
        match error {
            BotError::Network { .. } => "network_timeout".to_string(),
            BotError::PriceValidation { .. } => "invalid_price".to_string(),
            BotError::Contract { .. } => "contract_error".to_string(),
            BotError::InsufficientLiquidity { .. } => "low_liquidity".to_string(),
            BotError::DataParsing { .. } => "parse_error".to_string(),
            BotError::CircuitBreakerOpen { .. } => "circuit_breaker".to_string(),
        }
    }
}
