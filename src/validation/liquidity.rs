//! Liquidity validation functions

use anyhow::Result;
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;

pub fn validate_liquidity(weth_reserve: Decimal, usd_reserve: Decimal) -> Result<()> {
    const MIN_WETH_LIQUIDITY: Decimal = dec!(0.1);
    const MIN_USD_LIQUIDITY: Decimal = dec!(100);
    
    if weth_reserve < MIN_WETH_LIQUIDITY {
        return Err(anyhow::anyhow!("Insufficient WETH liquidity: {} WETH", weth_reserve));
    }
    
    if usd_reserve < MIN_USD_LIQUIDITY {
        return Err(anyhow::anyhow!("Insufficient USD liquidity: ${}", usd_reserve));
    }
    
    Ok(())
}
