//! Aerodrome Arbitrage Bot - Main Entry Point
//! 
//! Production-ready bot for Base network arbitrage and market making

use aero_arb_mm_bot::*;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::time;
use tracing::{info, warn, error, debug};
use alloy::providers::Provider;
use crate::errors::RecoveryAction;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    
    // Initialize logging
    let _logging_guard = utils::setup_logging()?;
    utils::setup_output_directories()?;
    
    // Load configuration
    let config = CONFIG.clone();
    
    info!("🛩️  Aerodrome Arbitrage Bot v0.5.0 - Trade Execution & Volatility");
    info!("📋 Configuration:");
    info!("   Network: {}", config.network);
    info!("   Trade Size: {} ETH", config.trade_size_eth);
    info!("   Min Profit: ${}", config.min_profit_usd);
    info!("   Safety Checks: {}", config.enable_safety_checks);
    info!("   Market Making: {}", config.enable_market_making);
    info!("   Trade Execution: {}", config.enable_trade_execution);
    
    // pause for 5 seconds to allow for the user to read the configuration
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    if config.enable_trade_execution {
        info!("   Max Gas Price: {} gwei", config.max_gas_price_gwei);
        info!("   Slippage Tolerance: {} bps", config.slippage_tolerance_bps);
        info!("   ⚠️  TESTNET MODE - No real funds at risk");
    }
    info!("   Volatility Threshold: {}%", config.volatility_threshold);
    info!("   Volatility Spread Multiplier: {}x", config.volatility_spread_multiplier);
    
    // Validate configuration
    if config.trade_size_eth < config::MIN_TRADE_SIZE_ETH || 
       config.trade_size_eth > config::MAX_TRADE_SIZE_ETH {
        return Err(anyhow::anyhow!("Trade size out of bounds: {} ETH", config.trade_size_eth));
    }
    
    // Initialize components
    let circuit_breaker = Arc::new(errors::CircuitBreaker::new(config.circuit_breaker_cooldown_secs));
    let error_recovery = Arc::new(errors::ErrorRecovery::new());
    
    // Setup network providers
    let provider = network::setup_mainnet_provider(&config).await?;
    let trade_execution_engine = execution::TradeExecutionEngine::new(&config).await?;
    let market_making_engine = market_making::MarketMakingEngine::new();
    
    // Test Sepolia connection if trade execution enabled
    if config.enable_trade_execution {
        if let Some(sepolia_provider) = &trade_execution_engine.sepolia_provider {
            info!("🔗 Testing connection to Base Sepolia...");
            let sepolia_block = network::retry_with_backoff(
                || async {
                    sepolia_provider.get_block_number().await
                        .map_err(|e| anyhow::anyhow!("Failed to get Sepolia block number: {}", e))
                },
                &network::RetryConfig::default(),
                "Base Sepolia connection",
            ).await.map_err(|e| {
                error!("Failed to connect to Base Sepolia after multiple attempts");
                anyhow::anyhow!("Network connection failed: {}", e)
            })?;
            info!("✅ Connected to Base Sepolia at block {}", sepolia_block);
        }
    }
    
    // Initialize and validate pools
    let valid_pools = pools::initialize_and_validate_pools(&provider, &config).await?;
    
    if valid_pools.is_empty() {
        return Err(anyhow::anyhow!("No valid pools found after validation"));
    }
    
    info!("✅ Initialized {} valid pools", valid_pools.len());
    
    if config.enable_market_making {
        info!("\n🎯 Market Making Engine initialized with volatility adaptation");
        info!("   Multi-timeframe volatility tracking: 5min, 30min, 1hour");
        info!("   Dynamic spread adjustment based on volatility");
        info!("   Position sizing adapts to market conditions");
    }
    
    if config.enable_trade_execution {
        info!("\n🚀 Trade Execution Engine initialized");
        info!("   Mode: Simulation on Base Sepolia testnet");
        info!("   Success rate modeling based on volatility");
        info!("   Realistic gas and slippage simulation");
    }
    
    // Setup monitoring state
    let start_time = Instant::now();
    let mut monitoring_state = MonitoringState::new();
    
    // Setup shutdown handler
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
    
    let mut interval = time::interval(Duration::from_secs(2));
    
    // Main monitoring loop
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(e) = run_monitoring_cycle(
                    &provider,
                    &trade_execution_engine,
                    &market_making_engine,
                    &valid_pools,
                    &config,
                    &circuit_breaker,
                    &error_recovery,
                    &mut monitoring_state,
                    start_time,
                ).await {
                    error!("Monitoring cycle error: {}", e);
                    if circuit_breaker.record_error().await {
                        error!("Circuit breaker activated due to monitoring errors");
                    }
                }
            }
            _ = &mut shutdown_rx => {
                info!("Shutdown signal received, exiting main loop...");
                break;
            }
        }
    }
    
    // Print final statistics
    print_final_statistics(start_time, &monitoring_state);
    
    Ok(())
}

