//! Health monitoring types

use std::time::Instant;

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub dex_connection: bool,
    pub cex_connection: bool,
    #[allow(dead_code)]
    pub last_dex_update: Option<Instant>,
    #[allow(dead_code)]
    pub last_cex_update: Option<Instant>,
    pub consecutive_errors: u32,
    #[allow(dead_code)]
    pub circuit_breaker_active: bool,
    pub uptime_seconds: u64,
}
