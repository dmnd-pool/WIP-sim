//! Versioned native settings test input. No miner or API implementation.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub version: u32,
    pub action: Action,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Write { theme: String },
    Restore { original: Value, changed: Value },
    Verify { original: Value },
}

pub fn storage(value: &Value) -> Result<()> {
    let fields = value.as_object().context("native storage object absent")?;
    ensure!(fields.len() == 3, "native storage fields differ");
    for name in ["webTheme", "dashboardDensity", "preferredLanguage"] {
        ensure!(
            fields.get(name).is_some_and(
                |v| v.is_null() || v.as_str().is_some_and(|s| !s.is_empty() && s.len() <= 64)
            ),
            "native storage field absent or invalid: {name}"
        );
    }
    Ok(())
}

pub fn parse(bytes: &[u8]) -> Result<Plan> {
    ensure!(bytes.len() <= 4096, "dashboard state plan bound");
    let plan: Plan = serde_json::from_slice(bytes)?;
    ensure!(plan.version == 1, "dashboard state plan version differs");
    match &plan.action {
        Action::Write { theme } => ensure!(
            matches!(theme.as_str(), "DARK" | "LIGHT"),
            "test theme differs"
        ),
        Action::Restore { original, changed } => {
            storage(original)?;
            storage(changed)?;
            let mut expected = original.clone();
            ensure!(
                matches!(changed["webTheme"].as_str(), Some("DARK" | "LIGHT")),
                "changed theme differs"
            );
            expected["webTheme"] = changed["webTheme"].clone();
            ensure!(
                expected == *changed && original != changed,
                "only a changed test theme may be restored"
            );
        }
        Action::Verify { original } => storage(original)?,
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn restart_plan_rejects_missing_state_and_changes_to_other_fields() {
        let original = json!({"webTheme":null,"dashboardDensity":null,"preferredLanguage":null});
        let mut changed = original.clone();
        changed["webTheme"] = json!("DARK");
        let mut plan =
            json!({"version":1,"action":{"phase":"restore","original":original,"changed":changed}});
        parse(&serde_json::to_vec(&plan).unwrap()).unwrap();
        plan["action"]["changed"]["preferredLanguage"] = json!("en");
        assert!(parse(&serde_json::to_vec(&plan).unwrap()).is_err());
        assert!(parse(br#"{"version":1,"action":{"phase":"verify","original":{}}}"#).is_err());
        assert!(parse(br#"{"version":2,"action":{"phase":"write","theme":"DARK"}}"#).is_err());
    }
}
