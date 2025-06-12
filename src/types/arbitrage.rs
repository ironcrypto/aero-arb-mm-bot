//! Arbitrage opportunity types

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use super::{ValidationResult, VolatilityMetrics, TradeExecution};

#[derive(Debug, Clone, Serialize)]
pub struct ArbitrageOpportunity {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub pool: String,
    pub direction: String,
    pub dex_price: Decimal,
    pub cex_price: Decimal,
    pub price_diff_pct: Decimal,
    pub size_eth: Decimal,
    pub gross_profit_usd: Decimal,
    pub gas_cost_usd: Decimal,
    pub net_profit_usd: Decimal,
    pub roi_pct: Decimal,
    pub validation_checks: ValidationResult,
    pub volatility_assessment: Option<VolatilityMetrics>,
    pub execution_simulation: Option<TradeExecution>,
}
