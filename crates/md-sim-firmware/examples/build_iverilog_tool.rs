//! Build the exact upstream Icarus toolchain in the existing closed preparation runtime.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[allow(dead_code)]
#[path = "support/qemu_build.rs"]
mod qemu_build;
use qemu_build::{pack_dir, step, unpack};
const COMPILER_ARCHIVE: &str = "cc98e84002c7b7fefb0128128e4cf4546d333e780715da16d048e17b73dafaa5";
const COMPILER_MANIFEST: &str = "e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a";
const GPERF: &str = "588546b945bba4b70b6a3a616e80b4ab466e3f33024a352fc2198112cdbb3ae2";
const ICARUS: &str = "690f02e6fd0e0c05e4768bcb65a2c7f1b64d20289344d1ad3ccfb69bae8bd55b";
const ICARUS_DIRECTORY: &str = "iverilog-4fd5291632232fbe1ba49b2c26bb6b2bf1c6c9cf";
const SMOKE: &str = r#"`timescale 1ns/1ps
module smoke;
 reg [31:0] a=32'hffff_ffff; reg [31:0] b=1;
 reg [3:0] unknown=4'bx101; reg clock=0; integer edges=0;
 always @(posedge clock) edges=edges+1;
 initial begin
  #1; if ((a+b)!==32'd0) $fatal(1,"wrap");
  if ((unknown & 4'b0010)!==4'b0000) $fatal(1,"four state mask");
  if ((unknown | 4'b0000)!==4'bx101) $fatal(1,"unknown propagation");
  #2 clock=1; #2 clock=0; #2 clock=1; #1;
  if (edges!==2 || $time!=8) $fatal(1,"event schedule");
  $display("MD_SIM_ICARUS_SMOKE_PASS wrap=0 unknown=x101 edges=2 time=8"); $finish;
 end
endmodule
"#;
fn check_id(path: &Path, expected: &str) -> Result<ObjectId> {
    let id = hash_file(path)?;
    ensure!(
        id.sha256 == expected,
        "exact input differs: {}",
        path.display()
    );
    Ok(id)
}
fn validate_compiler_environment(environment: &BTreeMap<String, String>) -> Result<()> {
    for name in ["CC", "CFLAGS", "LDFLAGS"] {
        let value = environment
            .get(name)
            .filter(|v| !v.is_empty())
            .context("compiler setting absent")?;
        ensure!(
            environment.get(&format!("{name}_FOR_BUILD")) == Some(value),
            "build compiler setting differs: {name}"
        );
    }
    Ok(())
}
fn command(program: &str, args: &[&str], cwd: &Path) -> Command {
    let mut c = Command::new(program);
    c.args(args).current_dir(cwd);
    c
}
fn build(report: &mut Value) -> Result<()> {
    for (path, sha) in [
        ("/toolchain.tar.gz", COMPILER_ARCHIVE),
        ("/gperf.tar.gz", GPERF),
        ("/iverilog.tar.gz", ICARUS),
    ] {
        check_id(Path::new(path), sha)?;
        unpack(path, "/work")?;
    }
    let gperf = Path::new("/work/gperf-3.1");
    ensure!(
        step(
            report,
            "gperf_configure",
            command(
                "/bin/sh",
                &["./configure", "--prefix=/work/gperf-install"],
                gperf
            )
        )?,
        "gperf configure failed"
    );
    ensure!(
        step(
            report,
            "gperf_build",
            command("/work/toolchain/usr/bin/make", &["-j2"], gperf)
        )?,
        "gperf build failed"
    );
    ensure!(
        step(
            report,
            "gperf_check",
            command("/work/toolchain/usr/bin/make", &["-j1", "check"], gperf)
        )?,
        "gperf checks failed"
    );
    ensure!(
        step(
            report,
            "gperf_install",
            command("/work/toolchain/usr/bin/make", &["install"], gperf)
        )?,
        "gperf install failed"
    );
    report["gperf_binary"] =
        serde_json::to_value(hash_file(Path::new("/work/gperf-install/bin/gperf"))?)?;
    let source = Path::new("/work").join(ICARUS_DIRECTORY);
    ensure!(
        step(
            report,
            "icarus_autoconf",
            command("/bin/sh", &["./autoconf.sh"], &source)
        )?,
        "Icarus generators failed"
    );
    ensure!(
        step(
            report,
            "icarus_configure",
            command(
                "/bin/sh",
                &["./configure", "--prefix=/work/iverilog-install"],
                &source
            )
        )?,
        "Icarus configure failed"
    );
    ensure!(
        step(
            report,
            "icarus_build",
            command("/work/toolchain/usr/bin/make", &["-j2"], &source)
        )?,
        "Icarus build failed"
    );
    ensure!(
        step(
            report,
            "icarus_install",
            command("/work/toolchain/usr/bin/make", &["install"], &source)
        )?,
        "Icarus install failed"
    );
    ensure!(
        step(
            report,
            "iverilog_version",
            command("/work/iverilog-install/bin/iverilog", &["-V"], &source)
        )?,
        "Icarus version failed"
    );
    fs::write("/work/smoke.v", SMOKE)?;
    ensure!(
        step(
            report,
            "smoke_compile",
            command(
                "/work/iverilog-install/bin/iverilog",
                &[
                    "-g2012",
                    "-s",
                    "smoke",
                    "-o",
                    "/work/smoke.vvp",
                    "/work/smoke.v"
                ],
                &source
            )
        )?,
        "Icarus smoke compile failed"
    );
    ensure!(
        step(
            report,
            "smoke_execute",
            command(
                "/work/iverilog-install/bin/vvp",
                &["/work/smoke.vvp"],
                &source
            )
        )?,
        "Icarus smoke execution failed"
    );
    let smoke = fs::read_to_string("/work/smoke_execute.stdout.log")?;
    ensure!(
        smoke
            .lines()
            .any(|line| line == "MD_SIM_ICARUS_SMOKE_PASS wrap=0 unknown=x101 edges=2 time=8"),
        "Icarus smoke result absent"
    );
    // Independent failure oracle: malformed HDL must fail actual compilation.
    fs::write("/work/invalid.v", "module invalid; wire x = ; endmodule\n")?;
    ensure!(
        !step(
            report,
            "invalid_compile",
            command(
                "/work/iverilog-install/bin/iverilog",
                &[
                    "-g2012",
                    "-s",
                    "invalid",
                    "-o",
                    "/work/invalid.vvp",
                    "/work/invalid.v"
                ],
                &source
            )
        )?,
        "Icarus accepted invalid syntax"
    );
    let negative = &report["invalid_compile"];
    ensure!(
        negative["code"].as_i64().is_some_and(|code| code != 0)
            && negative["stderr"]
                .as_str()
                .is_some_and(|text| text.contains("syntax error"))
            && negative["stdout_truncated"] == false
            && negative["stderr_truncated"] == false,
        "invalid HDL did not produce an untruncated syntax diagnostic"
    );
    pack_dir(
        Path::new("/work/iverilog-install"),
        "iverilog-install",
        Path::new("/work/iverilog-tools.tar.gz"),
    )?;
    // Check the export within the recorded build runtime closure. Remove the original
    // installation from its compiled-in path before restoring the archive.
    fs::rename(
        "/work/iverilog-install",
        "/work/original-install-unavailable",
    )?;
    unpack("/work/iverilog-tools.tar.gz", "/work")?;
    ensure!(
        step(
            report,
            "restored_compile",
            command(
                "/work/iverilog-install/bin/iverilog",
                &[
                    "-g2012",
                    "-s",
                    "smoke",
                    "-o",
                    "/work/restored.vvp",
                    "/work/smoke.v"
                ],
                &source
            )
        )?,
        "restored tool compile failed"
    );
    ensure!(
        step(
            report,
            "restored_execute",
            command(
                "/work/iverilog-install/bin/vvp",
                &["/work/restored.vvp"],
                &source
            )
        )?,
        "restored tool execution failed"
    );
    ensure!(
        fs::read_to_string("/work/restored_execute.stdout.log")?
            .lines()
            .any(|line| line == "MD_SIM_ICARUS_SMOKE_PASS wrap=0 unknown=x101 edges=2 time=8"),
        "restored smoke result absent"
    );
    report["install_contract"] = json!({"archive_root":"iverilog-install","required_extract_root":"/work","compiled_prefix":"/work/iverilog-install","restore_test":"Original installation renamed away before restoring exported files and repeating compile/execute; recorded build runtime closure remains present.","standalone_runtime_closure":false,"runtime_dependencies":{"compiler_archive_sha256":COMPILER_ARCHIVE,"compiler_manifest_sha256":COMPILER_MANIFEST,"loader_inputs":"Exact ProcessSpec inputs and LD_LIBRARY_PATH remain required until a separate closed runtime test proves a smaller closure."}});
    let id = hash_file(Path::new("/work/iverilog-tools.tar.gz"))?;
    ensure!(id.size <= 24 << 20, "tool output bound");
    report["tools"] = serde_json::to_value(id)?;
    report["tools_hex"] = json!(hex::encode(fs::read("/work/iverilog-tools.tar.gz")?));
    report["smoke_source"] = serde_json::to_value(hash_file(Path::new("/work/smoke.v"))?)?;
    for (name, value) in report.as_object().context("build report object")? {
        if value.get("stdout_truncated").is_some() {
            ensure!(
                value["stdout_truncated"] == false && value["stderr_truncated"] == false,
                "build step diagnostics truncated: {name}"
            );
        }
    }
    Ok(())
}
fn inner() -> Result<()> {
    confinement::require_hdl_tool_build()?;
    validate_compiler_environment(&std::env::vars().collect())?;
    let mut report = json!({"schema":1,"source":{"gperf":GPERF,"iverilog":ICARUS},"claim":"existing HDL compiler build and bounded language smoke tests only; no selected netlist, PS7 behavior or firmware acceptance"});
    let result = build(&mut report);
    report["success"] = json!(result.is_ok());
    if let Err(error) = &result {
        report["error"] = json!(format!("{error:#}"));
        let mut logs = serde_json::Map::new();
        for path in [
            "/work/gperf-3.1/config.log".to_owned(),
            format!("/work/{ICARUS_DIRECTORY}/config.log"),
        ] {
            if Path::new(&path).is_file() {
                let mut bytes = Vec::new();
                fs::File::open(&path)?
                    .take((2 << 20) + 1)
                    .read_to_end(&mut bytes)?;
                let truncated = bytes.len() > 2 << 20;
                bytes.truncate(2 << 20);
                logs.insert(path.clone(),json!({"object":hash_file(Path::new(&path))?,"text":String::from_utf8_lossy(&bytes),"truncated":truncated}));
            }
        }
        report["configuration_logs"] = Value::Object(logs);
    }
    serde_json::to_writer(std::io::stdout(), &report)?;
    std::io::stdout().flush()?;
    result
}
fn add(store: &Store, inputs: &mut Vec<Mount>, path: &Path, guest: &str) -> Result<()> {
    ensure!(
        !inputs.iter().any(|m| m.guest_path == guest),
        "duplicate guest input {guest}"
    );
    let object = hash_file(path)?;
    store.import(path, &object)?;
    inputs.push(Mount {
        object,
        guest_path: guest.into(),
    });
    Ok(())
}
fn snapshot_file(tree: &Path, manifest: &Value, path: &Path) -> Result<ObjectId> {
    let relative = path
        .strip_prefix(tree)?
        .to_str()
        .context("snapshot path UTF-8")?;
    ensure!(
        path.canonicalize()? == tree.canonicalize()?.join(relative),
        "snapshot selected path has a link"
    );
    let stat = fs::symlink_metadata(path)?;
    let entry = &manifest["entries"][relative];
    ensure!(
        stat.is_file()
            && !stat.file_type().is_symlink()
            && entry["kind"] == "file"
            && entry["link"].is_null(),
        "snapshot selected file is not regular"
    );
    let expected: ObjectId =
        serde_json::from_value(entry["object"].clone()).context("snapshot entry object absent")?;
    ensure!(
        hash_file(path)? == expected,
        "selected snapshot file differs from frozen manifest"
    );
    Ok(expected)
}
fn data_tree(
    store: &Store,
    inputs: &mut Vec<Mount>,
    source: &Path,
    guest: &str,
    records: &mut Vec<Value>,
    snapshot: Option<(&Path, &Value)>,
) -> Result<()> {
    let mut entries = fs::read_dir(source)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let name = e
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non UTF-8 tool path"))?;
        let dest = format!("{guest}/{name}");
        let m = fs::symlink_metadata(&p)?;
        ensure!(
            !m.file_type().is_symlink(),
            "tool data symlink requires explicit handling"
        );
        if m.is_dir() {
            data_tree(store, inputs, &p, &dest, records, snapshot)?;
        } else {
            ensure!(m.is_file() && m.len() < 8 << 20, "tool data bound");
            let expected = snapshot
                .map(|(tree, manifest)| snapshot_file(tree, manifest, &p))
                .transpose()?;
            add(store, inputs, &p, &dest)?;
            if let Some(expected) = expected {
                ensure!(
                    inputs.last().context("captured file absent")?.object == expected,
                    "captured snapshot object differs"
                );
            }
            records.push(json!({"source":p,"guest":dest,"object":hash_file(&p)?}));
        }
        ensure!(inputs.len() < 1800, "capture mount bound");
    }
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        return inner();
    }
    ensure!(
        args.len() == 3,
        "usage: build_iverilog_tool REPOSITORY NEW_OUTPUT"
    );
    let repo = PathBuf::from(&args[1]).canonicalize()?;
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "new output required");
    let snapshot = repo.join("work/simulator/firmware/toolchain-snapshot-003");
    let manifest_bytes = fs::read(snapshot.join("manifest.json"))?;
    ensure!(
        md_sim_firmware::prepare::hash_bytes(&manifest_bytes) == COMPILER_MANIFEST,
        "compiler manifest differs"
    );
    let manifest: Value = serde_json::from_slice(&manifest_bytes)?;
    let snapshot_tree = snapshot.join("tree");
    let toolchain = repo.join("work/simulator/qemu-s9/samurai-build-003/toolchain.tar.gz");
    check_id(&toolchain, COMPILER_ARCHIVE)?;
    let gperf = repo.join(
        "artifacts/files/source/source-gperf-3-1-complete-502b/md-sim-gperf-complete-502b.tar.gz",
    );
    check_id(&gperf, GPERF)?;
    let icarus = repo
        .join("artifacts/files/source/source-iverilog-4fd5291/md-sim-iverilog-source-502.tar.gz");
    check_id(&icarus, ICARUS)?;
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 512 << 20)?;
    let bin = snapshot.join("tree/usr/bin");
    let gcc = snapshot.join("tree/usr/lib/gcc/x86_64-pc-linux-gnu/16");
    let mut roots = vec![
        ("/guard".into(), std::env::current_exe()?),
        ("/bin/sh".into(), bin.join("bash")),
        ("/usr/bin/sh".into(), bin.join("bash")),
        (
            "/usr/lib/perl5/5.42/core_perl/CORE/libperl.so".into(),
            snapshot.join("tree/usr/lib/perl5/5.42/core_perl/CORE/libperl.so"),
        ),
        ("/usr/bin/m4".into(), bin.join("m4")),
        ("/usr/bin/env".into(), bin.join("env")),
        ("/usr/bin/bison".into(), PathBuf::from("/usr/bin/bison")),
        ("/usr/bin/flex".into(), PathBuf::from("/usr/bin/flex")),
    ];
    for n in ["gcc", "g++", "as", "ld"] {
        roots.push((format!("/loader-roots/{n}"), bin.join(n)));
    }
    for n in ["cc1", "cc1plus"] {
        roots.push((format!("/loader-roots/{n}"), gcc.join(n)));
    }
    let mut selected_snapshot = vec![];
    for (guest, path) in &roots {
        if path.starts_with(&snapshot_tree) {
            selected_snapshot.push((
                guest.clone(),
                snapshot_file(&snapshot_tree, &manifest, path)?,
            ));
        }
    }
    let mut inputs = md_sim_firmware::lab::elf_closure(&store, roots)?;
    for (guest, expected) in &selected_snapshot {
        ensure!(
            inputs
                .iter()
                .any(|m| m.guest_path == *guest && m.object == *expected),
            "captured compiler root differs from manifest"
        );
    }
    // This exact Perl interpreter has an upstream RUNPATH outside /usr/lib.
    // Bind that manifest-checked path explicitly; the generic ELF policy stays
    // unchanged. libperl and its transitive loader inputs use elf_closure above.
    let perl = bin.join("perl");
    let expected = snapshot_file(&snapshot_tree, &manifest, &perl)?;
    add(&store, &mut inputs, &perl, "/usr/bin/perl")?;
    ensure!(
        inputs.last().context("Perl capture absent")?.object == expected,
        "captured Perl differs"
    );
    selected_snapshot.push(("/usr/bin/perl".into(), expected));
    let mut records = vec![];
    data_tree(
        &store,
        &mut inputs,
        Path::new("/usr/share/bison"),
        "/usr/share/bison",
        &mut records,
        None,
    )?;
    data_tree(
        &store,
        &mut inputs,
        &snapshot.join("tree/usr/share/autoconf"),
        "/usr/share/autoconf",
        &mut records,
        Some((&snapshot_tree, &manifest)),
    )?;
    for (guest, host) in [
        ("/toolchain.tar.gz", toolchain),
        ("/gperf.tar.gz", gperf),
        ("/iverilog.tar.gz", icarus),
        ("/compiler-manifest.json", snapshot.join("manifest.json")),
    ] {
        add(&store, &mut inputs, &host, guest)?;
    }
    ensure!(
        inputs
            .iter()
            .any(|m| m.guest_path == "/compiler-manifest.json"
                && m.object.sha256 == COMPILER_MANIFEST),
        "captured compiler manifest differs"
    );
    let mut packages = vec![];
    for p in fs::read_dir("/var/lib/pacman/local")? {
        let p = p?.path();
        let n = p.file_name().context("package filename")?.to_string_lossy();
        if n.starts_with("bison-") || n.starts_with("flex-") {
            packages.push(json!({"package":n,"desc":fs::read_to_string(p.join("desc"))?,"files":fs::read_to_string(p.join("files"))?,"desc_object":hash_file(&p.join("desc"))?,"files_object":hash_file(&p.join("files"))?}));
        }
    }
    ensure!(packages.len() == 2, "bison/flex package identity ambiguous");
    let environment=BTreeMap::from([
        ("PATH".into(),"/work/gperf-install/bin:/work/toolchain/usr/bin:/usr/bin".into()),
        ("LD_LIBRARY_PATH".into(),"/work/toolchain/usr/lib".into()),
        ("PERL5LIB".into(),"/work/toolchain/usr/share/perl5/core_perl:/work/toolchain/usr/lib/perl5/5.42/core_perl".into()),
        ("AUTOM4TE".into(),"/work/toolchain/usr/bin/autom4te".into()),
        ("CC".into(),"/work/toolchain/usr/bin/gcc --sysroot=/work/toolchain -B/work/toolchain/usr/bin".into()),
        ("CXX".into(),"/work/toolchain/usr/bin/g++ --sysroot=/work/toolchain -B/work/toolchain/usr/bin".into()),
        ("CC_FOR_BUILD".into(),"/work/toolchain/usr/bin/gcc --sysroot=/work/toolchain -B/work/toolchain/usr/bin".into()),
        ("CFLAGS_FOR_BUILD".into(),"-O2 -g0".into()),
        ("LDFLAGS_FOR_BUILD".into(),"--sysroot=/work/toolchain -B/work/toolchain/usr/bin".into()),
        ("CFLAGS".into(),"-O2 -g0".into()),("CXXFLAGS".into(),"-O2 -g0 -std=c++11".into()),
        ("LDFLAGS".into(),"--sysroot=/work/toolchain -B/work/toolchain/usr/bin".into()),
        ("LC_ALL".into(),"C".into()),("TMPDIR".into(),"/tmp".into()),("CONFIG_SHELL".into(),"/bin/sh".into()),
    ]);
    validate_compiler_environment(&environment)?;
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/guard".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment,
        limits: Limits {
            wall_seconds: 1200,
            cpu_seconds: 600,
            memory_bytes: 3 << 30,
            processes: 128,
            file_bytes: 128 << 20,
            trace_bytes: 64 << 20,
            tmp_bytes: 2 << 30,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("input-identities.json"),
        &json!({"compiler_manifest":COMPILER_MANIFEST,"compiler_archive":COMPILER_ARCHIVE,"gperf":GPERF,"iverilog":ICARUS,"recipe":hash_file(&std::env::current_exe()?)?,"data":records,"selected_snapshot_roots":selected_snapshot,"packages":packages,"budget_basis":"Existing compiler snapshot expands698MB; source18MB; two compiler jobs; bounded preparation-only build. No firmware or physical timing claim.","capture":"Frozen compiler archive plus exact bison/flex executables, bison/autoconf data and dynamic loader inputs listed in ProcessSpec. No host tool tree is mounted."}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let raw = hex::decode(&result.stdout)?;
    fs::write(out.join("build-envelope.json"), &raw)?;
    ensure!(!result.trace_truncated, "build trace truncated");
    let mut report: Value =
        serde_json::from_slice(&raw).context("build report absent; raw evidence retained")?;
    if result.exit_code == Some(0) && report["success"] == true {
        let data = hex::decode(report["tools_hex"].as_str().context("tool output absent")?)?;
        let id: ObjectId = serde_json::from_value(report["tools"].clone())?;
        ensure!(
            data.len() as u64 == id.size
                && md_sim_firmware::prepare::hash_bytes(&data) == id.sha256,
            "tool output identity differs"
        );
        fs::write(out.join("iverilog-tools.tar.gz"), data)?;
    }
    report
        .as_object_mut()
        .context("report object")?
        .remove("tools_hex");
    save_json(&out.join("build-report.json"), &report)?;
    ensure!(
        result.exit_code == Some(0) && report["success"] == true,
        "isolated Icarus build failed; evidence retained"
    );
    // Destination verification happens after tool execution as well as before it.
    for m in &spec.inputs {
        store.verified_path(&m.object)?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_changed_source_before_execution() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("source");
        fs::write(&p, b"changed").unwrap();
        assert!(check_id(&p, GPERF).is_err());
    }
    #[test]
    fn input_recheck_rejects_changed_bytes() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("source");
        fs::write(&p, b"before").unwrap();
        let expected = hash_file(&p).unwrap();
        check_id(&p, &expected.sha256).unwrap();
        fs::write(&p, b"after").unwrap();
        assert!(check_id(&p, &expected.sha256).is_err());
    }
    #[test]
    fn selected_snapshot_cannot_drift_behind_unchanged_manifest() {
        let d = tempfile::tempdir().unwrap();
        let tree = d.path().join("tree");
        fs::create_dir(&tree).unwrap();
        let p = tree.join("tool");
        fs::write(&p, b"original").unwrap();
        let m =
            json!({"entries":{"tool":{"kind":"file","link":null,"object":hash_file(&p).unwrap()}}});
        snapshot_file(&tree, &m, &p).unwrap();
        fs::write(&p, b"changed").unwrap();
        assert!(snapshot_file(&tree, &m, &p).is_err());
    }
    #[test]
    fn missing_or_divergent_build_compiler_is_rejected_before_execution() {
        let mut environment = BTreeMap::new();
        for (name, value) in [
            ("CC", "gcc --sysroot=/frozen"),
            ("CFLAGS", "-O2"),
            ("LDFLAGS", "--sysroot=/frozen"),
        ] {
            environment.insert(name.into(), value.into());
            environment.insert(format!("{name}_FOR_BUILD"), value.into());
        }
        validate_compiler_environment(&environment).unwrap();
        environment.remove("CC_FOR_BUILD");
        assert!(validate_compiler_environment(&environment).is_err());
        environment.insert("CC_FOR_BUILD".into(), "gcc".into());
        assert!(validate_compiler_environment(&environment).is_err());
    }
}
