//! Configuration management for the Aerodrome bot

pub mod settings;

pub use settings::*;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref CONFIG: Config = Config::load();
}
