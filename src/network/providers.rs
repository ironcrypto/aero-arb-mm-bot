//! Network provider setup and price fetching

use alloy::{
    providers::{Provider, ProviderBuilder},
};
use anyhow::{Context, Result};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};
use crate::{
    config::Config,
    errors::{BotError, BotResult},
    network::retry::{retry_with_backoff, RetryConfig},
    ConcreteProvider,
};

pub async fn setup_mainnet_provider(config: &Config) -> Result<Arc<ConcreteProvider>> {
    let alchemy_key = config.alchemy_api_key.as_ref()
        .expect("ALCHEMY_API_KEY is required");
    let rpc_url = format!("https://base-mainnet.g.alchemy.com/v2/{}", alchemy_key);
    
    let provider: Arc<ConcreteProvider> = Arc::new(
        ProviderBuilder::new()
            .on_http(rpc_url.parse()?)
            .boxed()
    );
    
    info!("🔗 Testing connection to Base network...");
    let block = retry_with_backoff(
        || async {
            provider.get_block_number().await
                .context("Failed to get block number")
        },
        &RetryConfig {
            max_attempts: 5,
            initial_delay_ms: 500,
            max_delay_ms: 10000,
            exponential_base: 2.0,
        },
        "Base network connection",
    ).await
    .map_err(|e| {
        warn!("⚠️ Network connection attempt failed: {}", e);
        anyhow::anyhow!("Network connection failed: {}", e)
    })?;
    
    info!("✅ Connected to Base at block {}", block);
    Ok(provider)
}

pub async fn get_binance_price_enhanced() -> BotResult<Decimal> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| {
            warn!("⚠️ Failed to initialize HTTP client: {}", e);
            BotError::Network {
                message: "Failed to build HTTP client".to_string(),
                source: Some(e.into()),
                retry_count: 0,
            }
        })?;
    
    let operation = || async {
        let response = client
            .get("https://api.binance.com/api/v3/ticker/price?symbol=ETHUSDC")
            .send()
            .await
            .context("HTTP request failed")?;
            
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("⚠️ Binance API returned error status {}: {}", status, body);
            return Err(anyhow::anyhow!(
                "Binance API error: {} - {}",
                status,
                body
            ));
        }
        
        let json: serde_json::Value = response.json().await
            .context("Failed to parse JSON response")?;
            
        let price_str = json["price"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'price' field in response"))?;
            
        let price = Decimal::from_str(price_str)
            .context("Failed to parse price string")?;
            
        Ok(price)
    };
    
    let price = retry_with_backoff(
        operation,
        &RetryConfig {
            max_attempts: 5,
            initial_delay_ms: 200,
            ..Default::default()
        },
        "Binance price fetch",
    ).await?;
    
    if price <= dec!(0) || price < dec!(100) || price > dec!(100000) {
        warn!("⚠️ Invalid price received from Binance: {}", price);
        return Err(BotError::PriceValidation {
            source: Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Binance price validation failed")),
            price,
            reason: "Price outside valid range".to_string(),
        });
    }
    
    Ok(price)
}
