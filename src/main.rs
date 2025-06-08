// PRODUCTION-READY AERODROME BOT v0.5.0 - TRADE EXECUTION SIMULATION & VOLATILITY
// Enhanced with testnet trade simulation and multi-timeframe volatility analysis

use alloy::{
    primitives::{Address, address, U256, keccak256},
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::types::eth::TransactionRequest,
    sol_types::SolValue,
    transports::BoxTransport,
    signers::local::PrivateKeySigner,
    network::EthereumWallet,
};
use anyhow::{Result, Context};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use serde::Serialize;
use chrono::{DateTime, Utc};
use std::env;
use lazy_static::lazy_static;
use tracing_subscriber::layer::SubscriberExt;
use std::time::{Duration, Instant, SystemTime};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::error::Error;
use thiserror::Error;


// Type alias for our concrete provider
type ConcreteProvider = RootProvider<BoxTransport>;


// Network-specific constants
const WETH_MAINNET: Address = address!("4200000000000000000000000000000000000006");
const USDC_MAINNET: Address = address!("833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
const USDBC_MAINNET: Address = address!("d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA");

// Base Sepolia testnet addresses
const WETH_SEPOLIA: Address = address!("4200000000000000000000000000000000000006");
const USDC_SEPOLIA: Address = address!("AF33ADd7918F685B2A82C1077bd8c07d220FFA04"); // Base Sepolia USDC
#[allow(dead_code)]
const UNISWAP_V2_ROUTER_SEPOLIA: Address = address!("0xC532a74256D3Db42D0Bf7a0400fEFDbad7694008");

// Mainnet pools
const POOLS_MAINNET: &[(&str, Address)] = &[
    ("vAMM-WETH/USDbC", address!("B4885Bc63399BF5518b994c1d0C153334Ee579D0")),
    ("WETH/USDC", address!("cDAC0d6c6C59727a65F871236188350531885C43")),
];

// Sepolia testnet pools (for execution)
const POOLS_SEPOLIA: &[(&str, Address)] = &[
    ("WETH/USDC-Sepolia", address!("92b8274aba7ab667bee7eb776ec1de32438d90bf")), 
];

// Configuration with bounds
const MIN_TRADE_SIZE_ETH: Decimal = dec!(0.01);
const MAX_TRADE_SIZE_ETH: Decimal = dec!(10.0);
const MIN_PROFIT_USD: Decimal = dec!(0.10);
const MAX_SLIPPAGE_BPS: u32 = 100; // 1%
const PRICE_STALENESS_SECONDS: u64 = 10;
const MAX_PRICE_DEVIATION_PCT: Decimal = dec!(10); // 10% max difference between DEX/CEX

// Market Making Constants
const DEFAULT_SPREAD_BPS: u32 = 30; // 0.3% default spread
const MIN_SPREAD_BPS: u32 = 10; // 0.1% minimum spread
const MAX_SPREAD_BPS: u32 = 200; // 2% maximum spread
const TARGET_INVENTORY_RATIO: Decimal = dec!(0.5); // 50% target inventory balance
const REBALANCE_THRESHOLD: Decimal = dec!(0.1); // 10% inventory deviation triggers rebalance

// Trade Execution Constants
const DEFAULT_GAS_PRICE_GWEI: u32 = 50;
const MAX_GAS_PRICE_GWEI: u32 = 200;
const EXECUTION_TIMEOUT_SECS: u64 = 30;

lazy_static! {
    static ref CONFIG: Config = Config::load();
}

#[derive(Debug, Clone)]
struct Config {
    trade_size_eth: Decimal,
    min_profit_usd: Decimal,
    max_consecutive_errors: u32,
    circuit_breaker_cooldown_secs: u64,
    enable_safety_checks: bool,
    // Market Making Configuration
    enable_market_making: bool,
    base_spread_bps: u32,
    max_position_size_eth: Decimal,
    inventory_target_ratio: Decimal,
    // Trade Execution Configuration
    enable_trade_execution: bool,
    network: String,
    #[allow(dead_code)]
    execution_network: String,          // For execution (sepolia)
    max_gas_price_gwei: u32,
    slippage_tolerance_bps: u32,
    private_key: Option<String>,
    // Volatility Configuration
    volatility_threshold: Decimal,
    volatility_spread_multiplier: Decimal,
    // Alchemy API Key
    alchemy_api_key: Option<String>,
}

impl Config {
    fn load() -> Self {
        Self {
            alchemy_api_key: env::var("ALCHEMY_API_KEY").ok(),
            trade_size_eth: env::var("TRADE_SIZE_ETH")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(0.1))
                .max(MIN_TRADE_SIZE_ETH)
                .min(MAX_TRADE_SIZE_ETH),
            min_profit_usd: env::var("MIN_PROFIT_USD")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(0.50))
                .max(MIN_PROFIT_USD),
            max_consecutive_errors: 5,
            circuit_breaker_cooldown_secs: 300, // 5 minutes
            enable_safety_checks: env::var("ENABLE_SAFETY_CHECKS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            // Market Making defaults
            enable_market_making: env::var("ENABLE_MARKET_MAKING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            base_spread_bps: env::var("BASE_SPREAD_BPS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_SPREAD_BPS)
                .max(MIN_SPREAD_BPS)
                .min(MAX_SPREAD_BPS),
            max_position_size_eth: env::var("MAX_POSITION_SIZE_ETH")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(5.0)),
            inventory_target_ratio: TARGET_INVENTORY_RATIO,
            // Trade Execution Configuration
            enable_trade_execution: env::var("ENABLE_TRADE_EXECUTION")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            network: env::var("NETWORK")
                .unwrap_or_else(|_| "mainnet".to_string()),
            execution_network: env::var("EXECUTION_NETWORK")
                .unwrap_or_else(|_| "sepolia".to_string()),                
            max_gas_price_gwei: env::var("MAX_GAS_PRICE_GWEI")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_GAS_PRICE_GWEI)
                .min(MAX_GAS_PRICE_GWEI),
            slippage_tolerance_bps: env::var("SLIPPAGE_TOLERANCE_BPS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(50) // 0.5% default
                .min(MAX_SLIPPAGE_BPS),
            private_key: env::var("PRIVATE_KEY").ok(),
            // Volatility Configuration
            volatility_threshold: env::var("VOLATILITY_THRESHOLD")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(5.0)), // 5% threshold
            volatility_spread_multiplier: env::var("VOLATILITY_SPREAD_MULTIPLIER")
                .ok()
                .and_then(|s| Decimal::from_str(&s).ok())
                .unwrap_or(dec!(2.0)), // 2x multiplier for high volatility
        }
    }
}

// Trade Execution Types
#[derive(Debug, Clone, Serialize)]
struct TradeExecution {
    id: String,
    opportunity_id: String,
    timestamp: DateTime<Utc>,
    network: String,
    trade_type: TradeType,
    status: ExecutionStatus,
    tx_hash: Option<String>,
    gas_used: Option<u64>,
    gas_price_gwei: Option<Decimal>,
    execution_time_ms: u64,
    expected_profit_usd: Decimal,
    actual_profit_usd: Option<Decimal>,
    slippage_bps: Option<u32>,
    error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
enum TradeType {
    BuyDexSellCex,
    BuyCexSellDex,
}

#[derive(Debug, Clone, Serialize)]
enum ExecutionStatus {
    Simulated,
    Success,
    Failed,
}

// Volatility Analysis Types
#[derive(Debug, Clone, Serialize)]
pub struct VolatilityMetrics {
    short_term_volatility: Decimal,  // 5 min
    medium_term_volatility: Decimal, // 30 min
    long_term_volatility: Decimal,   // 1 hour
    volatility_trend: VolatilityTrend,
    impact_assessment: VolatilityImpact,
    recommended_adjustments: VolatilityAdjustments,
}

#[derive(Debug, Clone, Serialize)]
enum VolatilityTrend {
    Increasing,
    Decreasing,
    Stable,
    Volatile,
}

#[derive(Debug, Clone, Serialize)]
enum VolatilityImpact {
    Low,      // < 2%
    Moderate, // 2-5%
    High,     // 5-10%
    Extreme,  // > 10%
}

#[derive(Debug, Clone, Serialize)]
struct VolatilityAdjustments {
    spread_multiplier: Decimal,
    position_size_factor: Decimal,
    execution_urgency: ExecutionUrgency,
}

#[derive(Debug, Clone, Serialize)]
enum ExecutionUrgency {
    Fast,
    Normal,
    Cautious,
}

// Enhanced Volatility Calculator with multiple timeframes
pub struct MultiTimeframeVolatilityCalculator {
    short_term: Arc<RwLock<VolatilityCalculator>>,   // 5 minutes
    medium_term: Arc<RwLock<VolatilityCalculator>>,  // 30 minutes
    long_term: Arc<RwLock<VolatilityCalculator>>,    // 1 hour
}

impl MultiTimeframeVolatilityCalculator {
    pub fn new() -> Self {
        Self {
            short_term: Arc::new(RwLock::new(VolatilityCalculator::new(300))),    // 5 min
            medium_term: Arc::new(RwLock::new(VolatilityCalculator::new(1800))),  // 30 min
            long_term: Arc::new(RwLock::new(VolatilityCalculator::new(3600))),    // 1 hour
        }
    }

    pub async fn add_price(&self, price: Decimal) {
        let price_f64 = price.to_f64().unwrap_or(0.0);
        
        // Update all timeframes
        self.short_term.write().await.add_value(price_f64);
        self.medium_term.write().await.add_value(price_f64);
        self.long_term.write().await.add_value(price_f64);
    }

