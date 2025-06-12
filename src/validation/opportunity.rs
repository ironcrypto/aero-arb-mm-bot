//! Opportunity validation with volatility assessment

use alloy::providers::Provider;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::str::FromStr;
use crate::{
    config::{CONFIG, MAX_PRICE_DEVIATION_PCT, MAX_SLIPPAGE_BPS},
    pools::get_pool_reserves_enhanced,
    types::{
        ArbitrageOpportunity, PoolInfo, ValidationResult, VolatilityMetrics, VolatilityImpact,
        WETH_MAINNET
    },
    utils::pow10,
    validation::validate_liquidity,
};

pub async fn validate_opportunity_with_volatility(
    opp: &ArbitrageOpportunity,
    pool_info: &PoolInfo,
    provider: &dyn Provider,
    volatility_metrics: &VolatilityMetrics,
) -> ValidationResult {
    let mut result = ValidationResult::default();
    let mut all_good = true;

    // Price sanity check
    result.price_sanity = opp.price_diff_pct < MAX_PRICE_DEVIATION_PCT;
    if !result.price_sanity {
        result.warnings.push(format!(
            "Price deviation too high: {:.2}% (max: {}%)", 
            opp.price_diff_pct, MAX_PRICE_DEVIATION_PCT
        ));
        all_good = false;
    }

    // Volatility check
    result.volatility_acceptable = volatility_metrics.short_term_volatility < CONFIG.volatility_threshold;
    if !result.volatility_acceptable {
        result.warnings.push(format!(
            "Volatility too high: {:.2}% (threshold: {:.2}%)",
            volatility_metrics.short_term_volatility,
            CONFIG.volatility_threshold
        ));
        // Don't fail entirely on high volatility, just warn
    }

    // Liquidity check
    match get_pool_reserves_enhanced(provider, pool_info.address, &pool_info.name).await {
        Ok((r0, r1)) => {
            let (weth_reserve, usd_reserve) = if pool_info.token0 == WETH_MAINNET {
                (
                    Decimal::from_str(&r0.to_string()).unwrap_or_default() / pow10(18),
                    Decimal::from_str(&r1.to_string()).unwrap_or_default() / pow10(6)
                )
            } else {
                (
                    Decimal::from_str(&r1.to_string()).unwrap_or_default() / pow10(18),
                    Decimal::from_str(&r0.to_string()).unwrap_or_default() / pow10(6)
                )
            };

            result.liquidity_check = validate_liquidity(weth_reserve, usd_reserve).is_ok();
            if !result.liquidity_check {
                result.warnings.push(format!(
                    "Low liquidity: {:.4} WETH, ${:.2} USD", 
                    weth_reserve, usd_reserve
                ));
                all_good = false;
            }

            let trade_impact_pct = (opp.size_eth / weth_reserve) * dec!(100);
            if trade_impact_pct > dec!(1) {
                result.warnings.push(format!(
                    "Trade size is {:.2}% of pool liquidity", 
                    trade_impact_pct
                ));
                all_good = false;
            }
        }
        Err(e) => {
            result.warnings.push(format!("Failed to fetch liquidity data: {}", e));
            result.liquidity_check = false;
            all_good = false;
        }
    }

    // Gas economics check
    result.gas_economics = opp.net_profit_usd > dec!(0) && opp.roi_pct > dec!(0.01);
    if !result.gas_economics {
        result.warnings.push("Insufficient profit after gas".to_string());
        all_good = false;
    }

    // Slippage check with volatility adjustment
    let volatility_slippage_factor = match volatility_metrics.impact_assessment {
        VolatilityImpact::Low => dec!(1),
        VolatilityImpact::Moderate => dec!(1.5),
        VolatilityImpact::High => dec!(2),
        VolatilityImpact::Extreme => dec!(3),
    };
    
    let estimated_slippage_bps = (opp.size_eth * dec!(50) * volatility_slippage_factor) / dec!(1);
    result.slippage_acceptable = estimated_slippage_bps < Decimal::from(MAX_SLIPPAGE_BPS);
    if !result.slippage_acceptable {
        result.warnings.push(format!(
            "Estimated slippage too high: {} bps (volatility-adjusted)", 
            estimated_slippage_bps
        ));
        all_good = false;
    }

    result.all_passed = all_good;
    result
}
