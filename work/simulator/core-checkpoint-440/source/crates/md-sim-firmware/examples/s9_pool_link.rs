//! Actual Ethernet/TCP transport test in two disconnected local Linux VMs.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::net::UnixStream,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[allow(dead_code)]
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[allow(dead_code)]
#[path = "support/s9_pool_report.rs"]
mod pool_report;
#[allow(dead_code)]
#[path = "support/pool_vm.rs"]
mod pool_vm;

fn inside() -> Result<()> {
    confinement::require_board()?;
    let (server_fd, client_fd) = UnixStream::pair()?;
    let mut server = pool_vm::start(server_fd, false)?;
    let mut client = pool_vm::start(client_fd, true)?;
    let deadline = Instant::now() + Duration::from_secs(55);
    let (server_status, client_status) = loop {
        let s = server.child.try_wait()?;
        let c = client.child.try_wait()?;
        if s.is_some() && c.is_some() {
            break (s, c);
        }
        if Instant::now() >= deadline {
            break (s, c);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    println!(
        "MD_SIM_POOL_LINK_EXPORT {}",
        json!({
            "server":pool_vm::export(&server)?,"client":pool_vm::export(&client)?,
            "server_status":server_status.map(|s|s.to_string()),
            "client_status":client_status.map(|s|s.to_string()),
        })
    );
    ensure!(
        server_status.is_some_and(|s| s.success()) && client_status.is_some_and(|s| s.success()),
        "private pool transport VM failed or exceeded deadline"
    );
    Ok(())
}

fn host(out: &Path, backend: &Path) -> Result<()> {
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(out)?;
    let input = out.join("pool-input.json");
    save_json(&input, &pool_input::prepare(20, [-28, -27], None)?)?;
    let image = out.join("image");
    pool_vm::prepare(
        &image,
        &input,
        pool_vm::DashboardSuite::None,
        None,
        None,
        None,
    )?;
    let store = Store::open(&out.join("input-store"), 64 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/runner".into(), std::env::current_exe()?)],
    )?;
    pool_vm::capture(&store, &mut inputs, &image, backend, false)?;
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/runner".into(), "__inside".into()],
        inputs,
        environment: BTreeMap::new(),
        stdin_object: None,
        limits: Limits {
            wall_seconds: 60,
            cpu_seconds: 60,
            memory_bytes: 3 * 1024 * 1024 * 1024,
            processes: 64,
            file_bytes: 16 * 1024 * 1024,
            trace_bytes: 16 * 1024 * 1024,
            tmp_bytes: 32 * 1024 * 1024,
        },
    };
    save_json(&out.join("process-spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let text = String::from_utf8(hex::decode(&result.stdout)?)?;
    let exports = pool_vm::markers(&text, "MD_SIM_POOL_LINK_EXPORT ")?;
    ensure!(exports.len() == 1, "VM logs export missing");
    let export = &exports[0];
    for role in ["server", "client"] {
        fs::write(
            out.join(format!("{role}.pcap")),
            hex::decode(
                export[role]["pcap_hex"]
                    .as_str()
                    .context("packet capture missing")?,
            )?,
        )?;
        for (field, extension) in [("stdout", "log"), ("stderr", "stderr")] {
            fs::write(
                out.join(format!("{role}.{extension}")),
                export[role][field].as_str().context("VM output missing")?,
            )?;
        }
        ensure!(
            export[role]["pcap_identity"]
                == serde_json::to_value(hash_file(&out.join(format!("{role}.pcap")))?)?,
            "exported packet capture identity differs"
        );
    }
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated && result.stderr.is_empty(),
        "private VM transport execution failed; see saved VM logs"
    );
    let server = export["server"]["stdout"]
        .as_str()
        .context("server log missing")?;
    let client = export["client"]["stdout"]
        .as_str()
        .context("client log missing")?;
    for (role, log) in [("server", server), ("client", client)] {
        pool_vm::require_guest_success(
            log,
            export[role]["stderr"]
                .as_str()
                .context("VM stderr missing")?,
        )?;
    }
    let sessions = pool_vm::markers(server, "MD_SIM_PRIVATE_POOL_RESULT ")?;
    let collection = pool_vm::markers(server, "MD_SIM_PRIVATE_POOL_COLLECTION ")?;
    let clients = pool_vm::markers(client, "MD_SIM_POOL_LINK_CLIENT ")?;
    ensure!(
        sessions.len() == 33 && collection.len() == 1 && clients.len() == 1,
        "connection evidence count differs"
    );
    let input = pool_input::read(&input)?;
    pool_report::require_collection(&sessions, &collection[0], input.session_limit()?)?;
    for session in &sessions {
        let id = usize::try_from(
            session["session_id"]
                .as_u64()
                .context("session ID absent")?,
        )?;
        ensure!(
            session["seed_hex"] == hex::encode(input.session_seed(id)?),
            "connection seed differs from frozen input"
        );
    }
    let v1 = sessions
        .iter()
        .filter(|s| s["report"]["messages_received"] == 3)
        .count();
    let probe = sessions
        .iter()
        .filter(|s| {
            s["trace"]
                .as_str()
                .is_some_and(|t| t.contains("0900535452320141455347"))
        })
        .count();
    ensure!(
        v1 == 32 && probe == 1,
        "binary probe and V1 sessions were not isolated"
    );
    ensure!(
        clients[0]["connections"]
            .as_array()
            .is_some_and(|c| c.len() == 32),
        "32 V1 reconnects absent"
    );
    ensure!(
        collection[0]["incomplete_sessions"] == json!([]),
        "incomplete pool sessions"
    );
    ensure!(
        collection[0]["sessions_completed"] == 33
            && collection[0]["max_sessions"] == 128
            && sessions
                .iter()
                .all(|s| s["report"]["completed"] == false
                    && s["report"]["submissions"] == json!([])),
        "transport-only test unexpectedly reported mining"
    );
    save_json(
        &out.join("assessment.json"),
        &json!({
            "private_ethernet_tcp":true,"separate_binary_probe":true,"reconnect":true,
            "invalid_submission_rejections":32,"sessions":sessions,"client":clients[0],
            "lifetime_connections":33,"connection_limit":128,
            "process_spec":hash_file(&out.join("process-spec.json"))?,
            "external_physical_host":false,"firmware_mining_gate":false
        }),
    )?;
    println!(
        "{}",
        json!({"private_ethernet_tcp":true,"output":out,"firmware_mining_gate":false})
    );
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.get(1).is_some_and(|s| s == "__inside") {
        return inside();
    }
    ensure!(
        args.len() == 3,
        "usage: s9_pool_link NEW_OUTPUT POOL_BACKEND"
    );
    let out = Path::new(&args[1]);
    let result = host(out, Path::new(&args[2]));
    if let Err(error) = &result {
        if out.is_dir() {
            save_json(
                &out.join("failure.json"),
                &json!({"private_ethernet_tcp":false,"error":format!("{error:#}")}),
            )?;
        }
    }
    result
}
