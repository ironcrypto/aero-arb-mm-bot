//! Market making types and structures

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use std::time::Duration;
use super::VolatilityMetrics;

#[derive(Debug, Clone, Serialize)]
pub struct MarketMakingSignal {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub pool: String,
    pub fair_value_price: Decimal,
    pub current_pool_price: Decimal,
    pub target_bid_price: Decimal,
    pub target_ask_price: Decimal,
    pub effective_spread_bps: u32,
    pub position_size_eth: Decimal,
    pub inventory_analysis: InventoryAnalysis,
    pub market_conditions: MarketConditions,
    pub strategy: LiquidityStrategy,
    pub risk_metrics: RiskMetrics,
    pub volatility_metrics: VolatilityMetrics,
    pub execution_priority: ExecutionPriority,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryAnalysis {
    pub current_weth_balance: Decimal,
    pub current_usd_balance: Decimal,
    pub total_value_usd: Decimal,
    pub weth_ratio: Decimal,
    pub target_weth_ratio: Decimal,
    pub imbalance_severity: InventoryImbalance,
    pub rebalance_needed: bool,
    pub rebalance_amount_eth: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub enum InventoryImbalance {
    Balanced,
    SlightlyLong,
    SlightlyShort,
    SignificantlyLong,
    SignificantlyShort,
    CriticallyImbalanced,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarketConditions {
    pub price_volatility_1h: Decimal,
    pub liquidity_depth: LiquidityDepth,
    pub spread_environment: SpreadEnvironment,
    pub market_trend: MarketTrend,
    pub volume_profile: VolumeProfile,
}

#[derive(Debug, Clone, Serialize)]
pub enum MarketTrend {
    Bullish,
    Bearish,
    Sideways,
}

#[derive(Debug, Clone, Serialize)]
pub enum SpreadEnvironment {
    Tight,
    Normal,
    Wide,
    VeryWide,
}

#[derive(Debug, Clone, Serialize)]
pub enum VolumeProfile {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiquidityDepth {
    pub total_liquidity_usd: Decimal,
    pub weth_reserves: Decimal,
    pub usd_reserves: Decimal,
    pub depth_quality: DepthQuality,
}

#[derive(Debug, Clone, Serialize)]
pub enum DepthQuality {
    Excellent,
    Good,
    Fair,
    Poor,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiquidityStrategy {
    pub strategy_type: StrategyType,
    pub bid_size_eth: Decimal,
    pub ask_size_eth: Decimal,
    pub range_bounds: RangeBounds,
    pub duration_estimate: Duration,
    pub expected_daily_volume: Decimal,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize)]
pub enum StrategyType {
    TightSpread,
    WideSpread,
    InventoryManagement,
    TrendFollowing,
    VolatilityAdaptive,
}

#[derive(Debug, Clone, Serialize)]
pub struct RangeBounds {
    pub lower_bound: Decimal,
    pub upper_bound: Decimal,
    pub confidence_interval: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub enum RiskLevel {
    Conservative,
    Moderate,
    Aggressive,
    Speculative,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskMetrics {
    pub max_drawdown_usd: Decimal,
    pub value_at_risk_1d: Decimal,
    pub inventory_risk_score: Decimal,
    pub liquidity_risk_score: Decimal,
    pub volatility_risk_score: Decimal,
    pub overall_risk_score: Decimal,
    pub recommended_max_exposure: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub enum ExecutionPriority {
    Immediate,
    High,
    Medium,
    Low,
    Hold,
}
