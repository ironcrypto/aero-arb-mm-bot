//! Arbitrage opportunity calculation

use chrono::Utc;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use crate::types::{ArbitrageOpportunity, ValidationResult};

pub fn calculate_arbitrage(
    pool_name: &str,
    dex_price: Decimal,
    cex_price: Decimal,
    trade_size: Decimal,
) -> Option<ArbitrageOpportunity> {
    let price_diff = dex_price - cex_price;
    let price_diff_pct = (price_diff.abs() / cex_price) * dec!(100);
    
    if price_diff_pct < dec!(0.05) {
        return None;
    }
    
    let direction = if dex_price < cex_price {
        "Buy on Aerodrome → Sell on Binance"
    } else {
        "Buy on Binance → Sell on Aerodrome"
    };
    
    let gross_profit_usd = trade_size * price_diff.abs();
    let gas_cost_usd = dec!(0.02);
    let net_profit_usd = gross_profit_usd - gas_cost_usd;
    let roi_pct = (net_profit_usd / (trade_size * cex_price)) * dec!(100);
    
    Some(ArbitrageOpportunity {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp: Utc::now(),
        pool: pool_name.to_string(),
        direction: direction.to_string(),
        dex_price,
        cex_price,
        price_diff_pct,
        size_eth: trade_size,
        gross_profit_usd,
        gas_cost_usd,
        net_profit_usd,
        roi_pct,
        validation_checks: ValidationResult::default(),
        volatility_assessment: None,
        execution_simulation: None,
    })
}
