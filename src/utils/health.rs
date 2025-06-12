//! Health monitoring utilities

use std::time::Instant;
use crate::{
    config::PRICE_STALENESS_SECONDS,
    errors::CircuitBreaker,
    types::HealthStatus,
};

pub async fn run_health_check(
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
