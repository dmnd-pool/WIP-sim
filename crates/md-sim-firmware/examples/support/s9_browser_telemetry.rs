//! Native Boser fields from the pinned 2022 application and dashboard module.
//! These assertions describe the explicit S9 reference fixture, not any
//! physical miner or a functional hash-rate oracle.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;

pub const QUERY: &str = r#"subscription {
    bosminerInfo { __typename ... on BosminerInfoSample {
        timestamp
        fans { name rpm speed }
        tempCtrl { hotC targetC dangerousC }
        summary { poolStatus foundBlocks shares { ...Shares }
            temperature { name degreesC }
            realHashrate { mhs1M mhs5S } }
        poolGroups { name pools { active url user status generatedWork lastDifficulty shares { ...Shares } } }
        workSolver { name hwDetails { chips frequencyMhz voltageV }
            childSolvers { name hwDetails { chips frequencyMhz voltageV }
                temperatures { name degreesC } tuner { status }
                realHashrate { mhs1M mhs5S } } }
    } }
} fragment Shares on SharesInfo { acceptedSolutions rejectedSolutions staleSolutions }
"#;

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value[field]
        .as_array()
        .with_context(|| format!("native {field} array absent"))
}
fn number(value: &Value, field: &str) -> Result<f64> {
    let number = value[field]
        .as_f64()
        .with_context(|| format!("native {field} number absent"))?;
    ensure!(number.is_finite(), "nonfinite native {field}");
    Ok(number)
}

pub fn assess(sample: &Value) -> Result<Value> {
    ensure!(
        sample["__typename"] == "BosminerInfoSample",
        "native telemetry is not available"
    );
    let boards = array(&sample["workSolver"], "childSolvers")?;
    ensure!(
        boards.len() == 3,
        "native board count differs from the reference topology"
    );
    let mut addresses = BTreeSet::new();
    for board in boards {
        let address = board["name"]
            .as_str()
            .context("native board address absent")?;
        ensure!(
            addresses.insert(address) && matches!(address, "6" | "7" | "8"),
            "native board addresses differ or repeat"
        );
        ensure!(
            board["hwDetails"]["chips"] == 63
                && number(&board["hwDetails"], "frequencyMhz")? == 100.0,
            "native chip count or configured clock differs"
        );
        ensure!(
            number(&board["hwDetails"], "voltageV")? > 0.0,
            "native voltage readback is not positive"
        );
        ensure!(
            board["tuner"]["status"] == "DISABLED",
            "native tuner differs from the run configuration"
        );
        let temperatures = array(board, "temperatures")?;
        ensure!(
            temperatures.len() == 2,
            "native temperature channel count differs"
        );
        for (name, expected) in [("Board", 25.0), ("Chip", 65.0)] {
            let matching: Vec<_> = temperatures.iter().filter(|t| t["name"] == name).collect();
            ensure!(
                matching.len() == 1 && number(matching[0], "degreesC")? == expected,
                "native {name} temperature differs from the explicit fixture"
            );
        }
        for field in ["mhs1M", "mhs5S"] {
            ensure!(
                number(&board["realHashrate"], field)? >= 0.0,
                "negative native rate estimate"
            );
        }
    }
    let fans = array(sample, "fans")?;
    ensure!(fans.len() == 4, "native fan channel count differs");
    let names: BTreeSet<_> = fans.iter().map(|f| f["name"].as_str()).collect();
    ensure!(
        names == BTreeSet::from([Some("0"), Some("1"), Some("2"), Some("3")]),
        "native fan addresses differ"
    );
    for fan in fans {
        ensure!(
            (0.0..=100_000.0).contains(&number(fan, "rpm")?)
                && (0.0..=100.0).contains(&number(fan, "speed")?),
            "native fan data outside protocol range"
        );
    }
    let thermal = &sample["tempCtrl"];
    ensure!(
        number(thermal, "targetC")? < number(thermal, "hotC")?
            && number(thermal, "hotC")? < number(thermal, "dangerousC")?,
        "native thermal thresholds are not ordered"
    );
    let pools = array(sample, "poolGroups")?
        .iter()
        .map(|g| array(g, "pools"))
        .collect::<Result<Vec<_>>>()?;
    let matching: Vec<_> = pools
        .into_iter()
        .flatten()
        .filter(|p| p["url"] == "stratum+tcp://10.0.2.100:3333" && p["user"] == "simulator")
        .collect();
    ensure!(
        matching.len() == 1,
        "selected private pool is absent or ambiguous in native telemetry"
    );
    let pool = matching[0];
    ensure!(
        pool["active"] == true && pool["status"] == "ALIVE",
        "native selected pool is not active"
    );
    let shares = &pool["shares"];
    let accepted = shares["acceptedSolutions"]
        .as_u64()
        .context("native accepted count absent")?;
    ensure!(accepted > 0, "native pool has no accepted solutions");
    for field in ["acceptedSolutions", "rejectedSolutions", "staleSolutions"] {
        ensure!(
            shares[field].as_u64().is_some() && shares[field] == sample["summary"]["shares"][field],
            "native summary and selected pool share counts differ"
        );
    }
    Ok(
        json!({"boards":addresses,"chips_per_board":63,"frequency_mhz":100,
        "board_temperature_c":25,"chip_temperature_c":65,"fan_channels":4,
        "accepted_solutions":accepted,"selected_pool":pool["url"],
        "scope":"Live firmware telemetry for the recorded reference fixture. Rate estimates retain the firmware's production scale and are not functional hash-rate evidence. Physical sensor calibration and fault response remain separate tests."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_native_sample_checks_topology_sensors_and_pool_accounting() {
        let fixture: Value = serde_json::from_slice(include_bytes!(
            "../../../../fixtures/native-s9/boser-sample-050.json"
        ))
        .unwrap();
        let sample = fixture["sample"].clone();
        assert_eq!(assess(&sample).unwrap()["accepted_solutions"], 16);
        for mode in 0..9 {
            let mut changed = sample.clone();
            match mode {
                0 => changed["workSolver"]["childSolvers"][0]["hwDetails"]["chips"] = json!(62),
                1 => changed["workSolver"]["childSolvers"][1]["name"] = json!("6"),
                2 => {
                    changed["workSolver"]["childSolvers"][0]["temperatures"][0]["degreesC"] =
                        Value::Null
                }
                3 => changed["summary"]["shares"]["acceptedSolutions"] = json!(17),
                4 => {
                    changed["poolGroups"][0]["pools"][0]["url"] =
                        json!("stratum+tcp://192.0.2.1:3333")
                }
                5 => changed["poolGroups"][0]["pools"][0]["active"] = json!(false),
                6 => changed["fans"][1]["name"] = json!("0"),
                7 => changed["tempCtrl"]["targetC"] = json!(120),
                _ => {
                    changed["poolGroups"][0]["pools"][0]["shares"]["acceptedSolutions"] = json!(0);
                    changed["summary"]["shares"]["acceptedSolutions"] = json!(0);
                }
            }
            assert!(assess(&changed).is_err(), "changed native sample {mode}");
        }
    }
}
