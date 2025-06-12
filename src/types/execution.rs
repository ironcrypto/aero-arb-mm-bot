//! Trade execution types

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TradeExecution {
    pub id: String,
    pub opportunity_id: String,
    pub timestamp: DateTime<Utc>,
    pub network: String,
    pub trade_type: TradeType,
    pub status: ExecutionStatus,
    pub tx_hash: Option<String>,
    pub gas_used: Option<u64>,
    pub gas_price_gwei: Option<Decimal>,
    pub execution_time_ms: u64,
    pub expected_profit_usd: Decimal,
    pub actual_profit_usd: Option<Decimal>,
    pub slippage_bps: Option<u32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub enum TradeType {
    BuyDexSellCex,
    BuyCexSellDex,
}

#[derive(Debug, Clone, Serialize)]
pub enum ExecutionStatus {
    Simulated,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub enum ExecutionUrgency {
    Fast,
    Normal,
    Cautious,
}
