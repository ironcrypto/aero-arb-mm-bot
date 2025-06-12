//! Pool reserves fetching and management

use alloy::{
    primitives::{Address, keccak256, U256},
    providers::Provider,
    rpc::types::eth::TransactionRequest,
    sol_types::SolValue,
};
use anyhow::{Context, Result};
use std::sync::Arc;
use crate::{
    errors::{BotError, BotResult},
    network::retry::{retry_with_backoff, RetryConfig},
    types::PoolInfo,
    ConcreteProvider,
};

pub async fn get_pool_reserves(provider: &dyn Provider, pool: Address) -> Result<(U256, U256)> {
    let data = keccak256("getReserves()")[..4].to_vec();
    let tx = TransactionRequest::default()
        .to(pool)
        .input(data.into());
    
    let result = provider.call(&tx).await
        .context("Failed to call getReserves")?;
    let decoded = <(U256, U256, U256)>::abi_decode(&result, true)
        .context("Failed to decode reserves")?;
    Ok((decoded.0, decoded.1))
}

pub async fn get_pool_reserves_enhanced(
    provider: &dyn Provider,
    pool: Address,
    pool_name: &str,
) -> BotResult<(U256, U256)> {
    let operation = || async {
        get_pool_reserves(provider, pool).await
    };
    
    retry_with_backoff(
        operation,
        &RetryConfig::default(),
        &format!("get reserves for {}", pool_name),
    ).await
    .map_err(|e| match e {
        BotError::Network { .. } => e,
        _ => BotError::Contract {
            contract: pool,
            message: format!("Failed to get reserves for {}", pool_name),
            source: anyhow::anyhow!("{}", e),
        }
    })
}

pub async fn calculate_pool_price_safe_with_retry(
    provider: &Arc<ConcreteProvider>,
    pool_info: &PoolInfo,
) -> BotResult<rust_decimal::Decimal> {
    let operation = || async {
        calculate_pool_price_safe(provider.as_ref(), pool_info).await
    };
    
    retry_with_backoff(
        operation,
        &RetryConfig {
            max_attempts: 3,
            initial_delay_ms: 200,
            ..Default::default()
        },
        &format!("calculate price for {}", pool_info.name),
    ).await
    .map_err(|e| BotError::Contract {
        contract: pool_info.address,
        message: "Failed to calculate pool price".to_string(),
        source: anyhow::anyhow!("{}", e),
    })
}

pub async fn calculate_pool_price_safe(
    provider: &dyn Provider,
    pool_info: &PoolInfo,
) -> Result<rust_decimal::Decimal> {
    use rust_decimal::prelude::*;
    use rust_decimal_macros::dec;
    use std::str::FromStr;
    use crate::{
        types::{WETH_MAINNET, USDC_MAINNET, USDBC_MAINNET},
        validation::validate_price,
        utils::pow10,
    };
    
    let (r0, r1) = get_pool_reserves_enhanced(provider, pool_info.address, &pool_info.name).await
        .map_err(|e| anyhow::anyhow!("Failed to get reserves for price calculation: {}", e))?;
    
    if r0 == U256::from(0) || r1 == U256::from(0) {
        return Err(anyhow::anyhow!("Pool has zero reserves"));
    }
    
    let (weth_reserve, usd_reserve, usd_decimals) = if pool_info.token0 == WETH_MAINNET {
        let decimals = if pool_info.token1 == USDC_MAINNET || pool_info.token1 == USDBC_MAINNET { 6 } else { 18 };
        (r0, r1, decimals)
    } else if pool_info.token1 == WETH_MAINNET {
        let decimals = if pool_info.token0 == USDC_MAINNET || pool_info.token0 == USDBC_MAINNET { 6 } else { 18 };
        (r1, r0, decimals)
    } else {
        return Err(anyhow::anyhow!("Not a WETH/USD pool"));
    };
    
    let weth_amount = Decimal::from_str(&weth_reserve.to_string())
        .context("Failed to parse WETH reserve")? / pow10(18);
    let usd_amount = Decimal::from_str(&usd_reserve.to_string())
        .context("Failed to parse USD reserve")? / pow10(usd_decimals);
    
    if weth_amount == dec!(0) {
        return Err(anyhow::anyhow!("WETH amount is zero"));
    }
    
    let price = usd_amount / weth_amount;
    validate_price(price, "DEX")?;
    
    Ok(price)
}