    pub async fn get_volatility_metrics(&self) -> VolatilityMetrics {
        let short_vol = self.short_term.read().await.calculate_volatility_percentage()
            .map(|v| Decimal::from_f64(v).unwrap_or(dec!(0)))
            .unwrap_or(dec!(0));
            
        let medium_vol = self.medium_term.read().await.calculate_volatility_percentage()
            .map(|v| Decimal::from_f64(v).unwrap_or(dec!(0)))
            .unwrap_or(dec!(0));
            
        let long_vol = self.long_term.read().await.calculate_volatility_percentage()
            .map(|v| Decimal::from_f64(v).unwrap_or(dec!(0)))
            .unwrap_or(dec!(0));

        // Determine volatility trend
        let trend = if short_vol > medium_vol * dec!(1.2) && medium_vol > long_vol * dec!(1.2) {
            VolatilityTrend::Increasing
        } else if short_vol < medium_vol * dec!(0.8) && medium_vol < long_vol * dec!(0.8) {
            VolatilityTrend::Decreasing
        } else if (short_vol - long_vol).abs() < dec!(1) {
            VolatilityTrend::Stable
        } else {
            VolatilityTrend::Volatile
        };

        // Assess impact
        let impact = match short_vol {
            v if v < dec!(2) => VolatilityImpact::Low,
            v if v < dec!(5) => VolatilityImpact::Moderate,
            v if v < dec!(10) => VolatilityImpact::High,
            _ => VolatilityImpact::Extreme,
        };

        // Calculate recommended adjustments
        let spread_multiplier = match impact {
            VolatilityImpact::Low => dec!(1.0),
            VolatilityImpact::Moderate => dec!(1.5),
            VolatilityImpact::High => dec!(2.0),
            VolatilityImpact::Extreme => dec!(3.0),
        };

        let position_size_factor = match impact {
            VolatilityImpact::Low => dec!(1.0),
            VolatilityImpact::Moderate => dec!(0.8),
            VolatilityImpact::High => dec!(0.5),
            VolatilityImpact::Extreme => dec!(0.25),
        };

        let execution_urgency = match (&impact, &trend) {
            (VolatilityImpact::Extreme, _) => ExecutionUrgency::Cautious,
            (VolatilityImpact::High, VolatilityTrend::Increasing) => ExecutionUrgency::Cautious,
            (VolatilityImpact::Low, _) => ExecutionUrgency::Normal,
            (_, VolatilityTrend::Stable) => ExecutionUrgency::Fast,
            _ => ExecutionUrgency::Normal,
        };

        VolatilityMetrics {
            short_term_volatility: short_vol,
            medium_term_volatility: medium_vol,
            long_term_volatility: long_vol,
            volatility_trend: trend,
            impact_assessment: impact,
            recommended_adjustments: VolatilityAdjustments {
                spread_multiplier,
                position_size_factor,
                execution_urgency,
            },
        }
    }
}

// Trade Execution Engine
struct TradeExecutionEngine {
    sepolia_provider: Option<Arc<ConcreteProvider>>,
    wallet: Option<EthereumWallet>,
}

impl TradeExecutionEngine {
    async fn new(config: &Config) -> Result<Self> {
        let (sepolia_provider, wallet) = if config.enable_trade_execution {
            // Setup Sepolia provider
            let alchemy_key = CONFIG.alchemy_api_key.as_ref()
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

    async fn simulate_trade_execution(
        &self,
        opportunity: &ArbitrageOpportunity,
        volatility_metrics: &VolatilityMetrics,
    ) -> Result<TradeExecution> {
        let execution_start = Instant::now();
        let execution_id = uuid::Uuid::new_v4().to_string();

        info!("🚀 Simulating trade execution for opportunity {}", opportunity.id);

        // Check if we're in simulation mode or have real execution capability
        if self.sepolia_provider.is_none() || self.wallet.is_none() {
            // Pure simulation mode
            return self.create_simulated_execution(
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
                    timestamp: Utc::now(),
                    network: "Base Sepolia".to_string(),
                    trade_type: if opportunity.direction.contains("Buy on Aerodrome") {
                        TradeType::BuyDexSellCex
                    } else {
                        TradeType::BuyCexSellDex
                    },
                    status: ExecutionStatus::Success,
                    tx_hash: Some(tx_hash),
                    gas_used: Some(150000), // Estimated
                    gas_price_gwei: Some(Decimal::from(CONFIG.max_gas_price_gwei)),
                    execution_time_ms: execution_time,
                    expected_profit_usd: opportunity.net_profit_usd,
                    actual_profit_usd: Some(opportunity.net_profit_usd * dec!(0.95)), // 5% slippage
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

    async fn create_simulated_execution(
        &self,
        execution_id: String,
        opportunity: &ArbitrageOpportunity,
        volatility_metrics: &VolatilityMetrics,
        start_time: Instant,
    ) -> Result<TradeExecution> {
        // Simulate network latency based on volatility
        let base_latency = 100;
        let volatility_latency = match volatility_metrics.impact_assessment {
            VolatilityImpact::Low => 0,
            VolatilityImpact::Moderate => 50,
            VolatilityImpact::High => 150,
            VolatilityImpact::Extreme => 300,
        };
        
        tokio::time::sleep(Duration::from_millis(base_latency + volatility_latency)).await;

        // Simulate success rate based on volatility
        let success_rate = match volatility_metrics.impact_assessment {
            VolatilityImpact::Low => 0.95,
            VolatilityImpact::Moderate => 0.85,
            VolatilityImpact::High => 0.70,
            VolatilityImpact::Extreme => 0.50,
        };

        let is_successful = rand::random::<f64>() < success_rate;

        // Calculate simulated slippage based on volatility
        let base_slippage_bps = 25;
        let volatility_slippage = match volatility_metrics.impact_assessment {
            VolatilityImpact::Low => 0,
            VolatilityImpact::Moderate => 25,
            VolatilityImpact::High => 75,
            VolatilityImpact::Extreme => 150,
        };
        let total_slippage_bps = base_slippage_bps + volatility_slippage;

        // Calculate actual profit after slippage
        let slippage_factor = dec!(1) - (Decimal::from(total_slippage_bps) / dec!(10000));
        let actual_profit = opportunity.net_profit_usd * slippage_factor;

        let execution_time = start_time.elapsed().as_millis() as u64;

        Ok(TradeExecution {
            id: execution_id,
            opportunity_id: opportunity.id.clone(),
            timestamp: Utc::now(),
            network: "Base Sepolia".to_string(),
            trade_type: if opportunity.direction.contains("Buy on Aerodrome") {
                TradeType::BuyDexSellCex
            } else {
                TradeType::BuyCexSellDex
            },
            status: if is_successful {
                ExecutionStatus::Simulated
            } else {
                ExecutionStatus::Failed
            },
            tx_hash: if is_successful {
                Some(format!("0x{}", uuid::Uuid::new_v4().to_string().replace("-", "")))
            } else {
                None
            },
            gas_used: Some(156000),
            gas_price_gwei: Some(dec!(0.12)),
            execution_time_ms: execution_time,
            expected_profit_usd: opportunity.net_profit_usd,
            actual_profit_usd: if is_successful { Some(actual_profit) } else { None },
            slippage_bps: if is_successful { Some(total_slippage_bps) } else { None },
            error_message: if !is_successful {
                Some("Simulated failure due to high volatility".to_string())
            } else {
                None
            },
        })
    }

    async fn execute_on_testnet(
        &self,
        opportunity: &ArbitrageOpportunity,
        _volatility_metrics: &VolatilityMetrics,
    ) -> Result<String> {
        let provider = self.sepolia_provider.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Sepolia provider not initialized"))?;
        
        let _wallet = self.wallet.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Wallet not initialized"))?;

        // For testing, we'll use Uniswap V2 Router on Sepolia
        // You can replace this with any DEX router on Sepolia
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
        info!("   Gas Limit: 300000");
        info!("   Max Gas Price: {} gwei", CONFIG.max_gas_price_gwei);

        // Sign and send transaction
        let pending_tx = provider
            .send_transaction(tx)
            .await
            .context("Failed to send transaction")?;

        let tx_hash = format!("{:?}", pending_tx.tx_hash());
        
        info!("📡 Transaction sent on Base Sepolia: {}", tx_hash);
        info!("   View on explorer: https://sepolia.basescan.org/tx/{}", tx_hash);

        // Wait for confirmation with timeout
        tokio::select! {
            result = pending_tx.get_receipt() => {
                match result {
                    Ok(receipt) => {
                        info!("✅ Transaction confirmed: {:?}", receipt.transaction_hash);
                        info!("   Block: {:?}", receipt.block_number);
                        info!("   Gas Used: {:?}", receipt.gas_used);
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
        // We'll encode a Uniswap V2 style swap for testing
        // Function: swapExactTokensForTokens(uint amountIn, uint amountOutMin, address[] path, address to, uint deadline)
        
        // Calculate amounts
        let amount_in = U256::from((opportunity.size_eth * dec!(1e18)).to_u128().unwrap_or(0));
        
        // Calculate minimum amount out with slippage
        let expected_out = opportunity.size_eth * opportunity.cex_price;
        let slippage_factor = dec!(1) - (Decimal::from(CONFIG.slippage_tolerance_bps) / dec!(10000));
        let amount_out_min = U256::from((expected_out * slippage_factor * dec!(1e6)).to_u128().unwrap_or(0)); // USDC has 6 decimals
        
        // Build the path based on trade direction
        let path: Vec<Address> = if opportunity.direction.contains("Buy on Aerodrome") {
            // Buy WETH on DEX: USDC -> WETH
            vec![USDC_SEPOLIA, WETH_SEPOLIA]
        } else {
            // Sell WETH on DEX: WETH -> USDC
            vec![WETH_SEPOLIA, USDC_SEPOLIA]
        };
        
        // Get the recipient address (the wallet address)
        let to = if let Some(_wallet) = &self.wallet {
            // In real implementation, get the address from wallet
            // For now, use a placeholder
            address!("0000000000000000000000000000000000000001")
        } else {
            return Err(anyhow::anyhow!("No wallet configured"));
        };
        
        // Set deadline to 20 minutes from now
        let deadline = U256::from(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs() + 1200);
        
        // Encode the function call
        // First, the function selector
        let mut encoded = keccak256("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)")[..4].to_vec();
        
        // Then encode the parameters
        // For dynamic arrays, we need to encode: offset, array length, array elements
        
        // Encode amountIn (uint256)
        encoded.extend_from_slice(&amount_in.to_be_bytes::<32>());
        
        // Encode amountOutMin (uint256)
        encoded.extend_from_slice(&amount_out_min.to_be_bytes::<32>());
        
        // Encode offset for path array (dynamic, so we put offset to where data starts)
        // 5 parameters * 32 bytes = 160, so path data starts at position 160
        encoded.extend_from_slice(&U256::from(160).to_be_bytes::<32>());
        
        // Encode 'to' address
        encoded.extend_from_slice(&[0u8; 12]); // Padding for address
        encoded.extend_from_slice(to.as_slice());
        
        // Encode deadline (uint256)
        encoded.extend_from_slice(&deadline.to_be_bytes::<32>());
        
        // Now encode the path array at offset 160
        // First, the array length
        encoded.extend_from_slice(&U256::from(path.len()).to_be_bytes::<32>());
        
        // Then each address in the path
        for addr in path {
            encoded.extend_from_slice(&[0u8; 12]); // Padding for address
            encoded.extend_from_slice(addr.as_slice());
        }
        
        info!("📝 Encoded swap data:");
        info!("   Function: swapExactTokensForTokens");
        info!("   Amount In: {} ETH", opportunity.size_eth);
        info!("   Min Amount Out: {} USDC (with {:.1}% slippage)", 
            expected_out * slippage_factor, 
            Decimal::from(CONFIG.slippage_tolerance_bps) / dec!(100)
        );
        info!("   Path: {} -> {}", 
            if opportunity.direction.contains("Buy on Aerodrome") { "USDC" } else { "WETH" },
            if opportunity.direction.contains("Buy on Aerodrome") { "WETH" } else { "USDC" }
        );
        info!("   Deadline: {} seconds from now", 1200);
        
        Ok(encoded)
    }

    async fn create_failed_execution(
        &self,
        execution_id: String,
        opportunity: &ArbitrageOpportunity,
        start_time: Instant,
        error: String,
    ) -> Result<TradeExecution> {
        Ok(TradeExecution {
            id: execution_id,
            opportunity_id: opportunity.id.clone(),
            timestamp: Utc::now(),
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

// Enhanced Market Making Structures
#[derive(Debug, Clone, Serialize)]
struct MarketMakingSignal {
    id: String,
    timestamp: DateTime<Utc>,
    pool: String,
    fair_value_price: Decimal,
    current_pool_price: Decimal,
    target_bid_price: Decimal,
    target_ask_price: Decimal,
    effective_spread_bps: u32,
    position_size_eth: Decimal,
    inventory_analysis: InventoryAnalysis,
    market_conditions: MarketConditions,
    strategy: LiquidityStrategy,
    risk_metrics: RiskMetrics,
    volatility_metrics: VolatilityMetrics, // NEW
    execution_priority: ExecutionPriority,
    rationale: String,
}

#[derive(Debug, Clone, Serialize)]
struct InventoryAnalysis {
    current_weth_balance: Decimal,
    current_usd_balance: Decimal,
    total_value_usd: Decimal,
    weth_ratio: Decimal,
    target_weth_ratio: Decimal,
    imbalance_severity: InventoryImbalance,
    rebalance_needed: bool,
    rebalance_amount_eth: Decimal,
}

#[derive(Debug, Clone, Serialize)]
enum InventoryImbalance {
    Balanced,
    SlightlyLong,
    SlightlyShort,
    SignificantlyLong,
    SignificantlyShort,
    CriticallyImbalanced,
}

#[derive(Debug, Clone, Serialize)]
struct MarketConditions {
    price_volatility_1h: Decimal,
    liquidity_depth: LiquidityDepth,
    spread_environment: SpreadEnvironment,
    market_trend: MarketTrend,
    volume_profile: VolumeProfile,
}

#[derive(Debug, Clone, Serialize)]
enum MarketTrend {
    Bullish,
    Bearish,
    Sideways,
}

#[derive(Debug, Clone, Serialize)]
enum SpreadEnvironment {
    Tight,
    Normal,
    Wide,
    VeryWide,
}

#[derive(Debug, Clone, Serialize)]
enum VolumeProfile {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Serialize)]
struct LiquidityDepth {
    total_liquidity_usd: Decimal,
    weth_reserves: Decimal,
    usd_reserves: Decimal,
    depth_quality: DepthQuality,
}

#[derive(Debug, Clone, Serialize)]
enum DepthQuality {
    Excellent,
    Good,
    Fair,
    Poor,
}

#[derive(Debug, Clone, Serialize)]
struct LiquidityStrategy {
    strategy_type: StrategyType,
    bid_size_eth: Decimal,
    ask_size_eth: Decimal,
    range_bounds: RangeBounds,
    duration_estimate: Duration,
    expected_daily_volume: Decimal,
    risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize)]
enum StrategyType {
    TightSpread,
    WideSpread,
    InventoryManagement,
    TrendFollowing,
    VolatilityAdaptive,
}

#[derive(Debug, Clone, Serialize)]
struct RangeBounds {
    lower_bound: Decimal,
    upper_bound: Decimal,
    confidence_interval: Decimal,
}

#[derive(Debug, Clone, Serialize)]
enum RiskLevel {
    Conservative,
    Moderate,
    Aggressive,
    Speculative,
}

#[derive(Debug, Clone, Serialize)]
struct RiskMetrics {
    max_drawdown_usd: Decimal,
    value_at_risk_1d: Decimal,
    inventory_risk_score: Decimal,
    liquidity_risk_score: Decimal,
    volatility_risk_score: Decimal, // NEW
    overall_risk_score: Decimal,
    recommended_max_exposure: Decimal,
}

#[derive(Debug, Clone, Serialize)]
enum ExecutionPriority {
    Immediate,
    High,
    Medium,
    Low,
    Hold,
}

// Enhanced Volatility Calculator
pub struct VolatilityCalculator {
    window: VecDeque<(SystemTime, f64)>,
    max_duration: Duration,
}

impl VolatilityCalculator {
    pub fn new(max_duration_secs: u64) -> Self {
        VolatilityCalculator {
            window: VecDeque::new(),
            max_duration: Duration::from_secs(max_duration_secs),
        }
    }

    pub fn add_value(&mut self, price: f64) {
        let now = SystemTime::now();
        self.window.push_back((now, price));

        while let Some((timestamp, _)) = self.window.front() {
            if let Ok(duration) = now.duration_since(*timestamp) {
                if duration > self.max_duration {
                    self.window.pop_front();
                } else {
                    break;
                }
            } else {
                warn!("Encountered a timestamp in the future: {:?}", timestamp);
                self.window.pop_front();
            }
        }
    }

    pub fn calculate_volatility(&self) -> Option<f64> {
        if self.window.len() < 10 {
            return None;
        }

        let prices: Vec<f64> = self.window.iter().map(|(_, price)| *price).collect();
        let mean: f64 = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance: f64 = prices.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / prices.len() as f64;

        Some(variance.sqrt())
    }

    pub fn calculate_volatility_percentage(&self) -> Option<f64> {
        if let Some(volatility) = self.calculate_volatility() {
            if self.window.len() < 10 {
                return None;
            }
            
            let prices: Vec<f64> = self.window.iter().map(|(_, price)| *price).collect();
            let mean: f64 = prices.iter().sum::<f64>() / prices.len() as f64;
            
            if mean > 0.0 {
                Some((volatility / mean) * 100.0)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn sample_count(&self) -> usize {
        self.window.len()
    }

    pub fn window_duration(&self) -> Option<Duration> {
        if self.window.len() < 2 {
            return None;
        }
        
        if let (Some((first_time, _)), Some((last_time, _))) = (self.window.front(), self.window.back()) {
            last_time.duration_since(*first_time).ok()
        } else {
            None
        }
    }
}

// Custom error types
#[derive(Error, Debug)]
pub enum BotError {
    #[error("Network error: {message}")]
    Network {
        message: String,
        #[source]
        source: Option<anyhow::Error>,
        retry_count: u32,
    },
    
    #[error("Contract interaction failed: {contract} - {message}")]
    Contract {
        contract: Address,
        message: String,
        #[source]
        source: anyhow::Error,
    },
    
    #[error("Price validation failed: {source} price ${price} is invalid - {reason}")]
    PriceValidation {
        source: Box<dyn Error>,
        price: Decimal,
        reason: String,
    },
    
    #[error("Insufficient liquidity: {pool} - {details}")]
    InsufficientLiquidity {
        pool: String,
        details: String,
    },
    
    #[error("Data parsing error: {context}")]
    DataParsing {
        context: String,
        #[source]
        source: anyhow::Error,
    },
    
    #[error("Circuit breaker active: {reason}")]
    CircuitBreakerOpen {
        reason: String,
        cooldown_remaining: Duration,
    },
}

pub type BotResult<T> = Result<T, BotError>;

// Enhanced retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    max_attempts: u32,
    initial_delay_ms: u64,
    max_delay_ms: u64,
    exponential_base: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 5000,
            exponential_base: 2.0,
        }
    }
}

pub async fn retry_with_backoff<F, Fut, T>(
    operation: F,
    config: &RetryConfig,
    context: &str,
) -> BotResult<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, anyhow::Error>>,
{
    let mut attempt = 0;
    let mut delay = config.initial_delay_ms;
    
    loop {
        attempt += 1;
        
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt >= config.max_attempts => {
                return Err(BotError::Network {
                    message: format!("{} failed after {} attempts", context, attempt),
                    source: Some(e),
                    retry_count: attempt,
                });
            }
            Err(e) => {
                warn!(
                    "Attempt {}/{} failed for {}: {}. Retrying in {}ms...",
                    attempt, config.max_attempts, context, e, delay
                );
                
                tokio::time::sleep(Duration::from_millis(delay)).await;
                
                delay = (delay as f64 * config.exponential_base) as u64;
                delay = delay.min(config.max_delay_ms);
                let jitter = (delay as f64 * 0.1 * (rand::random::<f64>() - 0.5)) as u64;
                delay = delay.saturating_add(jitter);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ArbitrageOpportunity {
    id: String,
    timestamp: DateTime<Utc>,
    pool: String,
    direction: String,
    dex_price: Decimal,
    cex_price: Decimal,
    price_diff_pct: Decimal,
    size_eth: Decimal,
    gross_profit_usd: Decimal,
    gas_cost_usd: Decimal,
    net_profit_usd: Decimal,
    roi_pct: Decimal,
    validation_checks: ValidationResult,
    volatility_assessment: Option<VolatilityMetrics>, // NEW
    execution_simulation: Option<TradeExecution>,      // NEW
}

#[derive(Debug, Clone, Serialize, Default)]
struct ValidationResult {
    price_sanity: bool,
    liquidity_check: bool,
    gas_economics: bool,
    slippage_acceptable: bool,
    volatility_acceptable: bool, // NEW
    all_passed: bool,
    warnings: Vec<String>,
}

#[derive(Clone)]
struct PoolInfo {
    address: Address,
    name: String,
    token0: Address,
    token1: Address,
    #[allow(dead_code)]
    is_stable: bool,
    #[allow(dead_code)]
    min_liquidity: Decimal,
    #[allow(dead_code)]
    last_update: Instant,
}

#[derive(Debug, Clone)]
struct HealthStatus {
    dex_connection: bool,
    cex_connection: bool,
    #[allow(dead_code)]
    last_dex_update: Option<Instant>,
    #[allow(dead_code)]
    last_cex_update: Option<Instant>,
    consecutive_errors: u32,
    #[allow(dead_code)]
    circuit_breaker_active: bool,
    uptime_seconds: u64,
}

pub struct LoggingGuard {
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

struct CircuitBreaker {
    consecutive_errors: Arc<RwLock<u32>>,
    is_open: Arc<RwLock<bool>>,
    last_error_time: Arc<RwLock<Option<Instant>>>,
    cooldown_duration: Duration,
}

impl CircuitBreaker {
    fn new(cooldown_secs: u64) -> Self {
        Self {
            consecutive_errors: Arc::new(RwLock::new(0)),
            is_open: Arc::new(RwLock::new(false)),
            last_error_time: Arc::new(RwLock::new(None)),
            cooldown_duration: Duration::from_secs(cooldown_secs),
        }
    }

    async fn record_success(&self) {
        *self.consecutive_errors.write().await = 0;
        *self.is_open.write().await = false;
    }

    async fn record_error(&self) -> bool {
        let mut errors = self.consecutive_errors.write().await;
        *errors += 1;
        
        if *errors >= CONFIG.max_consecutive_errors {
            *self.is_open.write().await = true;
            *self.last_error_time.write().await = Some(Instant::now());
            error!("Circuit breaker OPEN after {} consecutive errors", *errors);
            return true;
        }
        false
    }

    async fn can_proceed(&self) -> bool {
        let is_open = *self.is_open.read().await;
        if !is_open {
            return true;
        }

        if let Some(last_error) = *self.last_error_time.read().await {
            if last_error.elapsed() > self.cooldown_duration {
                info!("Circuit breaker cooldown complete, resetting");
                *self.is_open.write().await = false;
                *self.consecutive_errors.write().await = 0;
                return true;
            }
        }
        false
    }
}

pub struct ErrorRecovery {
    error_counts: Arc<RwLock<HashMap<String, u32>>>,
    recovery_strategies: HashMap<String, RecoveryStrategy>,
}

#[derive(Clone)]
enum RecoveryStrategy {
    Retry { max_attempts: u32, delay_ms: u64 },
    Fallback { alternative_source: String },
    Skip { log_level: tracing::Level },
    #[allow(dead_code)]
    Shutdown { reason: String },
}

impl ErrorRecovery {
    pub fn new() -> Self {
        let mut strategies = HashMap::new();
        
        strategies.insert(
            "network_timeout".to_string(),
            RecoveryStrategy::Retry {
                max_attempts: 5,
                delay_ms: 1000,
            },
        );
        
        strategies.insert(
            "invalid_price".to_string(),
            RecoveryStrategy::Skip {
                log_level: tracing::Level::WARN,
            },
        );
        
        strategies.insert(
            "contract_error".to_string(),
            RecoveryStrategy::Fallback {
                alternative_source: "backup_pool".to_string(),
            },
        );
        
        Self {
            error_counts: Arc::new(RwLock::new(HashMap::new())),
            recovery_strategies: strategies,
        }
    }
    
    pub async fn handle_error(&self, error: &BotError, _context: &str) -> RecoveryAction {
        let error_type = self.classify_error(error);
        let mut counts = self.error_counts.write().await;
        let count = counts.entry(error_type.clone()).or_insert(0);
        *count += 1;
        
        match self.recovery_strategies.get(&error_type) {
            Some(RecoveryStrategy::Retry { max_attempts, delay_ms }) => {
                if *count <= *max_attempts {
                    RecoveryAction::Retry {
                        delay: Duration::from_millis(*delay_ms),
                    }
                } else {
                    RecoveryAction::Escalate
                }
            }
            Some(RecoveryStrategy::Fallback { alternative_source }) => {
                RecoveryAction::Fallback {
                    source: alternative_source.clone(),
                }
            }
            Some(RecoveryStrategy::Skip { log_level }) => {
                RecoveryAction::Skip {
                    log_level: *log_level,
                }
            }
            Some(RecoveryStrategy::Shutdown { reason }) => {
                RecoveryAction::Shutdown {
                    reason: reason.clone(),
                }
            }
            None => RecoveryAction::Escalate,
        }
    }
    
    fn classify_error(&self, error: &BotError) -> String {
        match error {
            BotError::Network { .. } => "network_timeout".to_string(),
            BotError::PriceValidation { .. } => "invalid_price".to_string(),
            BotError::Contract { .. } => "contract_error".to_string(),
            BotError::InsufficientLiquidity { .. } => "low_liquidity".to_string(),
            BotError::DataParsing { .. } => "parse_error".to_string(),
            BotError::CircuitBreakerOpen { .. } => "circuit_breaker".to_string(),
        }
    }
}

#[derive(Debug)]
pub enum RecoveryAction {
    Retry { delay: Duration },
    Fallback { source: String },
    Skip { log_level: tracing::Level },
    Escalate,
    Shutdown { reason: String },
}

// Enhanced Market Making Engine with Volatility
struct MarketMakingEngine {
    volatility_calculator: Arc<RwLock<MultiTimeframeVolatilityCalculator>>,
    last_signals: Arc<RwLock<HashMap<String, MarketMakingSignal>>>,
}

impl MarketMakingEngine {
    fn new() -> Self {
        Self {
            volatility_calculator: Arc::new(RwLock::new(MultiTimeframeVolatilityCalculator::new())),
            last_signals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn update_price_history(&self, price: Decimal) {
        self.volatility_calculator.write().await.add_price(price).await;
    }

    async fn get_volatility_metrics(&self) -> VolatilityMetrics {
        self.volatility_calculator.read().await.get_volatility_metrics().await
    }

    async fn generate_market_making_signal(
        &self,
        pool_info: &PoolInfo,
        fair_value_price: Decimal,
        current_pool_price: Decimal,
        liquidity_depth: LiquidityDepth,
        _provider: &dyn Provider,
    ) -> Result<MarketMakingSignal> {
        let signal_id = uuid::Uuid::new_v4().to_string();
        let timestamp = Utc::now();

        // Get volatility metrics
        let volatility_metrics = self.get_volatility_metrics().await;

        debug!(
            "Volatility analysis: Short={:.2}%, Medium={:.2}%, Long={:.2}%, Trend={:?}",
            volatility_metrics.short_term_volatility,
            volatility_metrics.medium_term_volatility,
            volatility_metrics.long_term_volatility,
            volatility_metrics.volatility_trend
        );

        // Use long-term volatility for market conditions
        let market_conditions = self.analyze_market_conditions(
            volatility_metrics.long_term_volatility,
            &liquidity_depth,
            current_pool_price,
            fair_value_price,
        ).await;

        let inventory_analysis = self.analyze_inventory_simulation(
            fair_value_price,
            &liquidity_depth,
        ).await;

        // Calculate spread with volatility adjustments
        let effective_spread_bps = self.calculate_dynamic_spread_with_volatility(
            &market_conditions,
            &inventory_analysis,
            &volatility_metrics,
            fair_value_price,
        ).await;

        let spread_decimal = Decimal::from(effective_spread_bps) / dec!(10000);
        let half_spread = fair_value_price * spread_decimal / dec!(2);
        
        let target_bid_price = fair_value_price - half_spread;
        let target_ask_price = fair_value_price + half_spread;

        // Adjust position size based on volatility
        let position_size_eth = self.calculate_position_size_with_volatility(
            &market_conditions,
            &inventory_analysis,
            &liquidity_depth,
            &volatility_metrics,
        ).await;

        let strategy = self.select_liquidity_strategy(
            &market_conditions,
            &inventory_analysis,
            current_pool_price,
            fair_value_price,
        ).await;

        // Enhanced risk metrics with volatility
        let risk_metrics = self.calculate_risk_metrics_with_volatility(
            position_size_eth,
            fair_value_price,
            &volatility_metrics,
            &liquidity_depth,
        ).await;

        let execution_priority = self.determine_execution_priority_with_volatility(
            &market_conditions,
            &inventory_analysis,
            &risk_metrics,
            &volatility_metrics,
            current_pool_price,
            fair_value_price,
        ).await;

        let rationale = self.generate_strategy_rationale_with_volatility(
            &market_conditions,
            &inventory_analysis,
            &strategy,
            &volatility_metrics,
            effective_spread_bps,
            fair_value_price,
            current_pool_price,
        ).await;

        let signal = MarketMakingSignal {
            id: signal_id,
            timestamp,
            pool: pool_info.name.clone(),
            fair_value_price,
            current_pool_price,
            target_bid_price,
            target_ask_price,
            effective_spread_bps,
            position_size_eth,
            inventory_analysis,
            market_conditions,
            strategy,
            risk_metrics,
            volatility_metrics,
            execution_priority,
            rationale,
        };

        let mut last_signals = self.last_signals.write().await;
        last_signals.insert(pool_info.name.clone(), signal.clone());

        Ok(signal)
    }

    async fn analyze_market_conditions(
        &self,
        volatility: Decimal,
        liquidity_depth: &LiquidityDepth,
        current_price: Decimal,
        fair_value: Decimal,
    ) -> MarketConditions {
        let price_diff_pct = ((current_price - fair_value).abs() / fair_value) * dec!(100);
        let spread_environment = match price_diff_pct {
            diff if diff < dec!(0.2) => SpreadEnvironment::Tight,
            diff if diff < dec!(0.5) => SpreadEnvironment::Normal,
            diff if diff < dec!(1.0) => SpreadEnvironment::Wide,
            _ => SpreadEnvironment::VeryWide,
        };

        let market_trend = if current_price > fair_value * dec!(1.002) {
            MarketTrend::Bullish
        } else if current_price < fair_value * dec!(0.998) {
            MarketTrend::Bearish
        } else {
            MarketTrend::Sideways
        };

        let volume_profile = match liquidity_depth.total_liquidity_usd {
            liq if liq > dec!(1_000_000) => VolumeProfile::High,
            liq if liq > dec!(100_000) => VolumeProfile::Normal,
            _ => VolumeProfile::Low,
        };

        MarketConditions {
            price_volatility_1h: volatility,
            liquidity_depth: liquidity_depth.clone(),
            spread_environment,
            market_trend,
            volume_profile,
        }
    }

    async fn analyze_inventory_simulation(
        &self,
        fair_value_price: Decimal,
        liquidity_depth: &LiquidityDepth,
    ) -> InventoryAnalysis {
        let current_weth_balance = CONFIG.max_position_size_eth * dec!(0.4);
        let current_usd_balance = CONFIG.max_position_size_eth * fair_value_price * dec!(0.6);

        let max_feasible_position = liquidity_depth.weth_reserves * dec!(0.1);
        let adjusted_weth_balance = current_weth_balance.min(max_feasible_position);

        let total_value_usd = (adjusted_weth_balance * fair_value_price) + current_usd_balance;
        let weth_ratio = (adjusted_weth_balance * fair_value_price) / total_value_usd;
        let target_weth_ratio = CONFIG.inventory_target_ratio;

        let ratio_diff = (weth_ratio - target_weth_ratio).abs();
        let imbalance_severity = match ratio_diff {
            diff if diff < dec!(0.05) => InventoryImbalance::Balanced,
            diff if diff < dec!(0.15) => {
                if weth_ratio > target_weth_ratio {
                    InventoryImbalance::SlightlyLong
                } else {
                    InventoryImbalance::SlightlyShort
                }
            }
            diff if diff < dec!(0.30) => {
                if weth_ratio > target_weth_ratio {
                    InventoryImbalance::SignificantlyLong
                } else {
                    InventoryImbalance::SignificantlyShort
                }
            }
            _ => InventoryImbalance::CriticallyImbalanced,
        };

        let rebalance_needed = ratio_diff > REBALANCE_THRESHOLD;
        let rebalance_amount_eth = if rebalance_needed {
            (target_weth_ratio - weth_ratio) * total_value_usd / fair_value_price
        } else {
            dec!(0)
        };

        InventoryAnalysis {
            current_weth_balance: adjusted_weth_balance,
            current_usd_balance,
            total_value_usd,
            weth_ratio,
            target_weth_ratio,
            imbalance_severity,
            rebalance_needed,
            rebalance_amount_eth,
        }
    }

    async fn calculate_dynamic_spread_with_volatility(
        &self,
        _market_conditions: &MarketConditions,
        inventory_analysis: &InventoryAnalysis,
        volatility_metrics: &VolatilityMetrics,
        fair_value_price: Decimal,
    ) -> u32 {
        let mut spread_bps = CONFIG.base_spread_bps;

        // Apply volatility adjustments based on multi-timeframe analysis
        spread_bps = (Decimal::from(spread_bps) * volatility_metrics.recommended_adjustments.spread_multiplier)
            .to_u32()
            .unwrap_or(spread_bps);

        // Additional adjustments for volatility trend
        match volatility_metrics.volatility_trend {
            VolatilityTrend::Increasing => spread_bps = (spread_bps as f64 * 1.2) as u32,
            VolatilityTrend::Volatile => spread_bps = (spread_bps as f64 * 1.3) as u32,
            _ => {}
        }

        // Price-based adjustments
        if fair_value_price > dec!(5000) || fair_value_price < dec!(1000) {
            spread_bps = (spread_bps as f64 * 1.2) as u32;
        }

        // Inventory adjustments
        match inventory_analysis.imbalance_severity {
            InventoryImbalance::Balanced => {},
            InventoryImbalance::SlightlyLong | InventoryImbalance::SlightlyShort => {
                spread_bps = (spread_bps as f64 * 1.1) as u32;
            },
            InventoryImbalance::SignificantlyLong | InventoryImbalance::SignificantlyShort => {
                spread_bps = (spread_bps as f64 * 1.25) as u32;
            },
            InventoryImbalance::CriticallyImbalanced => {
                spread_bps = (spread_bps as f64 * 1.5) as u32;
            },
        }

        // Liquidity depth adjustments
        match _market_conditions.liquidity_depth.depth_quality {
            DepthQuality::Excellent => {},
            DepthQuality::Good => spread_bps = (spread_bps as f64 * 1.05) as u32,
            DepthQuality::Fair => spread_bps = (spread_bps as f64 * 1.15) as u32,
            DepthQuality::Poor => spread_bps = (spread_bps as f64 * 1.3) as u32,
        }

        // Spread environment adjustments
        match _market_conditions.spread_environment {
            SpreadEnvironment::Tight => spread_bps = (spread_bps as f64 * 0.8) as u32,
            SpreadEnvironment::Normal => {},
            SpreadEnvironment::Wide => spread_bps = (spread_bps as f64 * 1.2) as u32,
            SpreadEnvironment::VeryWide => spread_bps = (spread_bps as f64 * 1.5) as u32,
        }

        spread_bps.max(MIN_SPREAD_BPS).min(MAX_SPREAD_BPS)
    }

    async fn calculate_position_size_with_volatility(
        &self,
        _market_conditions: &MarketConditions,
        inventory_analysis: &InventoryAnalysis,
        liquidity_depth: &LiquidityDepth,
        volatility_metrics: &VolatilityMetrics,
    ) -> Decimal {
        let mut base_size = CONFIG.max_position_size_eth * dec!(0.1);

        // Apply volatility-based position sizing
        base_size *= volatility_metrics.recommended_adjustments.position_size_factor;

        // Additional adjustments for extreme volatility
        match volatility_metrics.impact_assessment {
            VolatilityImpact::Extreme => base_size *= dec!(0.5),
            VolatilityImpact::High if matches!(volatility_metrics.volatility_trend, VolatilityTrend::Increasing) => {
                base_size *= dec!(0.7)
            },
            _ => {}
        }

        // Pool impact check
        let pool_impact = base_size / liquidity_depth.weth_reserves;
        if pool_impact > dec!(0.01) {
            base_size = liquidity_depth.weth_reserves * dec!(0.005);
        }

        // Inventory adjustments
        match inventory_analysis.imbalance_severity {
            InventoryImbalance::CriticallyImbalanced => base_size *= dec!(0.3),
            InventoryImbalance::SignificantlyLong | InventoryImbalance::SignificantlyShort => base_size *= dec!(0.7),
            _ => {},
        }

        base_size.max(MIN_TRADE_SIZE_ETH).min(CONFIG.max_position_size_eth)
    }

    async fn select_liquidity_strategy(
        &self,
        market_conditions: &MarketConditions,
        inventory_analysis: &InventoryAnalysis,
        current_price: Decimal,
        fair_value: Decimal,
    ) -> LiquidityStrategy {
        let price_deviation = ((current_price - fair_value).abs() / fair_value) * dec!(100);

        let strategy_type = match (
            &market_conditions.price_volatility_1h,
            &inventory_analysis.imbalance_severity,
            &market_conditions.spread_environment,
        ) {
            (vol, _, _) if *vol > dec!(15) => StrategyType::VolatilityAdaptive,
            (_, InventoryImbalance::SignificantlyLong | InventoryImbalance::SignificantlyShort, _) => 
                StrategyType::InventoryManagement,
            (_, _, SpreadEnvironment::Tight) if price_deviation < dec!(0.1) => 
                StrategyType::TightSpread,
            (_, _, SpreadEnvironment::Wide | SpreadEnvironment::VeryWide) => 
                StrategyType::WideSpread,
            _ => StrategyType::TrendFollowing,
        };

        let base_size = CONFIG.max_position_size_eth * dec!(0.1);
        
        let (bid_size_eth, ask_size_eth) = match (&strategy_type, &inventory_analysis.imbalance_severity) {
            (StrategyType::InventoryManagement, InventoryImbalance::SignificantlyLong) => 
                (base_size * dec!(0.3), base_size * dec!(1.5)),
            (StrategyType::InventoryManagement, InventoryImbalance::SignificantlyShort) => 
                (base_size * dec!(1.5), base_size * dec!(0.3)),
            _ => (base_size, base_size),
        };

        let range_bounds = RangeBounds {
            lower_bound: fair_value * dec!(0.95),
            upper_bound: fair_value * dec!(1.05),
            confidence_interval: dec!(0.95),
        };

        let duration_estimate = match strategy_type {
            StrategyType::TightSpread => Duration::from_secs(300),
            StrategyType::WideSpread => Duration::from_secs(3600),
            StrategyType::InventoryManagement => Duration::from_secs(1800),
            StrategyType::TrendFollowing => Duration::from_secs(7200),
            StrategyType::VolatilityAdaptive => Duration::from_secs(600),
        };

        let risk_level = match market_conditions.price_volatility_1h {
            vol if vol > dec!(20) => RiskLevel::Speculative,
            vol if vol > dec!(10) => RiskLevel::Aggressive,
            vol if vol > dec!(5) => RiskLevel::Moderate,
            _ => RiskLevel::Conservative,
        };

        LiquidityStrategy {
            strategy_type,
            bid_size_eth,
            ask_size_eth,
            range_bounds,
            duration_estimate,
            expected_daily_volume: base_size * dec!(10),
            risk_level,
        }
    }

    async fn calculate_risk_metrics_with_volatility(
        &self,
        position_size: Decimal,
        fair_value: Decimal,
        volatility_metrics: &VolatilityMetrics,
        liquidity_depth: &LiquidityDepth,
    ) -> RiskMetrics {
        let position_value = position_size * fair_value;
        
        // Use short-term volatility for VaR calculation
        let daily_volatility = volatility_metrics.short_term_volatility * dec!(4.899);
        let value_at_risk_1d = position_value * daily_volatility / dec!(100) * dec!(1.65);

        let max_drawdown_usd = position_value * dec!(0.1);

        let inventory_risk_score = (position_size / CONFIG.max_position_size_eth * dec!(100))
            .min(dec!(100));

        let liquidity_risk_score = match liquidity_depth.depth_quality {
            DepthQuality::Excellent => dec!(10),
            DepthQuality::Good => dec!(25),
            DepthQuality::Fair => dec!(50),
            DepthQuality::Poor => dec!(80),
        };

        // Calculate volatility risk score
        let volatility_risk_score = match volatility_metrics.impact_assessment {
            VolatilityImpact::Low => dec!(10),
            VolatilityImpact::Moderate => dec!(30),
            VolatilityImpact::High => dec!(60),
            VolatilityImpact::Extreme => dec!(90),
        };

        // Weighted overall risk score
        let overall_risk_score = inventory_risk_score * dec!(0.3) + 
                                 liquidity_risk_score * dec!(0.25) + 
                                 volatility_risk_score * dec!(0.35) +
                                 volatility_metrics.short_term_volatility.min(dec!(50)) * dec!(0.1);

        let recommended_max_exposure = CONFIG.max_position_size_eth * 
            (dec!(100) - overall_risk_score) / dec!(100);

        RiskMetrics {
            max_drawdown_usd,
            value_at_risk_1d,
            inventory_risk_score,
            liquidity_risk_score,
            volatility_risk_score,
            overall_risk_score,
            recommended_max_exposure,
        }
    }

    async fn determine_execution_priority_with_volatility(
        &self,
        market_conditions: &MarketConditions,
        inventory_analysis: &InventoryAnalysis,
        risk_metrics: &RiskMetrics,
        volatility_metrics: &VolatilityMetrics,
        current_price: Decimal,
        fair_value: Decimal,
    ) -> ExecutionPriority {
        let price_deviation = ((current_price - fair_value).abs() / fair_value) * dec!(100);

        // Check volatility-based urgency first
        match volatility_metrics.recommended_adjustments.execution_urgency {
            ExecutionUrgency::Fast if price_deviation > dec!(0.5) => return ExecutionPriority::Immediate,
            ExecutionUrgency::Cautious => return ExecutionPriority::Low,
            _ => {}
        }

        if matches!(inventory_analysis.imbalance_severity, InventoryImbalance::CriticallyImbalanced) {
            return ExecutionPriority::Immediate;
        }

        if risk_metrics.overall_risk_score > dec!(80) {
            return ExecutionPriority::Hold;
        }

        if price_deviation > dec!(1.0) && matches!(volatility_metrics.impact_assessment, VolatilityImpact::Low | VolatilityImpact::Moderate) {
            return ExecutionPriority::Immediate;
        }

        if market_conditions.price_volatility_1h > dec!(15) {
            return ExecutionPriority::High;
        }

        if price_deviation > dec!(0.3) {
            return ExecutionPriority::High;
        }

        if price_deviation > dec!(0.1) {
            return ExecutionPriority::Medium;
        }

        ExecutionPriority::Low
    }

    async fn generate_strategy_rationale_with_volatility(
        &self,
        market_conditions: &MarketConditions,
        inventory_analysis: &InventoryAnalysis,
        strategy: &LiquidityStrategy,
        volatility_metrics: &VolatilityMetrics,
        spread_bps: u32,
        fair_value: Decimal,
        current_price: Decimal,
    ) -> String {
        let mut rationale = String::new();

        rationale.push_str(&format!(
            "Market Analysis: Current price ${:.4} vs fair value ${:.4} ({:+.2}% deviation). ",
            current_price, fair_value, 
            ((current_price - fair_value) / fair_value) * dec!(100)
        ));

        // Add volatility analysis
        rationale.push_str(&format!(
            "Volatility: Short={:.1}%, Medium={:.1}%, Long={:.1}% (Trend: {:?}, Impact: {:?}). ",
            volatility_metrics.short_term_volatility,
            volatility_metrics.medium_term_volatility,
            volatility_metrics.long_term_volatility,
            volatility_metrics.volatility_trend,
            volatility_metrics.impact_assessment
        ));

        rationale.push_str(&format!("Effective spread: {} bps. ", spread_bps));

        match strategy.strategy_type {
            StrategyType::TightSpread => {
                rationale.push_str("TIGHT SPREAD strategy selected due to stable conditions and tight current spreads. ");
            },
            StrategyType::WideSpread => {
                rationale.push_str("WIDE SPREAD strategy selected to capture larger price movements in volatile environment. ");
            },
            StrategyType::InventoryManagement => {
                rationale.push_str(&format!(
                    "INVENTORY MANAGEMENT strategy selected due to {:?} position. ",
                    inventory_analysis.imbalance_severity
                ));
            },
            StrategyType::TrendFollowing => {
                rationale.push_str(&format!(
                    "TREND FOLLOWING strategy selected based on {:?} market trend. ",
                    market_conditions.market_trend
                ));
            },
            StrategyType::VolatilityAdaptive => {
                rationale.push_str("VOLATILITY ADAPTIVE strategy selected due to high market volatility requiring frequent adjustments. ");
            },
        }

        // Volatility-based adjustments
        if matches!(volatility_metrics.impact_assessment, VolatilityImpact::High | VolatilityImpact::Extreme) {
            rationale.push_str(&format!(
                "High volatility detected - spreads widened by {:.1}x, position size reduced by {:.0}%. ",
                volatility_metrics.recommended_adjustments.spread_multiplier,
                (dec!(1) - volatility_metrics.recommended_adjustments.position_size_factor) * dec!(100)
            ));
        }

        match strategy.risk_level {
            RiskLevel::Conservative => rationale.push_str("Conservative sizing due to uncertain conditions. "),
            RiskLevel::Moderate => rationale.push_str("Moderate risk profile with balanced exposure. "),
            RiskLevel::Aggressive => rationale.push_str("Aggressive positioning to capitalize on clear opportunities. "),
            RiskLevel::Speculative => rationale.push_str("Speculative approach warranted by extreme conditions. "),
        }

        if inventory_analysis.rebalance_needed {
            rationale.push_str(&format!(
                "Portfolio rebalancing needed: {:.1}% WETH vs {:.1}% target. ",
                inventory_analysis.weth_ratio * dec!(100),
                inventory_analysis.target_weth_ratio * dec!(100)
            ));
        }

        rationale
    }
}

// Price validation
fn validate_price(price: Decimal, source: &str) -> Result<()> {
    if price <= dec!(0) {
        return Err(anyhow::anyhow!("{} price is zero or negative: {}", source, price));
    }
    
    if price < dec!(100) || price > dec!(100000) {
        return Err(anyhow::anyhow!("{} price out of reasonable range: ${}", source, price));
    }
    
    Ok(())
}

// Liquidity validation
fn validate_liquidity(weth_reserve: Decimal, usd_reserve: Decimal) -> Result<()> {
    const MIN_WETH_LIQUIDITY: Decimal = dec!(0.1);
    const MIN_USD_LIQUIDITY: Decimal = dec!(100);
    
    if weth_reserve < MIN_WETH_LIQUIDITY {
        return Err(anyhow::anyhow!("Insufficient WETH liquidity: {} WETH", weth_reserve));
    }
    
    if usd_reserve < MIN_USD_LIQUIDITY {
        return Err(anyhow::anyhow!("Insufficient USD liquidity: ${}", usd_reserve));
    }
    
    Ok(())
}

// Standard pool reserves fetching
async fn get_pool_reserves(provider: &dyn Provider, pool: Address) -> Result<(U256, U256)> {
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

// Enhanced pool reserves fetching with error handling
async fn get_pool_reserves_enhanced(
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

async fn get_pool_info_internal(
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
        min_liquidity: dec!(1000),
        last_update: Instant::now(),
    })
}

// Enhanced liquidity depth analysis
async fn analyze_liquidity_depth(
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

// Enhanced opportunity validation with volatility
async fn validate_opportunity_with_volatility(
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

// Safe price calculation with validation
async fn calculate_pool_price_safe(
    provider: &dyn Provider,
    pool_info: &PoolInfo,
) -> Result<Decimal> {
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

// Enhanced Binance price fetching
async fn get_binance_price_enhanced() -> BotResult<Decimal> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| BotError::Network {
            message: "Failed to build HTTP client".to_string(),
            source: Some(e.into()),
            retry_count: 0,
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
        return Err(BotError::PriceValidation {
            source: "Binance".to_string().into(),
            price,
            reason: "Price outside valid range".to_string(),
        });
    }
    
    Ok(price)
}

fn pow10(n: i32) -> Decimal {
    match n {
        0 => dec!(1),
        6 => dec!(1_000_000),
        18 => dec!(1_000_000_000_000_000_000),
        _ => {
            let mut result = dec!(1);
            if n > 0 {
                for _ in 0..n {
                    result *= dec!(10);
                }
            } else {
                for _ in 0..(-n) {
                    result /= dec!(10);
                }
            }
            result
        }
    }
}

// Health check endpoint
async fn run_health_check(
    dex_last_update: &Option<Instant>,
    cex_last_update: &Option<Instant>,
    circuit_breaker: &CircuitBreaker,
    start_time: Instant,
) -> HealthStatus {
    HealthStatus {
        dex_connection: dex_last_update
            .map(|t| t.elapsed().as_secs() < PRICE_STALENESS_SECONDS)
            .unwrap_or(false),
        cex_connection: cex_last_update
            .map(|t| t.elapsed().as_secs() < PRICE_STALENESS_SECONDS)
            .unwrap_or(false),
        last_dex_update: *dex_last_update,
        last_cex_update: *cex_last_update,
        consecutive_errors: *circuit_breaker.consecutive_errors.read().await,
        circuit_breaker_active: *circuit_breaker.is_open.read().await,
        uptime_seconds: start_time.elapsed().as_secs(),
    }
}

fn calculate_arbitrage(
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

fn save_opportunity(opp: &ArbitrageOpportunity) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    
    let filename = format!("output/opportunities/arbitrage_{}.jsonl", 
        Utc::now().format("%Y-%m-%d"));
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)?;
    
    writeln!(file, "{}", serde_json::to_string(opp)?)?;
    
    info!(
        opportunity_id = %opp.id,
        profit = %opp.net_profit_usd,
        validations_passed = opp.validation_checks.all_passed,
        "Saved validated arbitrage opportunity"
    );
    
    Ok(())
}

fn save_market_making_signal(signal: &MarketMakingSignal) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    
    let filename = format!("output/market_making/signals_{}.jsonl", 
        Utc::now().format("%Y-%m-%d"));
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)?;
    
    writeln!(file, "{}", serde_json::to_string(signal)?)?;
    
    info!(
        signal_id = %signal.id,
        strategy = ?signal.strategy.strategy_type,
        spread_bps = signal.effective_spread_bps,
        priority = ?signal.execution_priority,
        "Saved market making signal"
    );
    
    Ok(())
}

fn save_trade_execution(execution: &TradeExecution) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    
    let filename = format!("output/executions/trades_{}.jsonl", 
        Utc::now().format("%Y-%m-%d"));
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)?;
    
    writeln!(file, "{}", serde_json::to_string(execution)?)?;
    
    info!(
        execution_id = %execution.id,
        status = ?execution.status,
        actual_profit = ?execution.actual_profit_usd,
        "Saved trade execution"
    );
    
    Ok(())
}

fn setup_logging() -> Result<Arc<LoggingGuard>> {
    use tracing_subscriber::util::SubscriberInitExt;
    
    let file_appender = tracing_appender::rolling::hourly("output/logs", "aerodrome-bot.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_ansi(true)
                .with_level(true)
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_target(true)
                .with_thread_ids(false)
                .with_level(true)
                .with_ansi(false)
                .compact()
        )
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse()?)
        )
        .init();
    
    Ok(Arc::new(LoggingGuard { _guard: guard }))
}

fn setup_output_directories() -> Result<()> {
    use std::fs;
    
    fs::create_dir_all("output/logs")?;
    fs::create_dir_all("output/opportunities")?;
    fs::create_dir_all("output/reports")?;
    fs::create_dir_all("output/market_making")?;
    fs::create_dir_all("output/executions")?; // NEW
    
    Ok(())
}

// Helper function to validate pool with retry
async fn validate_pool_with_retry(
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

// Helper function to process a single pool safely
async fn process_pool_safe(
    provider: &Arc<ConcreteProvider>,
    pool: &PoolInfo,
    cex_price: Decimal,
    market_making_engine: &MarketMakingEngine,
    volatility_metrics: &VolatilityMetrics,
    _error_counts: &mut HashMap<String, u32>,
) -> BotResult<(Option<ArbitrageOpportunity>, Option<MarketMakingSignal>)> {
    let dex_price = calculate_pool_price_safe_with_retry(provider, pool).await?;
    
    let price_diff_pct = ((dex_price - cex_price).abs() / cex_price) * dec!(100);
    
    info!(
        "💹 {} | DEX: ${:.4} | CEX: ${:.4} | Diff: {:.3}% | Vol: {:.2}%",
        pool.name, dex_price, cex_price, price_diff_pct,
        volatility_metrics.short_term_volatility
    );
    
    market_making_engine.update_price_history(cex_price).await;
    
    let mut market_making_signal = None;
    
    let arbitrage_opportunity = calculate_arbitrage(
        &pool.name,
        dex_price,
        cex_price,
        CONFIG.trade_size_eth,
    );
    
    if CONFIG.enable_market_making {
        match analyze_liquidity_depth(provider.as_ref(), pool, cex_price).await {
            Ok(liquidity_depth) => {
                match market_making_engine.generate_market_making_signal(
                    pool,
                    cex_price,
                    dex_price,
                    liquidity_depth,
                    provider.as_ref(),
                ).await {
                    Ok(signal) => {
                        market_making_signal = Some(signal);
                    }
                    Err(e) => {
                        warn!("Failed to generate market making signal for {}: {}", pool.name, e);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to analyze liquidity depth for {}: {}", pool.name, e);
            }
        }
    }
    
    Ok((arbitrage_opportunity, market_making_signal))
}

// Enhanced pool price calculation with retry
async fn calculate_pool_price_safe_with_retry(
    provider: &Arc<ConcreteProvider>,
    pool_info: &PoolInfo,
) -> BotResult<Decimal> {
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
        message: format!("Failed to calculate pool price"),
        source: anyhow::anyhow!("{}", e),
    })
}

// Helper function to print session statistics
async fn print_session_stats(
    start_time: Instant,
    total_opportunities: u64,
    profitable_opportunities: u64,
    total_potential_profit: Decimal,
    total_market_making_signals: u64,
    total_executions: u64,
    successful_executions: u64,
    error_counts: &HashMap<String, u32>,
    circuit_breaker: &Arc<CircuitBreaker>,
) {
    let runtime = start_time.elapsed().as_secs() / 60;
    
    info!("\n📊 Session Statistics ({} minutes)", runtime);
    info!("   📈 ARBITRAGE:");
    info!("     Total opportunities: {}", total_opportunities);
    info!("     Profitable (validated): {}", profitable_opportunities);
    info!("     Success rate: {:.1}%", 
        if total_opportunities > 0 {
            (profitable_opportunities as f64 / total_opportunities as f64) * 100.0
        } else {
            0.0
        }
    );
    info!("     Total potential profit: ${:.2}", total_potential_profit);
    
    info!("   🎯 MARKET MAKING:");
    info!("     Total signals generated: {}", total_market_making_signals);
    info!("     Signals per hour: {:.1}", 
        if runtime > 0 {
            total_market_making_signals as f64 * 60.0 / runtime as f64
        } else {
            0.0
        }
    );
    
    info!("   🚀 TRADE EXECUTION:");
    info!("     Total executions: {}", total_executions);
    info!("     Successful: {}", successful_executions);
    info!("     Success rate: {:.1}%",
        if total_executions > 0 {
            (successful_executions as f64 / total_executions as f64) * 100.0
        } else {
            0.0
        }
    );
    
    info!("   ⚙️  SYSTEM:");
    info!("     Circuit breaker: {}", 
        if *circuit_breaker.is_open.read().await { "OPEN" } else { "CLOSED" }
    );
    
    if !error_counts.is_empty() {
        info!("     Error summary:");
        for (error_type, count) in error_counts.iter() {
            info!("       {}: {}", error_type, count);
        }
    }
    
    info!("");
}

// Print detailed market making signal
fn print_market_making_signal(signal: &MarketMakingSignal) {
    warn!("\n🎯 MARKET MAKING SIGNAL #{}", signal.id);
    warn!("📍 Pool: {}", signal.pool);
    warn!("💰 Price Analysis:");
    warn!("   Fair Value (CEX): ${:.4}", signal.fair_value_price);
    warn!("   Current Pool:     ${:.4}", signal.current_pool_price);
    warn!("   Target Bid:       ${:.4}", signal.target_bid_price);
    warn!("   Target Ask:       ${:.4}", signal.target_ask_price);
    warn!("   Effective Spread: {} bps ({:.3}%)", 
        signal.effective_spread_bps, 
        Decimal::from(signal.effective_spread_bps) / dec!(100)
    );
    
    warn!("📊 Volatility Analysis:");
    warn!("   Short-term:  {:.2}%", signal.volatility_metrics.short_term_volatility);
    warn!("   Medium-term: {:.2}%", signal.volatility_metrics.medium_term_volatility);
    warn!("   Long-term:   {:.2}%", signal.volatility_metrics.long_term_volatility);
    warn!("   Trend: {:?}, Impact: {:?}", 
        signal.volatility_metrics.volatility_trend,
        signal.volatility_metrics.impact_assessment
    );
    
    warn!("📋 Strategy: {:?}", signal.strategy.strategy_type);
    warn!("   Bid Size: {:.4} ETH", signal.strategy.bid_size_eth);
    warn!("   Ask Size: {:.4} ETH", signal.strategy.ask_size_eth);
    warn!("   Risk Level: {:?}", signal.strategy.risk_level);
    warn!("   Duration Est: {}min", signal.strategy.duration_estimate.as_secs() / 60);
    
    warn!("⚠️  Risk Assessment:");
    warn!("   Overall Risk Score: {:.1}/100", signal.risk_metrics.overall_risk_score);
    warn!("   Volatility Risk: {:.1}/100", signal.risk_metrics.volatility_risk_score);
    warn!("   Max Recommended Exposure: {:.4} ETH", signal.risk_metrics.recommended_max_exposure);
    warn!("   1-Day VaR: ${:.2}", signal.risk_metrics.value_at_risk_1d);
    
    warn!("🚨 Execution Priority: {:?}", signal.execution_priority);
    warn!("📝 Strategy Rationale:");
    warn!("   {}", signal.rationale);
    warn!("");
}

// Print trade execution details
fn print_trade_execution(execution: &TradeExecution) {
    match execution.status {
        ExecutionStatus::Success | ExecutionStatus::Simulated => {
            warn!("\n✅ TRADE EXECUTION #{}", execution.id);
            warn!("📍 Network: {}", execution.network);
            warn!("💰 Execution Details:");
            warn!("   Type: {:?}", execution.trade_type);
            warn!("   Status: {:?}", execution.status);
            if let Some(tx_hash) = &execution.tx_hash {
                warn!("   Tx Hash: {}", tx_hash);
            }
            warn!("   Expected Profit: ${:.2}", execution.expected_profit_usd);
            if let Some(actual_profit) = execution.actual_profit_usd {
                warn!("   Actual Profit: ${:.2}", actual_profit);
            }
            if let Some(slippage) = execution.slippage_bps {
                warn!("   Slippage: {} bps", slippage);
            }
            warn!("   Execution Time: {}ms", execution.execution_time_ms);
        }
        ExecutionStatus::Failed => {
            error!("\n❌ TRADE EXECUTION FAILED #{}", execution.id);
            error!("   Error: {}", execution.error_message.as_ref().unwrap_or(&"Unknown".to_string()));
        }
    }
}

// VERSION 0.3.0 - COMPLETE MAIN FUNCTION WITH TRADE EXECUTION & VOLATILITY
#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    let _logging_guard = setup_logging()?;
    setup_output_directories()?;
    
    info!("🛩️  Aerodrome Arbitrage Bot v0.5.0 - Trade Execution & Volatility");
    info!("📋 Configuration:");
    info!("   Network: {}", CONFIG.network);
    info!("   Trade Size: {} ETH", CONFIG.trade_size_eth);
    info!("   Min Profit: ${}", CONFIG.min_profit_usd);
    info!("   Safety Checks: {}", CONFIG.enable_safety_checks);
    info!("   Market Making: {}", CONFIG.enable_market_making);
    info!("   Trade Execution: {}", CONFIG.enable_trade_execution);
    // pause for 5 seconds to allow for the user to read the configuration
    tokio::time::sleep(Duration::from_secs(5)).await;
    if CONFIG.enable_trade_execution {
        info!("   Max Gas Price: {} gwei", CONFIG.max_gas_price_gwei);
        info!("   Slippage Tolerance: {} bps", CONFIG.slippage_tolerance_bps);
        info!("   ⚠️  TESTNET MODE - No real funds at risk");
    }
    info!("   Volatility Threshold: {}%", CONFIG.volatility_threshold);
    info!("   Volatility Spread Multiplier: {}x", CONFIG.volatility_spread_multiplier);
    
    if CONFIG.trade_size_eth < MIN_TRADE_SIZE_ETH || CONFIG.trade_size_eth > MAX_TRADE_SIZE_ETH {
        return Err(anyhow::anyhow!("Trade size out of bounds: {} ETH", CONFIG.trade_size_eth));
    }
    
    // Initialize trade execution engine
    let trade_execution_engine = TradeExecutionEngine::new(&CONFIG).await?;
    let alchemy_key = CONFIG.alchemy_api_key.as_ref()
        .expect("ALCHEMY_API_KEY is required");
    // Setup mainnet provider
    let rpc_url = format!("https://base-mainnet.g.alchemy.com/v2/{}", &alchemy_key);
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
        error!("Failed to connect to Base network after multiple attempts");
        anyhow::anyhow!("Network connection failed: {}", e)
    })?;
    
    info!("✅ Connected to Base at block {}", block);
    
    // Test Sepolia connection if trade execution enabled
    if CONFIG.enable_trade_execution {
        if let Some(sepolia_provider) = &trade_execution_engine.sepolia_provider {
            info!("🔗 Testing connection to Base Sepolia...");
            let sepolia_block = retry_with_backoff(
                || async {
                    sepolia_provider.get_block_number().await
                        .context("Failed to get Sepolia block number")
                },
                &RetryConfig::default(),
                "Base Sepolia connection",
            ).await.map_err(|e| {
                error!("Failed to connect to Base Sepolia after multiple attempts");
                anyhow::anyhow!("Network connection failed: {}", e)
            })?;
            info!("✅ Connected to Base Sepolia at block {}", sepolia_block);
        }
    }
    
    // Initialize components
    let circuit_breaker = Arc::new(CircuitBreaker::new(CONFIG.circuit_breaker_cooldown_secs));
    let error_recovery = Arc::new(ErrorRecovery::new());
    let market_making_engine = MarketMakingEngine::new();
    
    // Determine which pools to use based on network
    let pools_to_validate = if CONFIG.network == "mainnet" {
        POOLS_MAINNET
    } else {
        POOLS_SEPOLIA
    };
    
    let (weth_addr, usdc_addr, usdbc_addr) = if CONFIG.network == "mainnet" {
        (WETH_MAINNET, USDC_MAINNET, USDBC_MAINNET)
    } else {
        (WETH_SEPOLIA, USDC_SEPOLIA, USDC_SEPOLIA) // Use USDC for both on Sepolia
    };
    
    info!("\n🔍 Validating Aerodrome pools on {}...", CONFIG.network);
    let mut valid_pools = Vec::new();
    let mut pool_errors = 0;
    
    for (name, address) in pools_to_validate {
        match validate_pool_with_retry(&provider, name, *address, weth_addr, usdc_addr, usdbc_addr).await {
            Ok(pool_info) => {
                info!("✅ {} - Valid WETH/USD pool", name);
                valid_pools.push(pool_info);
            }
            Err(e) => {
                error!("❌ {} - Validation failed: {}", name, e);
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
    
    if CONFIG.enable_market_making {
        info!("\n🎯 Market Making Engine initialized with volatility adaptation");
        info!("   Multi-timeframe volatility tracking: 5min, 30min, 1hour");
        info!("   Dynamic spread adjustment based on volatility");
        info!("   Position sizing adapts to market conditions");
    }
    
    if CONFIG.enable_trade_execution {
        info!("\n🚀 Trade Execution Engine initialized");
        info!("   Mode: Simulation on Base Sepolia testnet");
        info!("   Success rate modeling based on volatility");
        info!("   Realistic gas and slippage simulation");
    }
    
    // Monitoring state
    let start_time = Instant::now();
    let mut dex_last_update: Option<Instant> = None;
    let mut cex_last_update: Option<Instant> = None;
    let mut last_known_cex_price: Option<Decimal> = None;
    let mut consecutive_cex_failures = 0;
    let mut total_opportunities = 0u64;
    let mut profitable_opportunities = 0u64;
    let mut total_potential_profit = dec!(0);
    let mut total_market_making_signals = 0u64;
    let mut total_executions = 0u64;
    let mut successful_executions = 0u64;
    let mut error_counts: HashMap<String, u32> = HashMap::new();
    
    let mut interval = tokio::time::interval(Duration::from_secs(2));
    
    // Set up Ctrl+C handler
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    let shutdown_tx = Arc::new(tokio::sync::Mutex::new(Some(shutdown_tx)));

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("\n📛 Received shutdown signal (Ctrl+C)...");
        if let Some(tx) = shutdown_tx.lock().await.take() {
            let _ = tx.send(());
        }
    });

    info!("\n🚀 Starting main monitoring loop...\n");
    
    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Main monitoring logic
                if !circuit_breaker.can_proceed().await {
                    warn!("⚡ Circuit breaker is OPEN, waiting for cooldown...");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
                
                if start_time.elapsed().as_secs() % 30 == 0 {
                    let health = run_health_check(
                        &dex_last_update, 
                        &cex_last_update, 
                        &circuit_breaker, 
                        start_time
                    ).await;
                    
                    info!("🏥 Health Check: DEX={}, CEX={}, Uptime={}s, Errors={}", 
                        if health.dex_connection { "OK" } else { "FAIL" },
                        if health.cex_connection { "OK" } else { "FAIL" },
                        health.uptime_seconds,
                        health.consecutive_errors
                    );
                    
                    if !error_counts.is_empty() {
                        debug!("Error summary: {:?}", error_counts);
                    }
                }
                
                let cex_price = match get_binance_price_enhanced().await {
                    Ok(price) => {
                        cex_last_update = Some(Instant::now());
                        last_known_cex_price = Some(price);
                        consecutive_cex_failures = 0;
                        circuit_breaker.record_success().await;
                        price
                    }
                    Err(e) => {
                        consecutive_cex_failures += 1;
                        *error_counts.entry("cex_price".to_string()).or_insert(0) += 1;
                        
                        let recovery_action = error_recovery.handle_error(&e, "CEX price fetch").await;
                        
                        match recovery_action {
                            RecoveryAction::Retry { delay } => {
                                warn!("CEX error (attempt {}): {}. Retrying in {:?}", 
                                    consecutive_cex_failures, e, delay);
                                tokio::time::sleep(delay).await;
                                continue;
                            }
                            RecoveryAction::Skip { .. } => {
                                if let Some(fallback_price) = last_known_cex_price {
                                    if consecutive_cex_failures <= 3 {
                                        warn!("Using last known CEX price: ${:.2} (age: {:?})", 
                                            fallback_price,
                                            cex_last_update.map(|t| t.elapsed()).unwrap_or(Duration::MAX)
                                        );
                                        fallback_price
                                    } else {
                                        error!("Too many CEX failures ({}), skipping iteration", consecutive_cex_failures);
                                        if circuit_breaker.record_error().await {
                                            error!("Circuit breaker activated due to CEX errors");
                                        }
                                        continue;
                                    }
                                } else {
                                    error!("No fallback CEX price available");
                                    continue;
                                }
                            }
                            RecoveryAction::Shutdown { reason } => {
                                error!("Critical error - shutting down: {}", reason);
                                break;
                            }
                            _ => {
                                error!("Unhandled CEX error: {}", e);
                                continue;
                            }
                        }
                    }
                };
                
                // Get current volatility metrics
                let volatility_metrics = market_making_engine.get_volatility_metrics().await;
                
                let mut pool_successes = 0;
                let mut pool_failures = 0;
                
                for pool in &valid_pools {
                    match process_pool_safe(&provider, pool, cex_price, &market_making_engine, &volatility_metrics, &mut error_counts).await {
                        Ok((arbitrage_opportunity, market_making_signal)) => {
                            pool_successes += 1;
                            
                            if let Some(mut opportunity) = arbitrage_opportunity {
                                total_opportunities += 1;
                                
                                // Add volatility assessment
                                opportunity.volatility_assessment = Some(volatility_metrics.clone());
                                
                                if CONFIG.enable_safety_checks {
                                    opportunity.validation_checks = validate_opportunity_with_volatility(
                                        &opportunity, 
                                        pool, 
                                        provider.as_ref(),
                                        &volatility_metrics
                                    ).await;
                                    
                                    if !opportunity.validation_checks.all_passed {
                                        warn!("Arbitrage opportunity failed validation: {:?}", 
                                            opportunity.validation_checks.warnings);
                                    } else if opportunity.net_profit_usd >= CONFIG.min_profit_usd {
                                        profitable_opportunities += 1;
                                        total_potential_profit += opportunity.net_profit_usd;
                                        
                                        warn!("🎯 ARBITRAGE OPPORTUNITY #{} [VALIDATED]", profitable_opportunities);
                                        warn!("📍 Pool: {}", opportunity.pool);
                                        warn!("📋 Strategy: {}", opportunity.direction);
                                        warn!("💰 Profit Analysis:");
                                        warn!("   DEX Price: ${:.4}", opportunity.dex_price);
                                        warn!("   CEX Price: ${:.4}", opportunity.cex_price);
                                        warn!("   Net Profit: ${:.2}", opportunity.net_profit_usd);
                                        warn!("   ROI: {:.3}%", opportunity.roi_pct);
                                        warn!("📊 Volatility: {:.2}% (Impact: {:?})",
                                            volatility_metrics.short_term_volatility,
                                            volatility_metrics.impact_assessment
                                        );
                                        warn!("✅ All validation checks passed");
                                        
                                        // Execute trade simulation if enabled
                                        if CONFIG.enable_trade_execution {
                                            match trade_execution_engine.simulate_trade_execution(
                                                &opportunity,
                                                &volatility_metrics
                                            ).await {
                                                Ok(execution) => {
                                                    total_executions += 1;
                                                    if matches!(execution.status, ExecutionStatus::Success | ExecutionStatus::Simulated) {
                                                        successful_executions += 1;
                                                    }
                                                    
                                                    print_trade_execution(&execution);
                                                    opportunity.execution_simulation = Some(execution.clone());
                                                    
                                                    if let Err(e) = save_trade_execution(&execution) {
                                                        error!("Failed to save trade execution: {}", e);
                                                        *error_counts.entry("save_execution".to_string()).or_insert(0) += 1;
                                                    }
                                                }
                                                Err(e) => {
                                                    error!("Trade execution simulation failed: {}", e);
                                                    *error_counts.entry("execution_simulation".to_string()).or_insert(0) += 1;
                                                }
                                            }
                                        }
                                        
                                        if let Err(e) = save_opportunity(&opportunity) {
                                            error!("Failed to save arbitrage opportunity: {}", e);
                                            *error_counts.entry("save_opportunity".to_string()).or_insert(0) += 1;
                                        }
                                    }
                                }
                            }
                            
                            if let Some(signal) = market_making_signal {
                                total_market_making_signals += 1;
                                
                                print_market_making_signal(&signal);
                                
                                if let Err(e) = save_market_making_signal(&signal) {
                                    error!("Failed to save market making signal: {}", e);
                                    *error_counts.entry("save_market_making_signal".to_string()).or_insert(0) += 1;
                                }
                            }
                        }
                        Err(e) => {
                            pool_failures += 1;
                            *error_counts.entry(format!("pool_{}", pool.name)).or_insert(0) += 1;
                            
                            match e {
                                BotError::InsufficientLiquidity { .. } => {
                                    debug!("Pool {} has insufficient liquidity", pool.name);
                                }
                                BotError::Contract { .. } => {
                                    warn!("Contract error for pool {}: {}", pool.name, e);
                                    if circuit_breaker.record_error().await {
                                        error!("Circuit breaker activated due to contract errors");
                                    }
                                }
                                _ => {
                                    error!("Error processing pool {}: {}", pool.name, e);
                                }
                            }
                        }
                    }
                    
                    dex_last_update = Some(Instant::now());
                }
                
                if pool_failures > 0 {
                    debug!("Pool processing: {} successful, {} failed", pool_successes, pool_failures);
                }
                
                if (total_opportunities > 0 && total_opportunities % 50 == 0) || 
                (total_market_making_signals > 0 && total_market_making_signals % 25 == 0) ||
                (total_executions > 0 && total_executions % 10 == 0) {
                    print_session_stats(
                        start_time,
                        total_opportunities,
                        profitable_opportunities,
                        total_potential_profit,
                        total_market_making_signals,
                        total_executions,
                        successful_executions,
                        &error_counts,
                        &circuit_breaker,
                    ).await;
                }
                
                let total_errors: u32 = error_counts.values().sum();
                if total_errors > 1000 {
                    error!("Too many total errors ({}), consider restarting", total_errors);
                    warn!("Error breakdown: {:?}", error_counts);
                }
            }
            _ = &mut shutdown_rx => {
                info!("Shutdown signal received, exiting main loop...");
                break;
            }
        }
    }
    
    info!("\n🛑 Shutting down gracefully...");
    info!("Final statistics:");
    info!("   Total runtime: {:?}", start_time.elapsed());
    info!("   Arbitrage opportunities found: {}", total_opportunities);
    info!("   Profitable arbitrage opportunities: {}", profitable_opportunities);
    info!("   Total potential arbitrage profit: ${:.2}", total_potential_profit);
    info!("   Market making signals generated: {}", total_market_making_signals);
    info!("   Trade executions simulated: {}", total_executions);
    info!("   Successful executions: {}", successful_executions);
    info!("   Total errors: {:?}", error_counts);
    
    Ok(())
}