//! Aerodrome Arbitrage Bot - Production-ready trading bot for Base network
//! 
//! This bot monitors Aerodrome DEX pools for arbitrage opportunities against
//! centralized exchanges, implements market making strategies, and can execute
//! trades on testnet for simulation.

pub mod config;
pub mod types;
pub mod errors;
pub mod network;
pub mod pools;
pub mod arbitrage;
pub mod market_making;
pub mod execution;
pub mod volatility;
pub mod validation;
pub mod utils;
pub mod storage;

// Re-export commonly used items
pub use config::{Config, CONFIG};
pub use errors::{BotError, BotResult};
pub use types::*;

// Type alias for our concrete provider
pub type ConcreteProvider = alloy::providers::RootProvider<alloy::transports::BoxTransport>;
