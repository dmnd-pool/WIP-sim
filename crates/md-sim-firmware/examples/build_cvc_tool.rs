//! Build the pinned source-available CVC mirror inside the closed tool runtime.
//! This is a tool prerequisite. It does not establish selected HDL equivalence.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
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
const ARCHIVE: &str = "bb9d8acd54f168db608b4a16a6bebc758cca59836fdfacebd89d0b635aa2cf13";
const SOURCE_DIR: &str = "/work/open-src-cvc-1c5e043ec33ef6f1fdbf38565501c944893a83cf";
const COMPILER: &str = "cc98e84002c7b7fefb0128128e4cf4546d333e780715da16d048e17b73dafaa5";
const MANIFEST: &str = "e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a";
const LICENSE: &str = "b0f3aa461c3835c312f9e56221481163a58034b0632c96b65f4ffe66ca3a849e";
const CC: &str = "/work/toolchain/usr/bin/gcc --sysroot=/work/toolchain -B/work/toolchain/usr/bin -std=gnu99 -MMD -fsanitize=undefined -fsanitize-undefined-trap-on-error -fno-sanitize-recover=all";
const CFLAGS: &str = "-D__RHEL6X__ -fno-pie -no-pie";
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
  $display("MD_SIM_CVC_SMOKE_PASS wrap=0 unknown=x101 edges=2 time=8"); $finish;
 end
