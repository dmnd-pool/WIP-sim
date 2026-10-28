//! Compare the same future-RW/two-NBA oracle in two pinned existing HDL engines.
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
#[path = "support/bounded_file.rs"]
mod bounded_file;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/cvc_lexer.rs"]
mod cvc_lexer;
#[allow(dead_code)]
#[path = "support/qemu_build.rs"]
mod qemu_build;
const C: &str = include_str!("../../../simulator/backends/cvc/rw-order.c");
const HDL: &str = include_str!("../../../simulator/backends/cvc/rw-order.v");
const PHASE_C: &str = include_str!("../../../simulator/backends/cvc/rw-phases.c");
const PHASE_HDL: &str = include_str!("../../../simulator/backends/cvc/rw-phases.v");
// Exact separate isolated build516b; the original tool output remains pinned below.
const DERIVED_CVC: &str = "f0d6e315d8bfd32412e78bf206fd5d4fd7056705828f6b796f46d8bd66a3fb47";
const LEXER_CVC: &str = "9a5ed2d7e4c5d5ca0f26f5009265bda9188dfd314a64e01ad9d8922d0239d41f";
const CVC: &str = "03078829c3f9f64e9615f9ba9b9aaee582a61527e668ca88006028ff45454c7b";
const ICARUS: &str = "3c5631a6d6285affa39d9fddb9acca3758988b9c2fe6cbc4e3ce227f5fc33f61";
const COMPILER: &str = "cc98e84002c7b7fefb0128128e4cf4546d333e780715da16d048e17b73dafaa5";
const PRIOR_SPEC: &str = "b4a4222c83556dd01d1e3060b3fdf6935978cfb1a71e5bc491cd83f1aaa0d00a";
const PASS: &str = "MD_RW_ORDER_CONTRACT_PASS";
const EXPECTED: &str = "MD_RW_ORDER time=5 input=1 edge=1 sampled=0";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Original,
    Rw,
    Lexer,
}
impl Tool {
    fn sha(self) -> &'static str {
        match self {
            Self::Original => CVC,
            Self::Rw => DERIVED_CVC,
            Self::Lexer => LEXER_CVC,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Rw => "rw-after-nba-v1",
            Self::Lexer => "rw-after-nba-lexer-context-v1",
        }
    }
    fn path(self) -> &'static str {
        match self {
            Self::Original => "work/simulator/qemu-s9/cvc-build-516/cvc-tools.tar.gz",
            Self::Rw => "work/simulator/qemu-s9/cvc-build-516b/cvc-tools.tar.gz",
            Self::Lexer => "work/simulator/qemu-s9/cvc-build-516f/cvc-tools.tar.gz",
        }
    }
}
fn exact(path: &Path, expected: &str) -> Result<ObjectId> {
    let id = hash_file(path)?;
    ensure!(id.sha256 == expected, "input changed: {}", path.display());
    Ok(id)
}
fn run(report: &mut Value, name: &str, program: &str, args: &[String]) -> Result<bool> {
    let mut c = Command::new(program);
    c.args(args).current_dir("/work");
    let pass = qemu_build::step(report, name, c)?;
    report[name]["program"] = json!(program);
    report[name]["argv"] = json!(args);
    ensure!(
        report[name]["stdout_truncated"] == false && report[name]["stderr_truncated"] == false,
        "step output truncated"
    );
    Ok(pass)
}
fn clean(row: &Value) -> Result<()> {
    ensure!(
        row["code"] == 0 && row["stderr"] == "",
        "compiler failed or diagnosed"
    );
    let s = row["stdout"].as_str().context("compiler stdout")?;
    ensure!(
        !["ERROR**", "WARN**", "INFORM"]
            .iter()
            .any(|m| s.contains(m)),
        "compiler diagnostic"
    );
    Ok(())
}
fn oracle(row: &Value) -> Result<()> {
    ensure!(
        row["code"] == 0 && row["stderr"] == "",
        "RW oracle process failed"
    );
    let out = row["stdout"].as_str().context("oracle stdout")?;
    let observations = out
        .lines()
        .filter(|line| line.starts_with("MD_RW_"))
        .collect::<Vec<_>>();
    ensure!(
        observations == [EXPECTED, PASS],
        "RW observations missing, repeated, reordered or unexpected"
    );
    ensure!(
        !out.contains("MISSING_CALLBACK") && !out.contains("ERROR**") && !out.contains("WARN**"),
        "RW oracle diagnostic"
    );
    Ok(())
}
fn phase_oracle(row: &Value, name: &str) -> Result<()> {
    ensure!(
        ["future", "zero", "nested", "cancel"].contains(&name),
        "unknown phase case"
    );
    clean(row)?;
    let nested = u8::from(name == "nested");
    let value = 1 - nested;
    let expected = format!("MD_RW_PHASE case={name} time=5 input={value} edge=1 sampled=0 echo={value} first=1 peer={nested} nested={nested} final={nested} cancelled=0");
    let out = row["stdout"].as_str().context("phase stdout")?;
    let lines = out
        .lines()
        .filter(|line| line.starts_with("MD_RW_"))
        .collect::<Vec<_>>();
    ensure!(
        lines == [expected, "MD_RW_PHASE_PASS".into()],
        "phase receipt missing, reordered, changed or duplicated"
    );
    Ok(())
}
fn compile_vpi(
    report: &mut Value,
    prefix: &str,
    source: &str,
    engine: &str,
    header: &str,
) -> Result<()> {
    let args = [
        "--sysroot=/work/toolchain",
        "-B/work/toolchain/usr/bin",
        "-std=c11",
        "-O2",
        "-fPIC",
        "-shared",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-fsanitize=undefined",
        "-fsanitize-undefined-trap-on-error",
        "-fno-sanitize-recover=all",
        "-Wl,-z,noexecstack",
        "-I",
        header,
        source,
        "-o",
    ]
    .map(str::to_owned)
    .into_iter()
    .chain([format!("/work/{prefix}_{engine}.so")])
    .collect::<Vec<_>>();
    let key = if prefix == "rw" {
        format!("{engine}_vpi_compile")
    } else {
        format!("{prefix}_{engine}_vpi_compile")
    };
    ensure!(
        run(report, &key, "/work/toolchain/usr/bin/gcc", &args)?,
        "VPI compile failed"
    );
    clean(&report[&key])?;
    report[if prefix == "rw" {
        format!("{engine}_vpi")
    } else {
        format!("{prefix}_{engine}_vpi")
    }] = json!(hash_file(Path::new(&format!(
        "/work/{prefix}_{engine}.so"
    )))?);
    Ok(())
}
fn phase_tests(report: &mut Value, selected: &str) -> Result<()> {
    ensure!(
        ["nba", "delayed", "no-nba"].contains(&selected),
        "unknown phase variant"
    );
    report["phase_variant"] = json!(selected);
    report["phase_passed_cases"] = json!([]);
    fs::write("/work/rw-phases.c", PHASE_C)?;
    fs::write("/work/rw-phases.v", PHASE_HDL)?;
    for (engine, header) in [
        ("icarus", "/work/iverilog-install/include/iverilog"),
        ("cvc", "/work/cvc-install/pli_incs"),
    ] {
        compile_vpi(report, "phases", "/work/rw-phases.c", engine, header)?;
    }
    for (variant, define, cases) in [
        ("nba", None, &["future", "zero", "nested", "cancel"][..]),
        (
            "delayed",
            Some("MD_DELAYED_NBA"),
            &["future", "zero", "nested", "cancel"][..],
        ),
        ("no-nba", Some("MD_NO_NBA"), &["future", "zero"][..]),
    ] {
        if variant != selected {
            continue;
        }
        let key_variant = if variant == "no-nba" {
            "no_nba"
        } else {
            variant
        };
        let vvp = format!("/work/phases-{variant}.vvp");
        let program = format!("/work/phases-{variant}-cvc");
        let mut args = [
            "-g2012",
            "-Wall",
            "-s",
            "rw_phases",
            "-o",
            &vvp,
            "/work/rw-phases.v",
        ]
        .map(str::to_owned)
        .to_vec();
        if let Some(define) = define {
            args.insert(0, format!("-D{define}"));
        }
        let key = format!("phases_{key_variant}_icarus_compile");
        ensure!(
            run(report, &key, "/work/iverilog-install/bin/iverilog", &args)?,
            "phase Icarus compile"
        );
        clean(&report[&key])?;
        let mut args = [
            "-q",
            "-sv",
            "+acc+2",
            "+loadvpi=/work/phases_cvc.so:mdsim_rw_phases_bootstrap",
            "-o",
            &program,
            "/work/rw-phases.v",
        ]
        .map(str::to_owned)
        .to_vec();
        if let Some(define) = define {
            args.insert(0, format!("+define+{define}"));
        }
        let key = format!("phases_{key_variant}_cvc_compile");
        ensure!(
            run(report, &key, "/work/cvc-install/cvc64", &args)?,
            "phase CVC compile"
        );
        clean(&report[&key])?;
        report[format!("phases_{key_variant}_programs")] =
            json!({"icarus":hash_file(Path::new(&vvp))?, "cvc":hash_file(Path::new(&program))?});
        for name in cases {
            let argument = format!("+md_rw_case={name}");
            let key = format!("phases_{key_variant}_{name}_icarus");
            let _ = run(
                report,
                &key,
                "/work/iverilog-install/bin/vvp",
                &[
                    "-m/work/phases_icarus.so".into(),
                    vvp.clone(),
                    argument.clone(),
                ],
            )?;
            phase_oracle(&report[&key], name)?;
            report["phase_passed_cases"]
                .as_array_mut()
                .context("phase case array")?
                .push(json!(key));
            let key = format!("phases_{key_variant}_{name}_cvc_compiled");
            let _ = run(report, &key, &program, std::slice::from_ref(&argument))?;
            phase_oracle(&report[&key], name)?;
            report["phase_passed_cases"]
                .as_array_mut()
                .context("phase case array")?
                .push(json!(key));
            let key = format!("phases_{key_variant}_{name}_cvc_interpreted");
            let mut args = [
                "-q",
                "-sv",
                "+interp",
                "+acc+2",
                "+loadvpi=/work/phases_cvc.so:mdsim_rw_phases_bootstrap",
                "/work/rw-phases.v",
            ]
            .map(str::to_owned)
            .to_vec();
            if let Some(define) = define {
                args.insert(0, format!("+define+{define}"));
            }
            args.push(argument);
            let _ = run(report, &key, "/work/cvc-install/cvc64", &args)?;
            phase_oracle(&report[&key], name)?;
            report["phase_passed_cases"]
                .as_array_mut()
                .context("phase case array")?
                .push(json!(key));
        }
    }
    let count = report["phase_passed_cases"]
        .as_array()
        .context("phase case array")?
        .len();
    let expected = if selected == "no-nba" { 6 } else { 12 };
    ensure!(count == expected, "phase case coverage incomplete");
    report["phase_cases_required"] = json!(expected);
    report["phase_cases_passed"] = json!(count);
    Ok(())
}
fn rw_mode(args: &[std::ffi::OsString], at: usize) -> Result<(bool, Option<&'static str>)> {
    match args.get(at) {
        None => Ok((false, None)),
        Some(value) if value == "--rw-after-nba" && args.len() == at + 1 => Ok((true, None)),
        Some(value)
            if value == "--rw-after-nba" && args.len() == at + 3 && args[at + 1] == "--phase" =>
        {
            match args[at + 2].to_str() {
                Some("nba") => Ok((true, Some("nba"))),
                Some("delayed") => Ok((true, Some("delayed"))),
                Some("no-nba") => Ok((true, Some("no-nba"))),
                _ => anyhow::bail!("unknown phase variant"),
            }
        }
        _ => anyhow::bail!("unknown RW probe profile"),
    }
}
fn mode(args: &[std::ffi::OsString], at: usize) -> Result<(Tool, Option<&'static str>, bool)> {
    let lexer = args.len() > at && args.last().is_some_and(|a| a == "--lexer");
    let mut slice = if lexer {
        args[..args.len() - 1].to_vec()
    } else {
        args.to_vec()
    };
    let context = slice.get(at + 1).is_some_and(|a| a == "--lexer-context");
    if context {
        slice.remove(at + 1);
    }
    let (after_nba, phase) = rw_mode(&slice, at)?;
    ensure!(
        !context || after_nba,
        "lexer context tool requires RW profile"
    );
    ensure!(
        !lexer || phase.is_none(),
        "lexer and RW phase modes are separate"
    );
    let tool = if context {
        Tool::Lexer
    } else if after_nba {
        Tool::Rw
    } else {
        Tool::Original
    };
    Ok((tool, phase, lexer))
}
fn probe(report: &mut Value, tool: Tool, phase: Option<&str>, lexer: bool) -> Result<()> {
    for (p, id) in [
        ("/compiler.tar.gz", COMPILER),
        ("/cvc.tar.gz", tool.sha()),
        ("/icarus.tar.gz", ICARUS),
    ] {
        exact(Path::new(p), id)?;
        qemu_build::unpack(p, "/work")?;
    }
    if lexer {
        return cvc_lexer::probe(report);
    }
    if let Some(phase) = phase {
        return phase_tests(report, phase);
    }
    fs::write("/work/rw-order.c", C)?;
    fs::write("/work/rw-order.v", HDL)?;
    for (name, header) in [
        ("icarus", "/work/iverilog-install/include/iverilog"),
        ("cvc", "/work/cvc-install/pli_incs"),
    ] {
        compile_vpi(report, "rw", "/work/rw-order.c", name, header)?;
    }
    ensure!(
        run(
            report,
            "icarus_compile",
            "/work/iverilog-install/bin/iverilog",
            &[
                "-g2012",
                "-Wall",
                "-s",
                "rw_order",
                "-o",
                "/work/rw.vvp",
                "/work/rw-order.v"
            ]
            .map(str::to_owned)
        )?,
        "Icarus oracle compile failed"
    );
    clean(&report["icarus_compile"])?;
    let _ = run(
        report,
        "icarus_execute",
        "/work/iverilog-install/bin/vvp",
        &["-m/work/rw_icarus.so", "/work/rw.vvp"].map(str::to_owned),
    )?;
    report["icarus_contract_pass"] = json!(oracle(&report["icarus_execute"]).is_ok());
    oracle(&report["icarus_execute"])?;
    ensure!(
        run(
            report,
            "cvc_compile",
            "/work/cvc-install/cvc64",
            &[
                "-q",
                "-sv",
                "+acc+2",
                "+loadvpi=/work/rw_cvc.so:mdsim_rw_bootstrap",
                "-o",
                "/work/rw-cvc",
                "/work/rw-order.v"
            ]
            .map(str::to_owned)
        )?,
        "CVC oracle compile failed"
    );
    clean(&report["cvc_compile"])?;
    report["icarus_program"] = json!(hash_file(Path::new("/work/rw.vvp"))?);
    report["cvc_program"] = json!(hash_file(Path::new("/work/rw-cvc"))?);
    let _ = run(report, "cvc_execute", "/work/rw-cvc", &[])?;
    report["cvc_contract_pass"] = json!(oracle(&report["cvc_execute"]).is_ok());
    // A retained CVC boundary failure remains a failing execution, even though
    // the old simulator and the diagnostic successfully identify the difference.
    oracle(&report["cvc_execute"])?;
    if tool != Tool::Original {
        let _ = run(
            report,
            "cvc_interpreted",
            "/work/cvc-install/cvc64",
            &[
                "-q",
                "-sv",
                "+interp",
                "+acc+2",
                "+loadvpi=/work/rw_cvc.so:mdsim_rw_bootstrap",
                "/work/rw-order.v",
            ]
            .map(str::to_owned),
        )?;
        oracle(&report["cvc_interpreted"])?;
    }
    Ok(())
}
fn retain(report: &mut Value) -> Result<()> {
    report["retained_artifacts"] = json!({});
    fs::create_dir("/work/compiled")?;
    for name in [
        "rw.vvp",
        "rw-cvc",
        "rw_icarus.so",
        "rw_cvc.so",
        "rw-order.c",
        "rw-order.v",
        "lexer-library.v",
        "lexer-top.v",
        "lexer.vvp",
        "lexer-cvc",
        "rw-phases.c",
        "rw-phases.v",
        "phases_icarus.so",
        "phases_cvc.so",
        "phases-nba.vvp",
        "phases-nba-cvc",
        "phases-delayed.vvp",
        "phases-delayed-cvc",
        "phases-no-nba.vvp",
        "phases-no-nba-cvc",
    ] {
        let path = Path::new("/work").join(name);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
            Ok(meta) => ensure!(
                meta.is_file() && meta.len() <= 32 << 20,
                "compiled artifact type/size bound"
            ),
        }
        let captured = bounded_file::capture(&path, 32 << 20)?;
        let target = Path::new("/work/compiled").join(name);
        fs::write(&target, &captured.bytes)?;
        ensure!(
            hash_file(&target)? == captured.object,
            "retained artifact changed"
        );
        report["retained_artifacts"][name] = json!(captured.object);
    }
    qemu_build::pack_dir(
        Path::new("/work/compiled"),
        "compiled",
        Path::new("/work/compiled.tar.gz"),
    )?;
    let id = hash_file(Path::new("/work/compiled.tar.gz"))?;
    ensure!(id.size <= 8 << 20, "oracle export bound");
    report["compiled_archive"] = json!(id);
    report["compiled_archive_hex"] = json!(hex::encode(fs::read("/work/compiled.tar.gz")?));
    Ok(())
}
fn inner(tool: Tool, phase: Option<&str>, lexer: bool) -> Result<()> {
    confinement::require(true)?;
    let mut report = json!({"schema":1,"claim":"Exact future-RW/two-NBA API boundary oracle only; no selected HDL, firmware or physical timing acceptance","icarus_contract_pass":false,"cvc_contract_pass":false,"selected_hdl_equivalence":false,"fixture_c":md_sim_firmware::prepare::hash_bytes(C.as_bytes()),"fixture_hdl":md_sim_firmware::prepare::hash_bytes(HDL.as_bytes()),"expected_observation":EXPECTED});
    report["profile"] = json!(tool.name());
    report["phase_variant"] = json!(phase);
    report["full_phase_suite_complete"] = json!(false);
    report["lexer_mode"] = json!(lexer);
    let r = probe(&mut report, tool, phase, lexer);
    let retained = retain(&mut report);
    if let Err(error) = &retained {
        report["retention_error"] = json!(format!("{error:#}"));
    }
    report["success"] = json!(r.is_ok() && retained.is_ok());
    if let Err(e) = &r {
        report["error"] = json!(format!("{e:#}"));
    }
    serde_json::to_writer(std::io::stdout(), &report)?;
    std::io::stdout().flush()?;
    r.and(retained)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        let (tool, phase, lexer) = mode(&args, 2)?;
        return inner(tool, phase, lexer);
    }
    ensure!(
        args.len() >= 3,
        "usage: cvc_rw_probe REPO NEW_OUTPUT [--rw-after-nba [--lexer-context] [--phase nba|delayed|no-nba]] [--lexer]"
    );
    let (tool, phase, lexer) = mode(&args, 3)?;
    let repo = PathBuf::from(&args[1]).canonicalize()?;
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "new output required");
    let previous = repo.join("work/simulator/qemu-s9/cvc-build-516");
    let raw = bounded_file::capture(&previous.join("spec.json"), 64 << 10)?;
    ensure!(
        raw.object.sha256 == PRIOR_SPEC,
        "prior compiler closure spec differs"
    );
    let prior: ProcessSpec = serde_json::from_slice(&raw.bytes)?;
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 512 << 20)?;
    let old = Store::open(&previous.join("inputs"), 512 << 20)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/guard".into(), std::env::current_exe()?)],
    )?;
    for m in &prior.inputs {
        if ["/guard", "/source.tar.gz"].contains(&m.guest_path.as_str()) {
            continue;
        }
        if let Some(known) = inputs.iter().find(|x| x.guest_path == m.guest_path) {
            ensure!(
                known.object == m.object,
                "new helper runtime differs from pinned prior closure"
            );
            continue;
        }
        let path = old.verified_path(&m.object)?;
        store.import(&path, &m.object)?;
        inputs.push(m.clone());
    }
    for (path, guest, hash) in [
        (repo.join(tool.path()), "/cvc.tar.gz", tool.sha()),
        (
            repo.join("work/simulator/qemu-s9/iverilog-build-502f/iverilog-tools.tar.gz"),
            "/icarus.tar.gz",
            ICARUS,
        ),
    ] {
        let object = exact(&path, hash)?;
        ensure!(
            !inputs.iter().any(|m| m.guest_path == guest),
            "duplicate input"
        );
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    let mut argv = vec!["/guard".into(), "__inside".into()];
    if tool != Tool::Original {
        argv.push("--rw-after-nba".into());
    }
    if tool == Tool::Lexer {
        argv.push("--lexer-context".into());
    }
    if let Some(phase) = phase {
        argv.extend(["--phase".into(), phase.into()]);
    }
    if lexer {
        argv.push("--lexer".into());
    }
    let spec = ProcessSpec {
        schema_version: "2.0".into(),
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        argv,
        inputs,
        stdin_object: None,
        environment: BTreeMap::from([
            ("PATH".into(), "/work/toolchain/usr/bin".into()),
            ("LD_LIBRARY_PATH".into(), "/work/toolchain/usr/lib".into()),
            ("LC_ALL".into(), "C".into()),
            ("TMPDIR".into(), "/tmp".into()),
        ]),
        limits: Limits {
            wall_seconds: 180,
            cpu_seconds: 120,
            memory_bytes: 2 << 30,
            processes: 64,
            file_bytes: 64 << 20,
            trace_bytes: 24 << 20,
            tmp_bytes: 1 << 30,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("inputs.json"),
        &json!({"prior_spec":PRIOR_SPEC,"cvc_tools":tool.sha(),"profile":tool.name(),"phase_variant":phase,"lexer_mode":lexer,"phase_c":md_sim_firmware::prepare::hash_bytes(PHASE_C.as_bytes()),"phase_hdl":md_sim_firmware::prepare::hash_bytes(PHASE_HDL.as_bytes()),"icarus_tools":ICARUS,"compiler":COMPILER,"helper":hash_file(&std::env::current_exe()?)?,"oracle_c":md_sim_firmware::prepare::hash_bytes(C.as_bytes()),"oracle_hdl":md_sim_firmware::prepare::hash_bytes(HDL.as_bytes())}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let raw = hex::decode(&result.stdout)?;
    fs::write(out.join("envelope.json"), &raw)?;
    ensure!(!result.trace_truncated, "oracle output truncated");
    let mut report: Value =
        serde_json::from_slice(&raw).context("oracle report absent; raw retained")?;
    if let Some(hex) = report["compiled_archive_hex"].as_str() {
        let data = hex::decode(hex)?;
        let id: ObjectId = serde_json::from_value(report["compiled_archive"].clone())?;
        ensure!(
            id.size == data.len() as u64
                && id.sha256 == md_sim_firmware::prepare::hash_bytes(&data),
            "oracle artifact changed"
        );
        fs::write(out.join("compiled.tar.gz"), data)?;
    }
    report
        .as_object_mut()
        .context("oracle object")?
        .remove("compiled_archive_hex");
    save_json(&out.join("report.json"), &report)?;
    for m in &spec.inputs {
        store.verified_path(&m.object)?;
    }
    ensure!(
        result.exit_code == Some(0) && result.stderr.is_empty() && report["success"] == true,
        "scheduler contract failed; raw result retained"
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lexer_mode_does_not_change_legacy_phase_selection() {
        let parse = |words: &[&str]| {
            mode(
                &words
                    .iter()
                    .map(std::ffi::OsString::from)
                    .collect::<Vec<_>>(),
                0,
            )
        };
        assert_eq!(parse(&["--lexer"]).unwrap(), (Tool::Original, None, true));
        assert_eq!(
            parse(&["--rw-after-nba", "--lexer"]).unwrap(),
            (Tool::Rw, None, true)
        );
        assert!(parse(&["--rw-after-nba", "--phase", "nba", "--lexer"]).is_err());
        assert!(parse(&["--lexer", "--lexer"]).is_err());
        assert_eq!(
            parse(&["--rw-after-nba", "--lexer-context", "--lexer"]).unwrap(),
            (Tool::Lexer, None, true)
        );
        assert_eq!(
            parse(&["--rw-after-nba", "--lexer-context", "--phase", "nba"]).unwrap(),
            (Tool::Lexer, Some("nba"), false)
        );
        assert!(parse(&["--lexer-context", "--lexer"]).is_err());
        assert!(parse(&["--rw-after-nba", "--lexer-context", "--lexer-context"]).is_err());
    }
    #[test]
    fn variant_selection_is_explicit_and_closed() {
        let parse = |words: &[&str]| {
            rw_mode(
                &words
                    .iter()
                    .map(std::ffi::OsString::from)
                    .collect::<Vec<_>>(),
                0,
            )
        };
        assert_eq!(parse(&[]).unwrap(), (false, None));
        assert_eq!(parse(&["--rw-after-nba"]).unwrap(), (true, None));
        for phase in ["nba", "delayed", "no-nba"] {
            assert_eq!(
                parse(&["--rw-after-nba", "--phase", phase]).unwrap(),
                (true, Some(phase))
            );
        }
        for words in [
            &["--phase", "nba"][..],
            &["--rw-after-nba", "--phase", "bad"][..],
            &["--rw-after-nba", "extra"][..],
        ] {
            assert!(parse(words).is_err());
        }
    }
    #[test]
    fn phase_receipts_reject_duplicate_changed_and_early_states() {
        let line = "MD_RW_PHASE case=nested time=5 input=0 edge=1 sampled=0 echo=0 first=1 peer=1 nested=1 final=1 cancelled=0";
        let mut row = json!({"code":0,"stderr":"","stdout":format!("{line}\nMD_RW_PHASE_PASS\n")});
        phase_oracle(&row, "nested").unwrap();
        for bad in [
            format!("{line}\n{line}\nMD_RW_PHASE_PASS\n"),
            format!("{}\nMD_RW_PHASE_PASS", line.replace("echo=0", "echo=1")),
            "MD_RW_PHASE_PASS".into(),
        ] {
            row["stdout"] = json!(bad);
            assert!(phase_oracle(&row, "nested").is_err());
        }
    }
    #[test]
    fn rejects_missing_duplicate_wrong_or_diagnostic_receipts() {
        let mut row = json!({"code":0,"stdout":format!("{EXPECTED}\n{PASS}\n"),"stderr":""});
        oracle(&row).unwrap();
        for text in [
            format!("{PASS}\n"),
            format!("{EXPECTED}\n{PASS}\n{PASS}\n"),
            format!("MD_RW_ORDER time=5 input=1 edge=1 sampled=1\n{PASS}"),
            format!("{EXPECTED}\n{PASS}\nMD_RW_ORACLE_MISSING_CALLBACK"),
            format!("MD_RW_ORDER time=5 input=1 edge=1 sampled=1\n{EXPECTED}\n{PASS}"),
        ] {
            row["stdout"] = json!(text);
            assert!(oracle(&row).is_err());
        }
    }
}