/// Monitoring state to track statistics
struct MonitoringState {
    total_opportunities: u64,
    profitable_opportunities: u64,
    total_potential_profit: rust_decimal::Decimal,
    total_market_making_signals: u64,
    total_executions: u64,
    successful_executions: u64,
    error_counts: HashMap<String, u32>,
    dex_last_update: Option<Instant>,
    cex_last_update: Option<Instant>,
    last_known_cex_price: Option<rust_decimal::Decimal>,
    consecutive_cex_failures: u32,
}

impl MonitoringState {
    fn new() -> Self {
        Self {
            total_opportunities: 0,
            profitable_opportunities: 0,
            total_potential_profit: rust_decimal_macros::dec!(0),
            total_market_making_signals: 0,
            total_executions: 0,
            successful_executions: 0,
            error_counts: HashMap::new(),
            dex_last_update: None,
            cex_last_update: None,
            last_known_cex_price: None,
            consecutive_cex_failures: 0,
        }
    }
}

/// Run a single monitoring cycle
async fn run_monitoring_cycle(
    provider: &Arc<ConcreteProvider>,
    trade_execution_engine: &execution::TradeExecutionEngine,
    market_making_engine: &market_making::MarketMakingEngine,
    valid_pools: &[PoolInfo],
    config: &Config,
    circuit_breaker: &Arc<errors::CircuitBreaker>,
    error_recovery: &Arc<errors::ErrorRecovery>,
    state: &mut MonitoringState,
    start_time: Instant,
) -> Result<()> {
    // Check circuit breaker
    if !circuit_breaker.can_proceed().await {
        warn!("⚡ Circuit breaker is OPEN, waiting for cooldown...");
        tokio::time::sleep(Duration::from_secs(10)).await;
        return Ok(());
    }
    
    // Periodic health check
    if start_time.elapsed().as_secs() % 30 == 0 {
        let health = utils::run_health_check(
            &state.dex_last_update,
            &state.cex_last_update,
            circuit_breaker,
            start_time,
        ).await;
        
        info!("🏥 Health Check: DEX={}, CEX={}, Uptime={}s, Errors={}",
            if health.dex_connection { "OK" } else { "FAIL" },
            if health.cex_connection { "OK" } else { "FAIL" },
            health.uptime_seconds,
            health.consecutive_errors
        );
        
        if !state.error_counts.is_empty() {
            debug!("Error summary: {:?}", state.error_counts);
        }
    }
    
    // Get CEX price with error handling
    let cex_price = match network::get_binance_price_enhanced().await {
        Ok(price) => {
            state.cex_last_update = Some(Instant::now());
            state.last_known_cex_price = Some(price);
            state.consecutive_cex_failures = 0;
            circuit_breaker.record_success().await;
            price
        }
        Err(e) => {
            state.consecutive_cex_failures += 1;
            *state.error_counts.entry("cex_price".to_string()).or_insert(0) += 1;
            
            // Use error recovery strategy
            let recovery_action = error_recovery.handle_error(&e, "CEX price fetch").await;
            return handle_cex_error_recovery(recovery_action, state, circuit_breaker, e).await;
        }
    };
    
    // Get volatility metrics
    let volatility_metrics = market_making_engine.get_volatility_metrics().await;
    
    let mut pool_successes = 0;
    let mut pool_failures = 0;
    
    // Process all pools
    for pool in valid_pools {
        match process_single_pool(
            provider,
            trade_execution_engine,
            market_making_engine,
            pool,
            cex_price,
            &volatility_metrics,
            config,
            state,
        ).await {
            Ok(_) => pool_successes += 1,
            Err(e) => {
                pool_failures += 1;
                *state.error_counts.entry(format!("pool_{}", pool.name)).or_insert(0) += 1;
                
                match e.downcast_ref::<BotError>() {
                    Some(BotError::InsufficientLiquidity { .. }) => {
                        debug!("Pool {} has insufficient liquidity", pool.name);
                    }
                    Some(BotError::Contract { .. }) => {
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
    }
    
    state.dex_last_update = Some(Instant::now());
    
    if pool_failures > 0 {
        debug!("Pool processing: {} successful, {} failed", pool_successes, pool_failures);
    }
    
    // Print periodic statistics
    if should_print_statistics(state) {
        utils::print_session_stats(
            start_time,
            state.total_opportunities,
            state.profitable_opportunities,
            state.total_potential_profit,
            state.total_market_making_signals,
            state.total_executions,
            state.successful_executions,
            &state.error_counts,
            circuit_breaker,
        ).await;
    }
    
    // Check for too many errors
    let total_errors: u32 = state.error_counts.values().sum();
    if total_errors > 1000 {
        error!("Too many total errors ({}), consider restarting", total_errors);
        warn!("Error breakdown: {:?}", state.error_counts);
    }
    
    Ok(())
}

/// Handle CEX price error recovery
async fn handle_cex_error_recovery(
    recovery_action: errors::RecoveryAction,
    state: &mut MonitoringState,
    circuit_breaker: &Arc<errors::CircuitBreaker>,
    error: BotError,
) -> Result<()> {

    
    match recovery_action {
        RecoveryAction::Retry { delay } => {
            warn!("CEX error (attempt {}): {}. Retrying in {:?}",
                state.consecutive_cex_failures, error, delay);
            tokio::time::sleep(delay).await;
        }
        RecoveryAction::Skip { .. } => {
            if let Some(fallback_price) = state.last_known_cex_price {
                if state.consecutive_cex_failures <= 3 {
                    warn!("Using last known CEX price: ${:.2} (age: {:?})",
                        fallback_price,
                        state.cex_last_update.map(|t| t.elapsed()).unwrap_or(Duration::MAX)
                    );
                    return Ok(());
                }
            }
            error!("Too many CEX failures, activating circuit breaker");
            circuit_breaker.record_error().await;
        }
        RecoveryAction::Shutdown { reason } => {
            return Err(anyhow::anyhow!("Critical error - shutting down: {}", reason));
        }
        _ => {
            error!("Unhandled CEX error: {}", error);
        }
    }
    
    Ok(())
}

/// Process a single pool for arbitrage and market making opportunities
async fn process_single_pool(
    provider: &Arc<ConcreteProvider>,
    trade_execution_engine: &execution::TradeExecutionEngine,
    market_making_engine: &market_making::MarketMakingEngine,
    pool: &PoolInfo,
    cex_price: rust_decimal::Decimal,
    volatility_metrics: &VolatilityMetrics,
    config: &Config,
    state: &mut MonitoringState,
) -> Result<()> {
    // Calculate DEX price
    let dex_price = pools::calculate_pool_price_safe_with_retry(provider, pool).await
        .map_err(|e| anyhow::anyhow!("Failed to calculate DEX price: {}", e))?;
    
    let price_diff_pct = ((dex_price - cex_price).abs() / cex_price) * rust_decimal_macros::dec!(100);
    
    info!(
        "💹 {} | DEX: ${:.4} | CEX: ${:.4} | Diff: {:.3}% | Vol: {:.2}%",
        pool.name, dex_price, cex_price, price_diff_pct,
        volatility_metrics.short_term_volatility
    );
    
    // Update market making price history
    market_making_engine.update_price_history(cex_price).await;
    
    // Check for arbitrage opportunities
    if let Some(mut opportunity) = arbitrage::calculate_arbitrage(
        &pool.name,
        dex_price,
        cex_price,
        config.trade_size_eth,
    ) {
        state.total_opportunities += 1;
        opportunity.volatility_assessment = Some(volatility_metrics.clone());
        
        // Validate opportunity
        if config.enable_safety_checks {
            opportunity.validation_checks = validation::validate_opportunity_with_volatility(
                &opportunity,
                pool,
                provider.as_ref(),
                volatility_metrics,
            ).await;
            
            if !opportunity.validation_checks.all_passed {
                warn!("Arbitrage opportunity failed validation: {:?}", 
                    opportunity.validation_checks.warnings);
            } else if opportunity.net_profit_usd >= config.min_profit_usd {
                state.profitable_opportunities += 1;
                state.total_potential_profit += opportunity.net_profit_usd;
                
                utils::print_arbitrage_opportunity(&opportunity, volatility_metrics);
                
                // Execute trade simulation if enabled
                if config.enable_trade_execution {
                    match trade_execution_engine.simulate_trade_execution(
                        &opportunity,
                        volatility_metrics,
                    ).await {
                        Ok(execution) => {
                            state.total_executions += 1;
                            if matches!(execution.status, ExecutionStatus::Success | ExecutionStatus::Simulated) {
                                state.successful_executions += 1;
                            }
                            
                            utils::print_trade_execution(&execution);
                            opportunity.execution_simulation = Some(execution.clone());
                            
                            if let Err(e) = storage::save_trade_execution(&execution) {
                                error!("Failed to save trade execution: {}", e);
                                *state.error_counts.entry("save_execution".to_string()).or_insert(0) += 1;
                            }
                        }
                        Err(e) => {
                            error!("Trade execution simulation failed: {}", e);
                            *state.error_counts.entry("execution_simulation".to_string()).or_insert(0) += 1;
                        }
                    }
                }
                
                if let Err(e) = storage::save_opportunity(&opportunity) {
                    error!("Failed to save arbitrage opportunity: {}", e);
                    *state.error_counts.entry("save_opportunity".to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    
    // Generate market making signals
    if config.enable_market_making {
        match pools::analyze_liquidity_depth(
            provider.as_ref(),
            pool,
            cex_price,
        ).await {
            Ok(liquidity_depth) => {
                match market_making_engine.generate_market_making_signal(
                    pool,
                    cex_price,
                    dex_price,
                    liquidity_depth,
                    provider.as_ref(),
                ).await {
                    Ok(signal) => {
                        state.total_market_making_signals += 1;
                        utils::print_market_making_signal(&signal);
                        
                        if let Err(e) = storage::save_market_making_signal(&signal) {
                            error!("Failed to save market making signal: {}", e);
                            *state.error_counts.entry("save_market_making_signal".to_string()).or_insert(0) += 1;
                        }
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
    
    Ok(())
}

/// Check if we should print statistics
fn should_print_statistics(state: &MonitoringState) -> bool {
    (state.total_opportunities > 0 && state.total_opportunities % 50 == 0) ||
    (state.total_market_making_signals > 0 && state.total_market_making_signals % 25 == 0) ||
    (state.total_executions > 0 && state.total_executions % 10 == 0)
}

/// Print final statistics on shutdown
fn print_final_statistics(start_time: Instant, state: &MonitoringState) {
    info!("\n🛑 Shutting down gracefully...");
    info!("Final statistics:");
    info!("   Total runtime: {:?}", start_time.elapsed());
    info!("   Arbitrage opportunities found: {}", state.total_opportunities);
    info!("   Profitable arbitrage opportunities: {}", state.profitable_opportunities);
    info!("   Total potential arbitrage profit: ${:.2}", state.total_potential_profit);
    info!("   Market making signals generated: {}", state.total_market_making_signals);
    info!("   Trade executions simulated: {}", state.total_executions);
    info!("   Successful executions: {}", state.successful_executions);
    info!("   Total errors: {:?}", state.error_counts);
}
