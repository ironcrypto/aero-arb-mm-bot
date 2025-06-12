//! Pool information retrieval and management

use alloy::{
    primitives::{Address, keccak256},
    providers::Provider,
    rpc::types::eth::TransactionRequest,
    sol_types::SolValue,
};
use anyhow::{Context, Result};
use std::time::Instant;
use tracing::debug;
use crate::types::PoolInfo;

pub async fn get_pool_info_internal(
    provider: &dyn Provider, 
    name: &str, 
    address: Address
) -> Result<PoolInfo> {
    debug!("Getting info for pool: {} at {}", name, address);
    
    let token0_data = keccak256("token0()")[..4].to_vec();
    let token1_data = keccak256("token1()")[..4].to_vec();
    let stable_data = keccak256("stable()")[..4].to_vec();
    
    let tx0 = TransactionRequest::default().to(address).input(token0_data.into());
    let tx1 = TransactionRequest::default().to(address).input(token1_data.into());
    let tx_stable = TransactionRequest::default().to(address).input(stable_data.into());
    
    let token0 = Address::abi_decode(&provider.call(&tx0).await
        .context("Failed to get token0")?, true)?;
    let token1 = Address::abi_decode(&provider.call(&tx1).await
        .context("Failed to get token1")?, true)?;
    let is_stable = bool::abi_decode(&provider.call(&tx_stable).await
        .context("Failed to get stable flag")?, true)?;
    
    Ok(PoolInfo {
        address,
        name: name.to_string(),
        token0,
        token1,
        is_stable,
        min_liquidity: rust_decimal_macros::dec!(1000),
        last_update: Instant::now(),
    })
}
