//! Arbitrage opportunity storage

use anyhow::Result;
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use tracing::info;
use crate::types::ArbitrageOpportunity;

pub fn save_opportunity(opp: &ArbitrageOpportunity) -> Result<()> {
    let filename = format!("output/opportunities/arbitrage_{}.jsonl", 
        Utc::now().format("%Y-%m-%d"));
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)?;
    
    writeln!(file, "{}", serde_json::to_string(opp)?)?;
    
    info!(
        opportunity_id = %opp.id,
        profit = %opp.net_profit_usd,
        validations_passed = opp.validation_checks.all_passed,
        "Saved validated arbitrage opportunity"
    );
    
    Ok(())
}
