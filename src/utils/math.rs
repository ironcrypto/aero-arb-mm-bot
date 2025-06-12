//! Mathematical utility functions

use rust_decimal::prelude::*;
use rust_decimal_macros::dec;

pub fn pow10(n: i32) -> Decimal {
    match n {
        0 => dec!(1),
        6 => dec!(1_000_000),
        18 => dec!(1_000_000_000_000_000_000),
        _ => {
            let mut result = dec!(1);
            if n > 0 {
                for _ in 0..n {
                    result *= dec!(10);
                }
            } else {
                for _ in 0..(-n) {
                    result /= dec!(10);
                }
            }
            result
        }
    }
}
