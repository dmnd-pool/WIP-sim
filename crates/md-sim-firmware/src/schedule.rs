//! Mining budgets use both programmed ASIC and pool targets. Bus timing stays separate.
use anyhow::{ensure, Result};
use md_sim_mining::{
    stratum::{difficulty_target, difficulty_target_scaled, DifficultyScale},
    Filter,
};
use serde::Serialize;

#[derive(Serialize)]
pub struct MiningSchedule {
    pub exponents: [i32; 2],
    pub expected_device_intervals_seconds: [f64; 2],
    pub expected_share_intervals_seconds: [f64; 2],
    pub timeout_seconds: u64,
}

pub fn automatic_exponent(cap: f64) -> i32 {
    (-20.0 + (cap / 10_000.0).log2().floor()).clamp(-31.0, -20.0) as i32
}

pub fn mining_schedule(cap: f64, exponent: i32) -> Result<MiningSchedule> {
    ensure!(
        cap.is_finite() && cap > 0.0 && cap <= 10_000.0,
        "invalid hash cap"
    );
    ensure!(
        (-31..=11).contains(&exponent),
        "difficulty exponent must be -31 through 11"
    );
    let mut schedule = MiningSchedule {
        exponents: [exponent, exponent + 1],
        expected_device_intervals_seconds: [0.0; 2],
        expected_share_intervals_seconds: [0.0; 2],
        timeout_seconds: 0,
    };
    for (i, exponent) in schedule.exponents.iter().enumerate() {
        let device = difficulty_target(*exponent).map_err(anyhow::Error::msg)?;
        let pool = difficulty_target_scaled(*exponent, DifficultyScale::ThreeHalves)
            .map_err(anyhow::Error::msg)?;
        schedule.expected_device_intervals_seconds[i] = Filter::Target(device)
            .expected_interval_seconds(device, cap)
            .map_err(anyhow::Error::msg)?;
        schedule.expected_share_intervals_seconds[i] = Filter::Target(device)
            .expected_interval_seconds(pool, cap)
            .map_err(anyhow::Error::msg)?;
    }
    let seconds = (5.0
        + 12.0
            * schedule
                .expected_share_intervals_seconds
                .iter()
                .sum::<f64>())
    .ceil()
    .max(30.0);
    ensure!(seconds <= 86_390.0, "expected work exceeds the one-day process budget; lower the difficulty exponent or increase the cap");
    schedule.timeout_seconds = seconds as u64;
    Ok(schedule)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lower_caps_reduce_default_difficulty_and_budget_uses_both_targets() {
        assert_eq!(automatic_exponent(10_000.0), -20);
        assert_eq!(automatic_exponent(1_000.0), -24);
        let a = mining_schedule(1000.0, -20).unwrap();
        let b = mining_schedule(100.0, -20).unwrap();
        assert!(
            (b.expected_share_intervals_seconds[0] / a.expected_share_intervals_seconds[0] - 10.0)
                .abs()
                < 1e-9
        );
        assert!(
            (a.expected_share_intervals_seconds[0] / a.expected_device_intervals_seconds[0] - 1.5)
                .abs()
                < 1e-9
        );
        assert!(b.timeout_seconds > a.timeout_seconds);
        assert!(mining_schedule(0.000001, -20).is_err());
    }
}
