//! Volatility analysis types

use rust_decimal::Decimal;
use serde::Serialize;
use super::ExecutionUrgency;

#[derive(Debug, Clone, Serialize)]
pub struct VolatilityMetrics {
    pub short_term_volatility: Decimal,  // 5 min
    pub medium_term_volatility: Decimal, // 30 min
    pub long_term_volatility: Decimal,   // 1 hour
    pub volatility_trend: VolatilityTrend,
    pub impact_assessment: VolatilityImpact,
    pub recommended_adjustments: VolatilityAdjustments,
}

#[derive(Debug, Clone, Serialize)]
pub enum VolatilityTrend {
    Increasing,
    Decreasing,
    Stable,
    Volatile,
}

#[derive(Debug, Clone, Serialize)]
pub enum VolatilityImpact {
    Low,      // < 2%
    Moderate, // 2-5%
    High,     // 5-10%
    Extreme,  // > 10%
}

#[derive(Debug, Clone, Serialize)]
pub struct VolatilityAdjustments {
    pub spread_multiplier: Decimal,
    pub position_size_factor: Decimal,
    pub execution_urgency: ExecutionUrgency,
}
