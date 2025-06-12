//! Multi-timeframe volatility analysis

use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::{
    types::{VolatilityMetrics, VolatilityTrend, VolatilityImpact, VolatilityAdjustments, ExecutionUrgency},
    volatility::VolatilityCalculator,
};

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
