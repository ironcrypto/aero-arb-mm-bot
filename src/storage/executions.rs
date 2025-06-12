//! Trade execution storage

use anyhow::Result;
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use tracing::info;
use crate::types::TradeExecution;

pub fn save_trade_execution(execution: &TradeExecution) -> Result<()> {
    let filename = format!("output/executions/trades_{}.jsonl", 
        Utc::now().format("%Y-%m-%d"));
    
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&filename)?;
    
    writeln!(file, "{}", serde_json::to_string(execution)?)?;
    
    info!(
        execution_id = %execution.id,
        status = ?execution.status,
        actual_profit = ?execution.actual_profit_usd,
        "Saved trade execution"
    );
    
    Ok(())
}
