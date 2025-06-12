//! Market making engine with volatility adaptation

use alloy::providers::Provider;
use anyhow::Result;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::debug;
use crate::{
    config::{CONFIG, MIN_SPREAD_BPS, MAX_SPREAD_BPS},
    types::{
        PoolInfo, LiquidityDepth, MarketMakingSignal, InventoryAnalysis, MarketConditions,
        LiquidityStrategy, RiskMetrics, ExecutionPriority, VolatilityMetrics,
        InventoryImbalance, MarketTrend, SpreadEnvironment, VolumeProfile, DepthQuality,
        StrategyType, RangeBounds, RiskLevel, VolatilityImpact, VolatilityTrend, ExecutionUrgency,
    },
    volatility::MultiTimeframeVolatilityCalculator,
};

pub struct MarketMakingEngine {
    volatility_calculator: Arc<RwLock<MultiTimeframeVolatilityCalculator>>,
    last_signals: Arc<RwLock<HashMap<String, MarketMakingSignal>>>,
}

impl MarketMakingEngine {
    pub fn new() -> Self {
        Self {
            volatility_calculator: Arc::new(RwLock::new(MultiTimeframeVolatilityCalculator::new())),
            last_signals: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_price_history(&self, price: Decimal) {
        self.volatility_calculator.write().await.add_price(price).await;
    }

    pub async fn get_volatility_metrics(&self) -> VolatilityMetrics {
        self.volatility_calculator.read().await.get_volatility_metrics().await
    }

    pub async fn generate_market_making_signal(
        &self,
        pool_info: &PoolInfo,
        fair_value_price: Decimal,
        current_pool_price: Decimal,
        liquidity_depth: LiquidityDepth,
        _provider: &dyn Provider,
    ) -> Result<MarketMakingSignal> {
        let signal_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now();

        // Get volatility metrics
        let volatility_metrics = self.get_volatility_metrics().await;

        debug!(
            "Volatility analysis: Short={:.2}%, Medium={:.2}%, Long={:.2}%, Trend={:?}",
            volatility_metrics.short_term_volatility,
            volatility_metrics.medium_term_volatility,
            volatility_metrics.long_term_volatility,
            volatility_metrics.volatility_trend
        );

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

        let rebalance_needed = ratio_diff > CONFIG.rebalance_threshold;
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
        market_conditions: &MarketConditions,
        inventory_analysis: &InventoryAnalysis,
        volatility_metrics: &VolatilityMetrics,
        fair_value_price: Decimal,
    ) -> u32 {
        let mut spread_bps = CONFIG.base_spread_bps;

        // Apply volatility adjustments
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
        match market_conditions.liquidity_depth.depth_quality {
            DepthQuality::Excellent => {},
            DepthQuality::Good => spread_bps = (spread_bps as f64 * 1.05) as u32,
            DepthQuality::Fair => spread_bps = (spread_bps as f64 * 1.15) as u32,
            DepthQuality::Poor => spread_bps = (spread_bps as f64 * 1.3) as u32,
        }

        // Spread environment adjustments
        match market_conditions.spread_environment {
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
        use crate::config::{MIN_TRADE_SIZE_ETH};
        
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
