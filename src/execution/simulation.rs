//! Trade execution simulation

use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::time::{Duration, Instant};
use tracing::info;
use crate::types::{
    ArbitrageOpportunity, TradeExecution, ExecutionStatus, TradeType, VolatilityMetrics, VolatilityImpact
};

pub async fn create_simulated_execution(
    execution_id: String,
    opportunity: &ArbitrageOpportunity,
    volatility_metrics: &VolatilityMetrics,
    start_time: Instant,
) -> anyhow::Result<TradeExecution> {
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

    info!("🎭 Simulated execution: success={}, slippage={}bps", is_successful, total_slippage_bps);

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
