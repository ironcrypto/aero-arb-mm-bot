//! Volatility calculator for single timeframes

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};
use tracing::warn;

pub struct VolatilityCalculator {
    window: VecDeque<(SystemTime, f64)>,
    max_duration: Duration,
}

impl VolatilityCalculator {
    pub fn new(max_duration_secs: u64) -> Self {
        VolatilityCalculator {
            window: VecDeque::new(),
            max_duration: Duration::from_secs(max_duration_secs),
        }
    }

    pub fn add_value(&mut self, price: f64) {
        let now = SystemTime::now();
        self.window.push_back((now, price));

        while let Some((timestamp, _)) = self.window.front() {
            if let Ok(duration) = now.duration_since(*timestamp) {
                if duration > self.max_duration {
                    self.window.pop_front();
                } else {
                    break;
                }
            } else {
                warn!("Encountered a timestamp in the future: {:?}", timestamp);
                self.window.pop_front();
            }
        }
    }

    pub fn calculate_volatility(&self) -> Option<f64> {
        if self.window.len() < 10 {
            return None;
        }

        let prices: Vec<f64> = self.window.iter().map(|(_, price)| *price).collect();
        let mean: f64 = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance: f64 = prices.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / prices.len() as f64;

        Some(variance.sqrt())
    }

    pub fn calculate_volatility_percentage(&self) -> Option<f64> {
        if let Some(volatility) = self.calculate_volatility() {
            if self.window.len() < 10 {
                return None;
            }
            
            let prices: Vec<f64> = self.window.iter().map(|(_, price)| *price).collect();
            let mean: f64 = prices.iter().sum::<f64>() / prices.len() as f64;
            
            if mean > 0.0 {
                Some((volatility / mean) * 100.0)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn sample_count(&self) -> usize {
        self.window.len()
    }

    pub fn window_duration(&self) -> Option<Duration> {
        if self.window.len() < 2 {
            return None;
        }
        
        if let (Some((first_time, _)), Some((last_time, _))) = (self.window.front(), self.window.back()) {
            last_time.duration_since(*first_time).ok()
        } else {
            None
        }
    }
}
