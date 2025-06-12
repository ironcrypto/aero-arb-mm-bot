//! Trade execution engine

use alloy::{
    network::EthereumWallet,
    primitives::{Address, address, keccak256, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::eth::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use anyhow::{Context, Result};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tracing::{info, warn};
use rust_decimal_macros::dec;
use rust_decimal::prelude::ToPrimitive;
use crate::{
    config::{Config, CONFIG},
    types::{ArbitrageOpportunity, TradeExecution, ExecutionStatus, TradeType, VolatilityMetrics},
    ConcreteProvider,
};

pub struct TradeExecutionEngine {
    pub sepolia_provider: Option<Arc<ConcreteProvider>>,
    pub wallet: Option<EthereumWallet>,
}

impl TradeExecutionEngine {
    pub async fn new(config: &Config) -> Result<Self> {
        let (sepolia_provider, wallet) = if config.enable_trade_execution {
            // Setup Sepolia provider
            let alchemy_key = config.alchemy_api_key.as_ref()
                .expect("ALCHEMY_API_KEY is required");
            let sepolia_url = format!("https://base-sepolia.g.alchemy.com/v2/{}", alchemy_key);
            let sepolia_provider = Arc::new(
                ProviderBuilder::new()
                    .on_http(sepolia_url.parse()?)
                    .boxed()
            );

            // Setup wallet if private key provided
            let wallet = if let Some(pk) = &config.private_key {
                let signer = PrivateKeySigner::from_str(pk)
                    .context("Failed to parse private key")?;
                Some(EthereumWallet::from(signer))
            } else {
                None
            };

            (Some(sepolia_provider), wallet)
        } else {
            (None, None)
        };

        Ok(Self {
            sepolia_provider,
            wallet,
        })
    }

    pub async fn simulate_trade_execution(
        &self,
        opportunity: &ArbitrageOpportunity,
        volatility_metrics: &VolatilityMetrics,
    ) -> Result<TradeExecution> {
        use crate::execution::simulation::create_simulated_execution;
        use std::time::Instant;
        
        let execution_start = Instant::now();
        let execution_id = uuid::Uuid::new_v4().to_string();

        info!("🚀 Simulating trade execution for opportunity {}", opportunity.id);

        // Check if we're in simulation mode or have real execution capability
        if self.sepolia_provider.is_none() || self.wallet.is_none() {
            // Pure simulation mode
            return create_simulated_execution(
                execution_id,
                opportunity,
                volatility_metrics,
                execution_start,
            ).await;
        }

        // Testnet execution simulation
        match self.execute_on_testnet(opportunity, volatility_metrics).await {
            Ok(tx_hash) => {
                let execution_time = execution_start.elapsed().as_millis() as u64;
                
                Ok(TradeExecution {
                    id: execution_id,
                    opportunity_id: opportunity.id.clone(),
                    timestamp: chrono::Utc::now(),
                    network: "Base Sepolia".to_string(),
                    trade_type: if opportunity.direction.contains("Buy on Aerodrome") {
                        TradeType::BuyDexSellCex
                    } else {
                        TradeType::BuyCexSellDex
                    },
                    status: ExecutionStatus::Success,
                    tx_hash: Some(tx_hash),
                    gas_used: Some(150000), // Estimated
                    gas_price_gwei: Some(rust_decimal::Decimal::from(CONFIG.max_gas_price_gwei)),
                    execution_time_ms: execution_time,
                    expected_profit_usd: opportunity.net_profit_usd,
                    actual_profit_usd: Some(opportunity.net_profit_usd * rust_decimal_macros::dec!(0.95)), // 5% slippage
                    slippage_bps: Some(50),
                    error_message: None,
                })
            }
            Err(e) => {
                warn!("Testnet execution failed: {}", e);
                self.create_failed_execution(
                    execution_id,
                    opportunity,
                    execution_start,
                    e.to_string(),
                ).await
            }
        }
    }

    async fn execute_on_testnet(
        &self,
        opportunity: &ArbitrageOpportunity,
        _volatility_metrics: &VolatilityMetrics,
    ) -> Result<String> {
        use crate::config::EXECUTION_TIMEOUT_SECS;
        
        let provider = self.sepolia_provider.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sepolia provider not initialized"))?;
        
        let _wallet = self.wallet.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Wallet not initialized"))?;

        // For testing, we'll use Uniswap V2 Router on Sepolia
        // We can replace this with any DEX router on Sepolia
        const UNISWAP_V2_ROUTER_SEPOLIA: Address = address!("0xC532a74256D3Db42D0Bf7a0400fEFDbad7694008");

        // Build swap transaction data
        let swap_data = self.encode_swap_data(opportunity)?;
        
        // Determine value - if buying WETH with USDC, no ETH value needed
        // If selling WETH for USDC, we need to send ETH
        let value = if opportunity.direction.contains("Sell on Aerodrome") {
            U256::from((opportunity.size_eth * dec!(1e18)).to_u128().unwrap_or(0))
        } else {
            U256::from(0)
        };
        
        // Build the transaction
        let tx = TransactionRequest::default()
            .to(UNISWAP_V2_ROUTER_SEPOLIA)
            .value(value)
            .input(swap_data.into())
            .gas_limit(300000)
            .max_fee_per_gas(CONFIG.max_gas_price_gwei as u128 * 1_000_000_000)
            .max_priority_fee_per_gas(1_000_000_000); // 1 gwei

        info!("📤 Sending transaction to Sepolia:");
        info!("   Router: {:?}", UNISWAP_V2_ROUTER_SEPOLIA);
        info!("   Value: {} ETH", opportunity.size_eth);

        // Sign and send transaction
        let pending_tx = provider
            .send_transaction(tx)
            .await
            .context("Failed to send transaction")?;

        let tx_hash = format!("{:?}", pending_tx.tx_hash());
        
        info!("📡 Transaction sent on Base Sepolia: {}", tx_hash);

        // Wait for confirmation with timeout
        tokio::select! {
            result = pending_tx.get_receipt() => {
                match result {
                    Ok(receipt) => {
                        info!("✅ Transaction confirmed: {:?}", receipt.transaction_hash);
                        Ok(tx_hash)
                    }
                    Err(e) => Err(anyhow::anyhow!("Transaction failed: {}", e))
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(EXECUTION_TIMEOUT_SECS)) => {
                Err(anyhow::anyhow!("Transaction timeout after {} seconds", EXECUTION_TIMEOUT_SECS))
            }
        }
    }

    fn encode_swap_data(&self, opportunity: &ArbitrageOpportunity) -> Result<Vec<u8>> {
        use rust_decimal::prelude::*;
        use rust_decimal_macros::dec;
        use crate::types::{USDC_SEPOLIA, WETH_SEPOLIA};
        
        // Calculate amounts
        let amount_in = U256::from((opportunity.size_eth * dec!(1e18)).to_u128().unwrap_or(0));
        
        // Calculate minimum amount out with slippage
        let expected_out = opportunity.size_eth * opportunity.cex_price;
        let slippage_factor = dec!(1) - (rust_decimal::Decimal::from(CONFIG.slippage_tolerance_bps) / dec!(10000));
        let amount_out_min = U256::from((expected_out * slippage_factor * dec!(1e6)).to_u128().unwrap_or(0));
        
        // Build the path based on trade direction
        let path: Vec<Address> = if opportunity.direction.contains("Buy on Aerodrome") {
            vec![USDC_SEPOLIA, WETH_SEPOLIA]
        } else {
            vec![WETH_SEPOLIA, USDC_SEPOLIA]
        };
        
        let to = address!("0000000000000000000000000000000000000001");
        let deadline = U256::from(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs() + 1200);
        
        // Encode the function call
        let mut encoded = keccak256("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)")[..4].to_vec();
        
        // Encode parameters
        encoded.extend_from_slice(&amount_in.to_be_bytes::<32>());
        encoded.extend_from_slice(&amount_out_min.to_be_bytes::<32>());
        encoded.extend_from_slice(&U256::from(160).to_be_bytes::<32>());
        encoded.extend_from_slice(&[0u8; 12]);
        encoded.extend_from_slice(to.as_slice());
        encoded.extend_from_slice(&deadline.to_be_bytes::<32>());
        encoded.extend_from_slice(&U256::from(path.len()).to_be_bytes::<32>());
        
        for addr in path {
            encoded.extend_from_slice(&[0u8; 12]); // Padding for address
            encoded.extend_from_slice(addr.as_slice());
        }
        
        info!("📝 Encoded swap data for testnet execution");
        Ok(encoded)
    }

    async fn create_failed_execution(
        &self,
        execution_id: String,
        opportunity: &ArbitrageOpportunity,
        start_time: std::time::Instant,
        error: String,
    ) -> Result<TradeExecution> {
        Ok(TradeExecution {
            id: execution_id,
            opportunity_id: opportunity.id.clone(),
            timestamp: chrono::Utc::now(),
            network: "Base Sepolia".to_string(),
            trade_type: if opportunity.direction.contains("Buy on Aerodrome") {
                TradeType::BuyDexSellCex
            } else {
                TradeType::BuyCexSellDex
            },
            status: ExecutionStatus::Failed,
            tx_hash: None,
            gas_used: None,
            gas_price_gwei: None,
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            expected_profit_usd: opportunity.net_profit_usd,
            actual_profit_usd: None,
            slippage_bps: None,
            error_message: Some(error),
        })
    }
}
