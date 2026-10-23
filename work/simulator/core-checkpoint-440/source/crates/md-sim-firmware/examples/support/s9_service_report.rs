//! Independent checks of the retained native HTTP observations. This does not
//! classify a service diagnostic as a full board, browser or remote test.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{save_json, ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub const SCRIPT: &[u8] = include_bytes!("../../../../simulator/firmware/s9-luci-service-check.sh");
const ASSET_SHA: &str = "8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477";

fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "evidence must be a regular file"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "evidence size exceeds limit");
    Ok(bytes)
}
fn identity(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: hex::encode(Sha256::digest(bytes)),
        size: bytes.len() as u64,
    }
}
fn exact_line<'a>(lines: &'a [&'a str], prefix: &str) -> Result<&'a str> {
    let mut found = lines.iter().filter_map(|line| line.strip_prefix(prefix));
    let value = found
        .next()
        .context(format!("missing service evidence {prefix}"))?;
    ensure!(
        found.next().is_none(),
        "duplicate service evidence {prefix}"
    );
    Ok(value)
}

pub fn report(run: &Path, output: &Path) -> Result<Value> {
    ensure!(!output.exists(), "service report must be new");
    let spec_bytes = bounded(&run.join("spec.json"), 2 * 1024 * 1024)?;
    let spec: ProcessSpec = serde_json::from_slice(&spec_bytes)?;
    spec.validate()?;
    ensure!(
        spec.argv == ["/probe", "__serial"],
        "not a declared serial service run"
    );
    ensure!(
        spec.stdin_object.as_ref() == Some(&identity(SCRIPT)),
        "service script identity differs"
    );
    for (name, hash) in super::COMPONENTS {
        ensure!(
            spec.inputs
                .iter()
                .any(|m| m.guest_path == format!("/{name}") && m.object.sha256 == *hash),
            "firmware component identity differs"
        );
    }
    let verifier = hash_file(&std::env::current_exe()?)?;
    ensure!(
        spec.inputs
            .iter()
            .any(|m| m.guest_path == "/probe" && m.object == verifier),
        "service driver differs from this tested verifier build"
    );
    let result_bytes = bounded(&run.join("result.json"), 40 * 1024 * 1024)?;
    let result: ProcessResult = serde_json::from_slice(&result_bytes)?;
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "service diagnostic did not pass process bounds"
    );
    let console_bytes = bounded(&run.join("stdout.bin"), 8 * 1024 * 1024)?;
    ensure!(
        hex::decode(result.stdout)? == console_bytes,
        "retained console differs from result"
    );
    let backend_bytes = bounded(&run.join("stderr.log"), 8 * 1024 * 1024)?;
    ensure!(
        hex::decode(result.stderr)? == backend_bytes,
        "retained backend log differs from result"
    );
    let text = std::str::from_utf8(&console_bytes)?;
    let lines: Vec<_> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    ensure!(
        exact_line(&lines, "MD_SIM_SERIAL_SCRIPT_FINISHED:")? == "0",
        "guest script reported failure"
    );
    ensure!(
        lines.contains(&"NATIVE_LUCI_LOGIN_ASSET_LIVE_CONFIG_RESTORE_SERVICE_RESTART_COMPLETE"),
        "guest assertions did not finish"
    );
    let mut http = Vec::new();
    let mut rpc = Vec::new();
    for line in &lines {
        if let Some(row) = line.strip_prefix("NATIVE_HTTP\t") {
            let cols: Vec<_> = row.split('\t').collect();
            ensure!(
                cols.len() == 6 && cols[1] == "1",
                "malformed native HTTP observation"
            );
            http.push(json!({"path":cols[0],"status":cols[2].parse::<u16>()?,"status_line":cols[3],"headers":serde_json::from_str::<Value>(cols[4])?,"body_bytes":cols[5].parse::<u64>()?}));
        }
        if let Some(row) = line.strip_prefix("NATIVE_RPC\t") {
            let cols: Vec<_> = row.splitn(3, '\t').collect();
            ensure!(cols.len() == 3, "malformed native RPC observation");
            let response: Value = serde_json::from_str(cols[2])?;
            ensure!(response["error"].is_null(), "native RPC returned an error");
            rpc.push(json!({"service":cols[0],"method":cols[1],"response":response}));
        }
    }
    ensure!(
        http.len() == 15 && rpc.len() == 9,
        "native transaction count differs"
    );
    for (index, path, status) in [
        (0, "/cgi-bin/luci/", 403),
        (1, "/cgi-bin/luci/", 302),
        (2, "/cgi-bin/luci/admin/status/overview", 200),
        (3, "/luci-static/bootstrap/cascade.css", 200),
    ] {
        ensure!(
            http[index]["path"] == path && http[index]["status"] == status,
            "native HTTP stage {index} differs"
        );
    }
    ensure!(
        http[1]["headers"]["location"] == "/cgi-bin/luci/"
            && http[1]["headers"]["set-cookie"]
                .as_str()
                .is_some_and(|s| s.starts_with("sysauth=") && s.ends_with("; path=/cgi-bin/luci/")),
        "native login redirect/cookie differs"
    );
    ensure!(
        http.iter().skip(2).all(|r| r["status"] == 200),
        "authenticated HTTP operation failed"
    );
    let asset = hex::decode(exact_line(&lines, "NATIVE_ASSET_HEX\t")?)?;
    ensure!(
        asset.len() == 50527
            && identity(&asset).sha256 == ASSET_SHA
            && http[3]["body_bytes"] == 50527,
        "served asset differs from exact archived stylesheet"
    );
    let uptime: Value = serde_json::from_str(exact_line(&lines, "UPTIME\t")?)?;
    ensure!(
        uptime["id"] == 2
            && uptime["error"].is_null()
            && uptime["result"].as_u64().is_some_and(|s| s > 0),
        "live firmware uptime is missing"
    );
    let restart: Vec<_> = exact_line(&lines, "NATIVE_SERVICE_RESTART\t")?
        .split('\t')
        .collect();
    ensure!(
        restart.len() == 2
            && restart[0] != restart[1]
            && restart
                .iter()
                .all(|s| s.parse::<u32>().is_ok_and(|pid| pid > 1)),
        "service restart did not change PID"
    );
    let methods = [
        ("fs", "readfile"),
        ("uci", "get_all"),
        ("uci", "set"),
        ("uci", "commit"),
        ("uci", "get"),
        ("uci", "get"),
        ("fs", "writefile"),
        ("fs", "readfile"),
        ("uci", "get"),
    ];
    for (index, (service, method)) in methods.iter().enumerate() {
        ensure!(
            rpc[index]["service"] == *service
                && rpc[index]["method"] == *method
                && rpc[index]["response"]["id"] == index + 3,
            "native RPC order differs"
        );
    }
    let result_at = |n: usize| &rpc[n]["response"]["result"];
    ensure!(
        result_at(0).as_str().is_some_and(|s| s.len() > 100) && result_at(0) == result_at(7),
        "configuration restore did not preserve original bytes"
    );
    ensure!(
        result_at(1)["max_connections"] == "100"
            && result_at(2) == true
            && result_at(3) == true
            && result_at(4) == "101"
            && result_at(5) == "101"
            && result_at(6) == 1
            && result_at(8) == "100",
        "configuration write/readback/restart/restoration differs"
    );
    let report = json!({
        "schema":"md-sim-native-service-diagnostic/1","scope":"archived native services in an adapted board environment",
        "firmware_artifact":"braiins-feed-25bba3b4a505ccbc0e54","archive_sha256":"469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e",
        "input_spec":identity(&spec_bytes),"process_result":identity(&result_bytes),"console":identity(&console_bytes),"backend_log":identity(&backend_bytes),"test_script":identity(SCRIPT),"verifier":verifier,
        "execution_inputs":spec.inputs,"isolation_tools":result.tools,
        "passed_component_checks":["unauthenticated login page","native login and redirect","authenticated overview","exact native stylesheet bytes","live firmware uptime","configuration write and commit","configuration readback","web service restart and readback","exact configuration restoration"],
        "asset":identity(&asset),"uptime":uptime,"service_pids":restart,"http":http,"rpc":rpc,
        "dashboard_source":"adapted-native","topology":"HTTP client and original services inside one isolated emulated ARM guest; guest loopback only; no host network",
        "adaptations":["direct Linux loader","single CPU","unmodeled XIIC driver disabled","init shell override","manual service startup","temporary writable /etc and /dev","kernel console level 1 during HTTP observations; full kernel ring retained after test"],
        "incomplete_gates":["original board boot","exact physical target","mining telemetry","browser JavaScript execution","full guest restart persistence","SSH","TLS gateway","separate remote client"],
        "full_native_dashboard_gate":"incomplete"
    });
    save_json(output, &report)?;
    Ok(report)
}
