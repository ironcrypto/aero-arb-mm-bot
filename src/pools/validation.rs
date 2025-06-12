//! Pool validation functions

use alloy::{primitives::Address, primitives::U256};
use anyhow::Result;
use std::sync::Arc;
use tracing::info;
use crate::{
    network::retry::{retry_with_backoff, RetryConfig},
    pools::{get_pool_info_internal, get_pool_reserves},
    types::PoolInfo,
    ConcreteProvider,
};

pub async fn validate_pool_with_retry(
    provider: &Arc<ConcreteProvider>,
    name: &str,
    address: Address,
    weth_addr: Address,
    usdc_addr: Address,
    usdbc_addr: Address,
) -> Result<PoolInfo> {
    retry_with_backoff(
        || async {
            let pool_info = get_pool_info_internal(provider.as_ref(), name, address).await?;
            
            // Validate it's a WETH/USD pool
            if !((pool_info.token0 == weth_addr || pool_info.token1 == weth_addr) &&
                 (pool_info.token0 == usdc_addr || pool_info.token1 == usdc_addr || 
                  pool_info.token0 == usdbc_addr || pool_info.token1 == usdbc_addr)) {
                return Err(anyhow::anyhow!("Not a WETH/USD pool"));
            }
            
            let (r0, r1) = get_pool_reserves(provider.as_ref(), pool_info.address).await?;
            if r0 == U256::from(0) || r1 == U256::from(0) {
                return Err(anyhow::anyhow!("Pool has zero liquidity"));
            }
            
            Ok(pool_info)
        },
        &RetryConfig::default(),
        &format!("validate pool {}", name),
    ).await
    .map_err(|e| anyhow::anyhow!("Pool validation failed: {}", e))
}

pub async fn initialize_and_validate_pools(
    provider: &Arc<ConcreteProvider>,
    config: &crate::config::Config,
) -> Result<Vec<PoolInfo>> {
    use crate::types::{
        POOLS_MAINNET, POOLS_SEPOLIA,
        WETH_MAINNET, USDC_MAINNET, USDBC_MAINNET,
        WETH_SEPOLIA, USDC_SEPOLIA,
    };
    
    // Determine which pools to use based on network
    let pools_to_validate = if config.network == "mainnet" {
        POOLS_MAINNET
    } else {
        POOLS_SEPOLIA
    };
    
    let (weth_addr, usdc_addr, usdbc_addr) = if config.network == "mainnet" {
        (WETH_MAINNET, USDC_MAINNET, USDBC_MAINNET)
    } else {
        (WETH_SEPOLIA, USDC_SEPOLIA, USDC_SEPOLIA) // Use USDC for both on Sepolia
    };
    
    info!("\n🔍 Validating Aerodrome pools on {}...", config.network);
    let mut valid_pools = Vec::new();
    let mut pool_errors = 0;
    
    for (name, address) in pools_to_validate {
        match validate_pool_with_retry(provider, name, *address, weth_addr, usdc_addr, usdbc_addr).await {
            Ok(pool_info) => {
                info!("✅ {} - Valid WETH/USD pool", name);
                valid_pools.push(pool_info);
            }
            Err(e) => {
                tracing::error!("❌ {} - Validation failed: {}", name, e);
                pool_errors += 1;
                
                if pool_errors >= pools_to_validate.len() {
                    return Err(anyhow::anyhow!("All pools failed validation"));
                }
            }
        }
    }
    
    if valid_pools.is_empty() {
        return Err(anyhow::anyhow!("No valid pools found after validation"));
    }
    
    info!("✅ Validated {} pools (failed: {})", valid_pools.len(), pool_errors);
    Ok(valid_pools)
}
