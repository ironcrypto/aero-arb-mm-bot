//! Pool-related types and structures

use alloy::primitives::Address;
use rust_decimal::Decimal;
use std::time::Instant;

#[derive(Clone)]
pub struct PoolInfo {
    pub address: Address,
    pub name: String,
    pub token0: Address,
    pub token1: Address,
    #[allow(dead_code)]
    pub is_stable: bool,
    #[allow(dead_code)]
    pub min_liquidity: Decimal,
    #[allow(dead_code)]
    pub last_update: Instant,
}
