//! Liquidity depth analysis

use alloy::providers::Provider;
use anyhow::{Context, Result};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::str::FromStr;
use crate::{
    pools::get_pool_reserves_enhanced,
    types::{LiquidityDepth, DepthQuality, PoolInfo, WETH_MAINNET, USDC_MAINNET, USDBC_MAINNET},
    utils::pow10,
};

pub async fn analyze_liquidity_depth(
    provider: &dyn Provider,
    pool_info: &PoolInfo,
    fair_value_price: Decimal,
) -> Result<LiquidityDepth> {
    let (r0, r1) = get_pool_reserves_enhanced(provider, pool_info.address, &pool_info.name).await
        .map_err(|e| anyhow::anyhow!("Failed to get reserves for liquidity analysis: {}", e))?;
    
    let (weth_reserves, usd_reserves, usd_decimals) = if pool_info.token0 == WETH_MAINNET {
        let decimals = if pool_info.token1 == USDC_MAINNET || pool_info.token1 == USDBC_MAINNET { 6 } else { 18 };
        (r0, r1, decimals)
    } else if pool_info.token1 == WETH_MAINNET {
        let decimals = if pool_info.token0 == USDC_MAINNET || pool_info.token0 == USDBC_MAINNET { 6 } else { 18 };
        (r1, r0, decimals)
    } else {
        return Err(anyhow::anyhow!("Not a WETH/USD pool"));
    };
    
    let weth_amount = Decimal::from_str(&weth_reserves.to_string())
        .context("Failed to parse WETH reserve")? / pow10(18);
    let usd_amount = Decimal::from_str(&usd_reserves.to_string())
        .context("Failed to parse USD reserve")? / pow10(usd_decimals);
    
    let total_liquidity_usd = (weth_amount * fair_value_price) + usd_amount;
    
    let depth_quality = match total_liquidity_usd {
        liq if liq > dec!(10000000) => DepthQuality::Excellent,
        liq if liq > dec!(1000000) => DepthQuality::Good,
        liq if liq > dec!(100000) => DepthQuality::Fair,
        _ => DepthQuality::Poor,
    };
    
    Ok(LiquidityDepth {
        total_liquidity_usd,
        weth_reserves: weth_amount,
        usd_reserves: usd_amount,
        depth_quality,
    })
}
