//! Execute the real NSS/OpenSSL profile import in a private filesystem and network namespace.
use anyhow::{ensure, Result};
use md_sim_access::credentials::{
    prepare_browser_profile, BrowserProfileTool, BrowserProfileTools,
};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;

fn inside(vm: bool) -> Result<()> {
    if vm {
        ensure!(
            unsafe { libc::geteuid() } == 0
                && fs::read_to_string("/proc/cmdline")?
                    .split_whitespace()
                    .any(|w| w == "md_sim_private_guest=1"),
            "NSS VM setup requires private guest root"
        );
        let interfaces = fs::read_dir("/sys/class/net")?
            .map(|e| Ok(e?.file_name()))
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            interfaces == [std::ffi::OsString::from("lo")],
            "unexpected NSS VM interface"
        );
        ensure!(
            unsafe { libc::chown(c"/work".as_ptr(), 1000, 1000) } == 0
                && unsafe { libc::setgroups(0, std::ptr::null()) } == 0
                && unsafe { libc::setgid(1000) } == 0
                && unsafe { libc::setuid(1000) } == 0
                && unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } == 0,
            "NSS VM privilege drop failed"
        );
        ensure!(
            fs::read_to_string("/proc/self/status")?.contains("CapEff:\t0000000000000000"),
            "NSS VM capabilities remain"
        );
    } else {
        confinement::require(true)?;
    }
    let pinned = |path: &str| -> Result<BrowserProfileTool> {
        Ok(BrowserProfileTool {
            program: path.into(),
            object: hash_file(Path::new(path))?,
        })
    };
    let tools = BrowserProfileTools {
        openssl: pinned("/usr/bin/openssl")?,
        certutil: pinned("/usr/bin/certutil")?,
        pk12util: pinned("/usr/bin/pk12util")?,
    };
    let run_id = "17".repeat(32);
    let credentials = Path::new("/work/credentials");
    let public = md_sim_access::credentials::create(
        &tools.openssl.program,
        credentials,
        "192.0.2.1".parse()?,
        &run_id,
    )?;
    let profile = Path::new("/work/profile");
    fs::create_dir(profile)?;
    fs::set_permissions(profile, fs::Permissions::from_mode(0o700))?;
    fs::write(profile.join("user.js"), "// private test profile\n")?;
    let report = prepare_browser_profile(
        &tools,
        profile,
        &credentials.join("ca.crt"),
        &credentials.join("client.crt"),
        &credentials.join("client.key"),
        &run_id,
    )?;
    ensure!(
        !profile.join(".md-sim-import").exists(),
        "private import directory remains"
    );
    ensure!(
        prepare_browser_profile(
            &tools,
            profile,
            &credentials.join("ca.crt"),
            &credentials.join("client.crt"),
            &credentials.join("client.key"),
            &run_id
        )
        .is_err(),
        "existing NSS database was reused"
    );
    let invalid = Path::new("/work/wrong-key-profile");
    fs::create_dir(invalid)?;
    fs::set_permissions(invalid, fs::Permissions::from_mode(0o700))?;
    ensure!(
        prepare_browser_profile(
            &tools,
            invalid,
            &credentials.join("ca.crt"),
            &credentials.join("client.crt"),
            &credentials.join("server.key"),
            &run_id
        )
        .is_err(),
        "unrelated private key was accepted"
    );
    ensure!(
        !invalid.join(".md-sim-import").exists(),
        "failed import left private temporary files"
    );
    println!(
        "{}",
        json!({"schema":"md-sim-browser-profile-check/1","passed":true,"credentials":public,"profile":report,
        "existing_profile_rejected":true,"wrong_private_key_rejected":true,"failure_private_temporaries_removed":true,
        "browser_execution_gate":false,"native_dashboard_gate":false})
    );
    Ok(())
}

fn host(output: &Path) -> Result<()> {
    ensure!(!output.exists(), "output directory must be new");
    fs::create_dir_all(output)?;
    save_json(
        &output.join("assessment.json"),
        &json!({"passed":false,"stage":"capture"}),
    )?;
    fs::write(
        output.join("driver-source.rs"),
        include_bytes!("browser_profile_check.rs"),
    )?;
    let store = Store::open(&output.join("inputs"), 256 << 20)?;
    let mut roots = vec![("/probe".into(), std::env::current_exe()?)];
    for path in [
        "/usr/bin/openssl",
        "/usr/bin/certutil",
        "/usr/bin/pk12util",
        "/usr/lib/libsoftokn3.so",
        "/usr/lib/libfreeblpriv3.so",
        "/usr/lib/libsqlite3.so.0",
    ] {
        roots.push((path.into(), PathBuf::from(path)));
    }
    let inputs = md_sim_firmware::lab::elf_closure(&store, roots)?;
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 90,
            cpu_seconds: 30,
            memory_bytes: 512 << 20,
            processes: 32,
            file_bytes: 8 << 20,
            trace_bytes: 2 << 20,
            tmp_bytes: 64 << 20,
        },
    };
    save_json(&output.join("process-spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&output.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    fs::write(output.join("stdout.log"), &stdout)?;
    fs::write(output.join("stderr.log"), hex::decode(&result.stderr)?)?;
    ensure!(
        result.status == "exited"
            && result.exit_code == Some(0)
            && !result.trace_truncated
            && result.stderr.is_empty(),
        "isolated NSS execution failed"
    );
    let report: Value = serde_json::from_slice(&stdout)?;
    ensure!(
        report["schema"] == "md-sim-browser-profile-check/1"
            && report["passed"] == true
            && report["existing_profile_rejected"] == true
            && report["wrong_private_key_rejected"] == true
            && report["failure_private_temporaries_removed"] == true,
        "NSS behavioral assertions absent"
    );
    save_json(&output.join("report.json"), &report)?;
    save_json(
        &output.join("assessment.json"),
        &json!({"passed":true,"spec":hash_file(&output.join("process-spec.json"))?,"result":hash_file(&output.join("result.json"))?,"report":hash_file(&output.join("report.json"))?,
        "driver_source":hash_file(&output.join("driver-source.rs"))?,"scope":"Actual support-tool NSS credential import in private user/network/mount/PID namespace; no browser or firmware acceptance"}),
    )?;
    Ok(())
}
fn main() -> Result<()> {
    let argument = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: browser_profile_check NEW_OUTPUT"))?;
    if argument == "__inside" {
        return inside(false);
    }
    if argument == "__guest" {
        return inside(true);
    }
    let output = Path::new(&argument);
    let new = !output.exists();
    let result = host(output);
    if let Err(error) = &result {
        if new && output.is_dir() {
            save_json(
                &output.join("failure.json"),
                &json!({"passed":false,"error":format!("{error:#}")}),
            )?;
        }
    }
    result
}
