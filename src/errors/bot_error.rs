//! Custom error types for the bot

use alloy::primitives::Address;
use rust_decimal::Decimal;
use std::error::Error;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BotError {
    #[error("Network error: {message}")]
    Network {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
        retry_count: u32,
    },
    
    #[error("Contract interaction failed: {contract} - {message}")]
    Contract {
        contract: Address,
        message: String,
        #[source]
        source: anyhow::Error,
    },
    
    #[error("Price validation failed: {source} price ${price} is invalid - {reason}")]
    PriceValidation {
        source: Box<dyn Error + Send + Sync>,
        price: Decimal,
        reason: String,
    },
    
    #[error("Insufficient liquidity: {pool} - {details}")]
    InsufficientLiquidity {
        pool: String,
        details: String,
    },
    
    #[error("Data parsing error: {context}")]
    DataParsing {
        context: String,
        #[source]
        source: anyhow::Error,
    },
    
    #[error("Circuit breaker active: {reason}")]
    CircuitBreakerOpen {
        reason: String,
        cooldown_remaining: Duration,
    },
}

pub type BotResult<T> = Result<T, BotError>;

// implement a safe send and sync trait for BotError to allow for cross-thread communication
unsafe impl Send for BotError {}
unsafe impl Sync for BotError {}