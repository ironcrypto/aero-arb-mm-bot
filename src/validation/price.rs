//! Price validation functions

use anyhow::Result;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;

pub fn validate_price(price: Decimal, source: &str) -> Result<()> {
    if price <= dec!(0) {
        return Err(anyhow::anyhow!("{} price is zero or negative: {}", source, price));
    }
    
    if price < dec!(100) || price > dec!(100000) {
        return Err(anyhow::anyhow!("{} price out of reasonable range: ${}", source, price));
    }
    
    Ok(())
}