endmodule
"#;
const PASS: &str = "MD_SIM_CVC_SMOKE_PASS wrap=0 unknown=x101 edges=2 time=8";
fn exact(path: &Path, sha: &str) -> Result<ObjectId> {
    let id = hash_file(path)?;
    ensure!(id.sha256 == sha, "exact input changed: {}", path.display());
    Ok(id)
}
fn run(report: &mut Value, name: &str, program: &str, args: &[String], cwd: &Path) -> Result<bool> {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    let result = qemu_build::step(report, name, command)?;
    report[name]["program"] = json!(program);
    report[name]["argv"] = json!(args);
    report[name]["cwd"] = json!(cwd);
    ensure!(
        report[name]["stdout_truncated"] == false && report[name]["stderr_truncated"] == false,
        "step output truncated: {name}"
    );
    Ok(result)
}
fn diagnostic_free(row: &Value) -> Result<()> {
    ensure!(row["stderr"] == "", "unexpected CVC stderr diagnostic");
    let text = row["stdout"].as_str().context("CVC stdout")?;
    // Pinned v_ms.c prints location-prefixed ERROR**, WARN** and FATAL ERROR**.
    ensure!(
        !["ERROR**", "WARN**", "WARNING**", "FATAL**"]
            .iter()
            .any(|marker| text.contains(marker)),
        "CVC compiler/runtime diagnostic"
    );
    Ok(())
}
fn parse_manifest(bytes: &[u8], expected: &str) -> Result<Value> {
    ensure!(
        md_sim_firmware::prepare::hash_bytes(bytes) == expected,
        "compiler manifest bytes changed"
    );
    Ok(serde_json::from_slice(bytes)?)
}
fn invalid_syntax(row: &Value) -> Result<()> {
    ensure!(
        row["code"].as_i64().is_some_and(|code| code != 0),
        "invalid syntax compiler did not fail"
    );
    let diagnostic = row["stdout"].as_str().context("negative stdout")?;
    ensure!(
        row["stderr"] == ""
            && diagnostic
                .lines()
                .any(|line| line.contains("invalid.v(1) ERROR**")
                    && (line.contains("expression")
                        || line.contains("primary")
                        || line.contains("syntax")
                        || line.contains("illegal token"))),
        "invalid HDL syntax diagnostic absent"
    );
    Ok(())
}
fn smoke_result(row: &Value) -> Result<()> {
    ensure!(row["code"] == 0, "smoke execution failed");
    let out = row["stdout"].as_str().context("smoke stdout")?;
    ensure!(
        out.lines().filter(|line| *line == PASS).count() == 1,
        "exact smoke completion absent or repeated"
    );
    diagnostic_free(row)?;
    Ok(())
}
fn compile_smoke(report: &mut Value, prefix: &str, binary: &str) -> Result<()> {
    let name = format!("{prefix}_compile");
    let output = format!("/work/{prefix}-sim");
    ensure!(
        run(
            report,
            &name,
            binary,
            &[
                "-q".into(),
                "-sv".into(),
                "-o".into(),
                output.clone(),
                "/work/smoke.v".into()
            ],
            Path::new("/work")
        )?,
        "CVC smoke compile failed"
    );
    diagnostic_free(&report[&name])?;
    ensure!(
        Path::new(&output).is_file(),
        "compiled smoke program absent"
    );
    report[&name]["program_object"] = json!(hash_file(Path::new(&output))?);
    let name = format!("{prefix}_execute");
    ensure!(
        run(report, &name, &output, &[], Path::new("/work"))?,
        "compiled smoke execution failed"
    );
    smoke_result(&report[&name])
}
fn build(report: &mut Value) -> Result<()> {
    for (path, hash) in [
        ("/source.tar.gz", ARCHIVE),
        ("/compiler.tar.gz", COMPILER),
        ("/compiler-manifest.json", MANIFEST),
    ] {
        exact(Path::new(path), hash)?;
    }
    qemu_build::unpack("/compiler.tar.gz", "/work")?;
    qemu_build::unpack("/source.tar.gz", "/work")?;
    let source = Path::new(SOURCE_DIR);
    exact(&source.join("OSS-CVC-MODIFIED-ARTISTIC-LIC.TXT"), LICENSE)?;
    let args = vec![
        "-j2".into(),
        "cvc64".into(),
        format!("CC={CC}"),
        format!("CVCCFLGS={CFLAGS}"),
        "LD=/work/toolchain/usr/bin/ld".into(),
    ];
    ensure!(
        run(
            report,
            "build",
            "/work/toolchain/usr/bin/make",
            &args,
            &source.join("src")
        )?,
        "CVC source build failed"
    );
    let built = source.join("build64/cvc64");
    report["binary"] = json!(hash_file(&built)?);
    let built = built.to_str().context("built path UTF-8")?;
    ensure!(
        run(report, "help", built, &["-h".into()], Path::new("/work"))?,
        "CVC help failed"
    );
    fs::write("/work/smoke.v", SMOKE)?;
    compile_smoke(report, "original", built)?;
    fs::write("/work/invalid.v", "module invalid; wire x = ; endmodule\n")?;
    ensure!(
        !run(
            report,
            "invalid_compile",
            built,
            &[
                "-q".into(),
                "-o".into(),
                "/work/invalid-sim".into(),
                "/work/invalid.v".into()
            ],
            Path::new("/work")
        )?,
        "invalid HDL compiled successfully"
    );
    let negative = &report["invalid_compile"];
    ensure!(
        negative["code"].as_i64().is_some_and(|code| code != 0)
            && !Path::new("/work/invalid-sim").exists(),
        "invalid syntax did not produce a real compiler rejection"
    );
    invalid_syntax(negative)?;
    let install = Path::new("/work/cvc-install");
    fs::create_dir(install)?;
    fs::copy(built, install.join("cvc64"))?;
    for name in [
        "OSS-CVC-MODIFIED-ARTISTIC-LIC.TXT",
        "README.md",
        "CHANGES.md",
        "INSTALL",
    ] {
        fs::copy(source.join(name), install.join(name))?;
    }
    fs::create_dir(install.join("pli_incs"))?;
    for entry in fs::read_dir(source.join("pli_incs"))? {
        let entry = entry?;
        ensure!(
            entry.file_type()?.is_file(),
            "unexpected PLI include object"
        );
        fs::copy(
            entry.path(),
            install.join("pli_incs").join(entry.file_name()),
        )?;
    }
    qemu_build::pack_dir(install, "cvc-install", Path::new("/work/cvc-tools.tar.gz"))?;
    fs::rename(install, "/work/original-install-unavailable")?;
    fs::rename(source, "/work/original-source-unavailable")?;
    qemu_build::unpack("/work/cvc-tools.tar.gz", "/work")?;
    ensure!(
        hash_file(&install.join("cvc64"))?
            == serde_json::from_value::<ObjectId>(report["binary"].clone())?,
        "restored binary changed"
    );
    compile_smoke(report, "restored", "/work/cvc-install/cvc64")?;
    report["install_contract"] = json!({"root":"cvc-install","compiler_closure_required":true,"standalone":false,"source_archive_sha256":ARCHIVE,"compiler_archive_sha256":COMPILER,"compiler_manifest_sha256":MANIFEST,"original_source_and_install_renamed_before_restore_test":true,"license":"OSS CVC dual licensing modified Artistic; original notices retained"});
    let id = hash_file(Path::new("/work/cvc-tools.tar.gz"))?;
    ensure!(id.size <= 24 << 20, "tool export bound");
    report["tools"] = json!(id);
    report["tools_hex"] = json!(hex::encode(fs::read("/work/cvc-tools.tar.gz")?));
    Ok(())
}
fn inner() -> Result<()> {
    confinement::require_hdl_tool_build()?;
    let mut report = json!({"schema":1,"source_archive_sha256":ARCHIVE,"source_revision":"1c5e043ec33ef6f1fdbf38565501c944893a83cf","sanitizer":"All CC invocations include undefined-behavior trap checks with no recovery, including upstream hexasm; generated assembly is not C instrumentation","four_state":true,"two_state_option_used":false,"claim":"Existing source-available CVC tool build and smoke only. Selected logic, firmware, timing and mining gates remain false.","selected_hdl_equivalence":false,"original_board_gate":false,"smoke_source":SMOKE});
    let result = build(&mut report);
    report["success"] = json!(result.is_ok());
    if let Err(error) = &result {
        report["error"] = json!(format!("{error:#}"));
    }
    serde_json::to_writer(std::io::stdout(), &report)?;
    std::io::stdout().flush()?;
    result
}
fn snapshot_root(tree: &Path, manifest: &Value, relative: &str) -> Result<(PathBuf, ObjectId)> {
    let path = tree.join(relative);
    let entry = &manifest["entries"][relative];
    ensure!(
        entry["kind"] == "file"
            && entry["link"].is_null()
            && fs::symlink_metadata(&path)?.is_file()
            && path.canonicalize()? == tree.canonicalize()?.join(relative),
        "snapshot root is not a regular captured file"
    );
    let id: ObjectId = serde_json::from_value(entry["object"].clone())?;
    ensure!(hash_file(&path)? == id, "snapshot root changed");
    Ok((path, id))
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        return inner();
    }
    ensure!(args.len() == 3, "usage: build_cvc_tool REPO NEW_OUTPUT");
    let repo = PathBuf::from(&args[1]).canonicalize()?;
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "new output required");
    let snapshot = repo.join("work/simulator/firmware/toolchain-snapshot-003");
    let tree = snapshot.join("tree");
    let manifest_bytes = fs::read(snapshot.join("manifest.json"))?;
    let manifest = parse_manifest(&manifest_bytes, MANIFEST)?;
    let source=repo.join("artifacts/files/source/source-cvc-mirror-1c5e043-20261028/open-src-cvc-1c5e043ec33e.tar.gz");
    exact(&source, ARCHIVE)?;
    let compiler = repo.join("work/simulator/qemu-s9/samurai-build-003/toolchain.tar.gz");
    exact(&compiler, COMPILER)?;
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 512 << 20)?;
    let mut roots = vec![("/guard".into(), std::env::current_exe()?)];
    let mut identities = vec![];
    for (guest, relative) in [
        ("/bin/sh", "usr/bin/bash"),
        ("/loader-roots/gcc", "usr/bin/gcc"),
        ("/loader-roots/as", "usr/bin/as"),
        ("/loader-roots/ld", "usr/bin/ld"),
        ("/loader-roots/make", "usr/bin/make"),
        (
            "/loader-roots/cc1",
            "usr/lib/gcc/x86_64-pc-linux-gnu/16/cc1",
        ),
    ] {
        let (path, id) = snapshot_root(&tree, &manifest, relative)?;
        roots.push((guest.into(), path));
        identities.push((guest.to_owned(), id));
    }
    let mut inputs = md_sim_firmware::lab::elf_closure(&store, roots)?;
    for (guest, id) in &identities {
        ensure!(
            inputs
                .iter()
                .any(|m| m.guest_path == *guest && m.object == *id),
            "captured loader root differs"
        );
    }
    for (path, guest, expected) in [
        (source, "/source.tar.gz", ARCHIVE),
        (compiler, "/compiler.tar.gz", COMPILER),
        (
            snapshot.join("manifest.json"),
            "/compiler-manifest.json",
            MANIFEST,
        ),
    ] {
        ensure!(
            !inputs.iter().any(|m| m.guest_path == guest),
            "duplicate input"
        );
        let object = exact(&path, expected)?;
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    let spec = ProcessSpec {
        schema_version: "2.0".into(),
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        argv: vec!["/guard".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::from([
            ("PATH".into(), "/work/toolchain/usr/bin".into()),
            ("LD_LIBRARY_PATH".into(), "/work/toolchain/usr/lib".into()),
            ("LC_ALL".into(), "C".into()),
            ("TMPDIR".into(), "/tmp".into()),
        ]),
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
        &json!({"source":ARCHIVE,"compiler":COMPILER,"compiler_manifest":MANIFEST,"selected_snapshot_roots":identities,"helper":hash_file(&std::env::current_exe()?)?,"budget_basis":"Existing compiler snapshot expands698MB; CVC source27MB; two compiler jobs. Preparation-only, unchanged existing HDL-build limits."}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let raw = hex::decode(&result.stdout)?;
    fs::write(out.join("build-envelope.json"), &raw)?;
    ensure!(
        !result.trace_truncated,
        "build trace truncated; raw result retained"
    );
    let mut report: Value =
        serde_json::from_slice(&raw).context("build report absent; raw output retained")?;
    if let Some(data) = report["tools_hex"].as_str() {
        let data = hex::decode(data)?;
        let id: ObjectId = serde_json::from_value(report["tools"].clone())?;
        ensure!(
            data.len() as u64 == id.size
                && md_sim_firmware::prepare::hash_bytes(&data) == id.sha256,
            "tool output identity differs"
        );
        fs::write(out.join("cvc-tools.tar.gz"), data)?;
    }
    report
        .as_object_mut()
        .context("report object")?
        .remove("tools_hex");
    save_json(&out.join("build-report.json"), &report)?;
    for mount in &spec.inputs {
        store.verified_path(&mount.object)?;
    }
    ensure!(
        result.exit_code == Some(0) && result.stderr.is_empty() && report["success"] == true,
        "isolated CVC tool proof failed; evidence retained"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parsed_manifest_is_bound_to_its_exact_bytes() {
        let bytes = br#"{"entries":{}}"#;
        let expected = md_sim_firmware::prepare::hash_bytes(bytes);
        parse_manifest(bytes, &expected).unwrap();
        assert!(parse_manifest(br#"{"entries":{"changed":1}}"#, &expected).is_err());
    }
    #[test]
    fn compiler_warnings_and_wrong_negative_are_rejected() {
        let mut row =
            json!({"code":0,"stdout":"**/work/smoke.v(1) WARN** [1] bad width", "stderr":""});
        assert!(diagnostic_free(&row).is_err());
        row["code"] = json!(1);
        row["stdout"] = json!("**/work/other.v(1) ERROR** [1] syntax error");
        assert!(invalid_syntax(&row).is_err());
        row["stdout"] = json!("**/work/invalid.v(1) ERROR** [1] syntax error");
        invalid_syntax(&row).unwrap();
    }
    #[test]
    fn modified_input_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("input");
        fs::write(&p, b"a").unwrap();
        let id = hash_file(&p).unwrap();
        exact(&p, &id.sha256).unwrap();
        fs::write(&p, b"b").unwrap();
        assert!(exact(&p, &id.sha256).is_err());
    }
    #[test]
    fn smoke_requires_exact_once_and_no_runtime_error() {
        let mut row = json!({"code":0,"stdout":format!("{PASS}\n"),"stderr":""});
        smoke_result(&row).unwrap();
        row["stdout"] = json!(format!("{PASS}\n{PASS}\n"));
        assert!(smoke_result(&row).is_err());
        row["stdout"] = json!(format!("{PASS}\n**ERROR** bad"));
        assert!(smoke_result(&row).is_err());
        row["stdout"] = json!(PASS);
        row["code"] = json!(1);
        assert!(smoke_result(&row).is_err());
    }
    #[test]
    fn changed_snapshot_root_is_rejected() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tool");
        fs::write(&p, b"original").unwrap();
        let m =
            json!({"entries":{"tool":{"kind":"file","link":null,"object":hash_file(&p).unwrap()}}});
        snapshot_root(d.path(), &m, "tool").unwrap();
        fs::write(&p, b"changed").unwrap();
        assert!(snapshot_root(d.path(), &m, "tool").is_err());
    }
}
