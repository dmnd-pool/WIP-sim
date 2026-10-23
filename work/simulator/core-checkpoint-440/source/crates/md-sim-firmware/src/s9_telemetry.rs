//! Validate native uptime samples without assuming host and guest clock rates.
use anyhow::{ensure, Context, Result};
use serde_json::Value;

/// False means the same valid whole-second sample. A reboot, backward time,
/// changed host or invalid value is an error, never a reason to retry.
pub fn uptime_progress(first: &Value, next: &Value) -> Result<bool> {
    let number = |value: &Value| -> Result<f64> {
        let value = value["uptime"]["durationS"]
            .as_f64()
            .context("native uptime missing")?;
        ensure!(
            value.is_finite() && (0.0..=9007199254740991.0).contains(&value),
            "native uptime invalid"
        );
        Ok(value)
    };
    ensure!(
        first["hostname"].as_str().is_some_and(|s| !s.is_empty())
            && first["hostname"] == next["hostname"],
        "native hostname changed or absent"
    );
    ensure!(
        !first["uptime"]["since"].is_null() && first["uptime"]["since"] == next["uptime"]["since"],
        "native boot identity changed or absent"
    );
    let (before, after) = (number(first)?, number(next)?);
    ensure!(after >= before, "native uptime went backward");
    Ok(after > before)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn equal_samples_are_pending_but_reboot_regression_and_invalid_values_fail() {
        let first = json!({"hostname":"miner","uptime":{"since":100,"durationS":5}});
        assert!(!uptime_progress(&first, &first).unwrap());
        let mut next = first.clone();
        next["uptime"]["durationS"] = json!(6);
        assert!(uptime_progress(&first, &next).unwrap());
        for value in [
            json!(-1),
            json!(4),
            json!(null),
            json!("6"),
            json!(9007199254740992_u64),
        ] {
            next["uptime"]["durationS"] = value;
            assert!(uptime_progress(&first, &next).is_err());
        }
        next = first.clone();
        next["uptime"]["since"] = json!(101);
        assert!(uptime_progress(&first, &next).is_err());
        next = first.clone();
        next["hostname"] = json!("another");
        assert!(uptime_progress(&first, &next).is_err());
        next = first.clone();
        next["uptime"]["since"] = Value::Null;
        assert!(uptime_progress(&next, &next).is_err());
    }
}
