//! Display and printing utilities

use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn, error};
use crate::{
    errors::CircuitBreaker,
    types::{ArbitrageOpportunity, MarketMakingSignal, TradeExecution, ExecutionStatus, VolatilityMetrics},
};

pub async fn print_session_stats(
    start_time: Instant,
    total_opportunities: u64,
    profitable_opportunities: u64,
    total_potential_profit: rust_decimal::Decimal,
    total_market_making_signals: u64,
    total_executions: u64,
    successful_executions: u64,
    error_counts: &HashMap<String, u32>,
    circuit_breaker: &CircuitBreaker,
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

pub fn print_arbitrage_opportunity(opportunity: &ArbitrageOpportunity, volatility_metrics: &VolatilityMetrics) {
    warn!("\n🎯 ARBITRAGE OPPORTUNITY #{}", opportunity.id);
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
}

pub fn print_market_making_signal(signal: &MarketMakingSignal) {
    warn!("\n🎯 MARKET MAKING SIGNAL #{}", signal.id);
    warn!("📍 Pool: {}", signal.pool);
    warn!("💰 Price Analysis:");
    warn!("   Fair Value (CEX): ${:.4}", signal.fair_value_price);
    warn!("   Current Pool:     ${:.4}", signal.current_pool_price);
    warn!("   Target Bid:       ${:.4}", signal.target_bid_price);
    warn!("   Target Ask:       ${:.4}", signal.target_ask_price);
    warn!("   Effective Spread: {} bps ({:.3}%)", 
        signal.effective_spread_bps, 
        rust_decimal::Decimal::from(signal.effective_spread_bps) / rust_decimal_macros::dec!(100)
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

pub fn print_trade_execution(execution: &TradeExecution) {
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
