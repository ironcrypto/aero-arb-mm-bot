//! Bot configuration settings and environment variable handling

use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::env;
use std::str::FromStr;

// Configuration constants
pub const MIN_TRADE_SIZE_ETH: Decimal = dec!(0.01);
pub const MAX_TRADE_SIZE_ETH: Decimal = dec!(10.0);
pub const MIN_PROFIT_USD: Decimal = dec!(0.10);
pub const MAX_SLIPPAGE_BPS: u32 = 100; // 1%
pub const PRICE_STALENESS_SECONDS: u64 = 10;
pub const MAX_PRICE_DEVIATION_PCT: Decimal = dec!(10); // 10% max difference between DEX/CEX

// Market Making Constants
pub const DEFAULT_SPREAD_BPS: u32 = 30; // 0.3% default spread
pub const MIN_SPREAD_BPS: u32 = 10; // 0.1% minimum spread
pub const MAX_SPREAD_BPS: u32 = 200; // 2% maximum spread


// Trade Execution Constants
pub const DEFAULT_GAS_PRICE_GWEI: u32 = 50;
pub const MAX_GAS_PRICE_GWEI: u32 = 200;
pub const EXECUTION_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct Config {
    pub trade_size_eth: Decimal,
    pub min_profit_usd: Decimal,
    pub max_consecutive_errors: u32,
    pub circuit_breaker_cooldown_secs: u64,
    pub enable_safety_checks: bool,
    // Market Making Configuration
    pub enable_market_making: bool,
    pub base_spread_bps: u32,
    pub max_position_size_eth: Decimal,
    pub inventory_target_ratio: Decimal,
    pub rebalance_threshold: Decimal,
    // Trade Execution Configuration
    pub enable_trade_execution: bool,
    pub network: String,
    pub execution_network: String,
    pub max_gas_price_gwei: u32,
    pub slippage_tolerance_bps: u32,
    pub private_key: Option<String>,
    // Volatility Configuration
    pub volatility_threshold: Decimal,
    pub volatility_spread_multiplier: Decimal,
    // Alchemy API Key
    pub alchemy_api_key: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        Self {
            alchemy_api_key: env::var("ALCHEMY_API_KEY").ok(),
            trade_size_eth: env::var("TRADE_SIZE_ETH")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(0.1))
                .max(MIN_TRADE_SIZE_ETH)
                .min(MAX_TRADE_SIZE_ETH),
            min_profit_usd: env::var("MIN_PROFIT_USD")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(0.50))
                .max(MIN_PROFIT_USD),
            max_consecutive_errors: 5,
            circuit_breaker_cooldown_secs: 300, // 5 minutes
            enable_safety_checks: env::var("ENABLE_SAFETY_CHECKS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            // Market Making defaults
            enable_market_making: env::var("ENABLE_MARKET_MAKING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            base_spread_bps: env::var("BASE_SPREAD_BPS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_SPREAD_BPS)
                .max(MIN_SPREAD_BPS)
                .min(MAX_SPREAD_BPS),
            max_position_size_eth: env::var("MAX_POSITION_SIZE_ETH")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(5.0)),
            inventory_target_ratio: env::var("INVENTORY_TARGET_RATIO")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(0.5)),
            rebalance_threshold: env::var("REBALANCE_THRESHOLD")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(0.1)),
            // Trade Execution Configuration
            enable_trade_execution: env::var("ENABLE_TRADE_EXECUTION")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            network: env::var("NETWORK")
                .unwrap_or_else(|_| "mainnet".to_string()),
            execution_network: env::var("EXECUTION_NETWORK")
                .unwrap_or_else(|_| "sepolia".to_string()),                
            max_gas_price_gwei: env::var("MAX_GAS_PRICE_GWEI")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_GAS_PRICE_GWEI)
                .min(MAX_GAS_PRICE_GWEI),
            slippage_tolerance_bps: env::var("SLIPPAGE_TOLERANCE_BPS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50) // 0.5% default
                .min(MAX_SLIPPAGE_BPS),
            private_key: env::var("PRIVATE_KEY").ok(),
            // Volatility Configuration
            volatility_threshold: env::var("VOLATILITY_THRESHOLD")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(5.0)), // 5% threshold
            volatility_spread_multiplier: env::var("VOLATILITY_SPREAD_MULTIPLIER")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(2.0)), // 2x multiplier for high volatility
        }
    }
}
