//! Validation result types

use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ValidationResult {
    pub price_sanity: bool,
    pub liquidity_check: bool,
    pub gas_economics: bool,
    pub slippage_acceptable: bool,
    pub volatility_acceptable: bool,
    pub all_passed: bool,
    pub warnings: Vec<String>,
}
