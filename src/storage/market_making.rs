//! Market making signal storage

use anyhow::Result;
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use tracing::info;
use crate::types::MarketMakingSignal;

pub fn save_market_making_signal(signal: &MarketMakingSignal) -> Result<()> {
    let filename = format!("output/market_making/signals_{}.jsonl", 
        Utc::now().format("%Y-%m-%d"));
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)?;
    
    writeln!(file, "{}", serde_json::to_string(signal)?)?;
    
    info!(
        signal_id = %signal.id,
        strategy = ?signal.strategy.strategy_type,
        spread_bps = signal.effective_spread_bps,
        priority = ?signal.execution_priority,
        "Saved market making signal"
    );
    
    Ok(())
}
