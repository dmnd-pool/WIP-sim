//! Compile the exact reconstructed S9 programmable logic with pinned vendor models.
//! The vendor PS7 file is a port shell, not a processor implementation.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};
#[path = "support/s9_axi_vectors.rs"]
mod axi;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/cvc_engine.rs"]
mod cvc_engine;
// Different callers use either strict PL transactions or raw QEMU requests.
#[allow(dead_code)]
#[path = "support/framed_child.rs"]
mod framed_child;
#[path = "support/s9_live_child.rs"]
mod live_child;
#[path = "support/s9_live_protocol.rs"]
mod live_protocol;
#[path = "support/s9_live_scenario.rs"]
mod live_scenario;
#[path = "support/s9_live_wrapper.rs"]
mod live_wrapper;
#[allow(dead_code)]
#[path = "support/qemu_build.rs"]
mod qemu_build;
#[path = "support/s9_ram_vectors.rs"]
mod ram;
const PROFILES: [&str; 6] = [
    "d6d730231227a2f6d299f4900b6868b858768459f9cceec07aa675269731658c",
    "f137f1e433e80074ecb539b8173bd993ce825ca5f1ccfe7f892f90eb0512f25b",
    "47dc7c50b5eb0e89ce0836f2ae33e617fa6e6050028b7a09db9c94d2966b95b1",
    "251764b432069b365e1a0f6c96bf7a33a61e8d0c5e0b67bb6592ae1273e399a7",
    "52f1fd366f0e399f22298dbfafe89ae5f194b1dadf2a124e62cc122018880a34",
    "25f0aac6bb39af4536d2d5dd9143b8ed11a07ecaa0eca6365420e65feef3a773",
];
#[derive(Deserialize)]
struct Input {
    path: PathBuf,
    guest_path: String,
    object: ObjectId,
}
#[derive(Deserialize)]
struct Profile {
    files: Vec<Input>,
    netlist: NamedInput,
    tools: NamedInput,
    vectors: NamedInput,
    #[serde(default)]
    provenance: Vec<Input>,
    #[serde(default)]
    axi: Option<Axi>,
    #[serde(default)]
    live: Option<Live>,
    #[serde(default)]
    cvc: Option<NamedInput>,
}
#[derive(Deserialize)]
struct Live {
    shim: NamedInput,
    compiler_archive: NamedInput,
    compiler_manifest: NamedInput,
}
#[derive(Deserialize)]
struct Axi {
    ports: NamedInput,
    contract: NamedInput,
}
#[derive(Deserialize)]
struct NamedInput {
    path: PathBuf,
    object: ObjectId,
}
fn run_step(report: &mut Value, name: &str, argv: &[String]) -> Result<bool> {
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]).current_dir("/work");
    let pass = qemu_build::step(report, name, command)?;
    report[name]["argv"] = json!(argv);
    ensure!(
        report[name]["stdout_truncated"] == false && report[name]["stderr_truncated"] == false,
        "HDL diagnostics truncated"
    );
    Ok(pass)
}
fn clean_diagnostics(step: &Value) -> Result<()> {
    ensure!(step["code"] == 0, "HDL compiler failed");
    for stream in ["stdout", "stderr"] {
        ensure!(
            step[stream]
                .as_str()
                .context("compiler stream absent")?
                .trim()
                .is_empty(),
            "unresolved HDL compiler diagnostic in {stream}; no warning waiver applies"
        );
    }
    Ok(())
}
fn diagnostic_oracles(report: &mut Value) -> Result<()> {
    for (name,source,expect_zero) in [
        ("width_warning", "module child(input [3:0] value); endmodule\nmodule bad; child c(2'b00); endmodule\n",true),
        ("implicit_warning", "module child(input value); endmodule\nmodule bad; child c(undeclared); endmodule\n",true),
        ("parameter_warning", "module child #(parameter PRESENT=0)(); endmodule\nmodule bad; child #(.ABSENT(1)) c(); endmodule\n",false),
    ] {
        let path=format!("/work/{name}.v");fs::write(&path,source)?;
        let argv=["/work/iverilog-install/bin/iverilog","-g2012","-Wall","-s","bad","-t","null",&path].into_iter().map(str::to_owned).collect::<Vec<_>>();
        let success=run_step(report,name,&argv)?;
        ensure!(report[name]["code"].as_i64().is_some(), "diagnostic oracle terminated without an exit code");
        ensure!(!expect_zero||success,"warning oracle did not exercise exit-zero diagnostics");
        let diagnostic = report[name]["stderr"].as_str().context("negative compiler stderr")?;
        let fragments: &[&str] = match name {
            "width_warning" => &["expects 4 bits, got 2", "Padding"],
            "implicit_warning" => &["implicit", "undeclared"],
            "parameter_warning" => &["parameter", "ABSENT"],
            _ => unreachable!(),
        };
        ensure!(fragments.iter().all(|part| diagnostic.contains(part)), "expected semantic diagnostic absent");
        ensure!(clean_diagnostics(&report[name]).is_err(),"diagnostic policy accepted malformed structure");
        report[name]["policy_rejected"]=json!(true);
    }
    Ok(())
}
fn compile_args(profile: &Profile, source: &str, top: &str, output: Option<&str>) -> Vec<String> {
    let mut argv: Vec<String> = [
        "/work/iverilog-install/bin/iverilog",
        "-g2012",
        "-Ttyp",
        "-Wall",
        "-s",
        top,
        "-s",
        "glbl",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    match output {
        Some(path) => argv.extend(["-o".into(), path.into()]),
        None => argv.extend(["-t".into(), "null".into()]),
    };
    argv.push(source.into());
    argv.extend(profile.files.iter().map(|f| f.guest_path.clone()));
    argv
}
fn ram_differential(report: &mut Value, profile: &Profile) -> Result<()> {
    let netlist = fs::read_to_string("/selected.v")?;
    let (source, inventory) = ram::generate(&netlist)?;
    report["ram_vectors"] = inventory;
    report["ram_vectors"]["source_sha256"] =
        json!(md_sim_firmware::prepare::hash_bytes(source.as_bytes()));
    report["ram_vectors"]["source"] = json!(source);
    fs::write("/work/ram-vectors.v", &source)?;
    for (label, explicit) in [("omitted", false), ("explicit_z", true)] {
        let output = format!("/work/ram-{label}.vvp");
        let name = format!("ram_{label}_compile");
        let mut args = compile_args(profile, "/work/ram-vectors.v", "ram_vectors", Some(&output));
        if explicit {
            args.insert(1, "-DEXPLICIT_OPEN".into());
        }
        ensure!(
            run_step(report, &name, &args)?,
            "RAM fixture compiler failed"
        );
        if explicit {
            clean_diagnostics(&report[&name])?;
        } else {
            ensure!(
                report[&name]["stdout"]
                    .as_str()
                    .context("RAM compiler stdout")?
                    .is_empty(),
                "unexpected RAM compiler output"
            );
            ram::check_omission_diagnostics(
                report[&name]["stderr"]
                    .as_str()
                    .context("RAM compiler stderr")?,
            )?;
        }
        let name = format!("ram_{label}_execute");
        ensure!(
            run_step(
                report,
                &name,
                &["/work/iverilog-install/bin/vvp".into(), output]
            )?,
            "RAM fixture failed"
        );
        ensure!(
            report[&name]["stderr"]
                .as_str()
                .context("RAM runtime stderr")?
                .is_empty(),
            "unexpected RAM runtime diagnostic"
        );
        let stdout = report[&name]["stdout"]
            .as_str()
            .context("RAM runtime stdout")?;
        ensure!(
            stdout
                .lines()
                .any(|line| line == "MD_SIM_RAM_OMISSION_PASS cases=510"),
            "RAM fixture completion absent"
        );
    }
    // The VVP finish location is identical because both runs use the same source.
    ensure!(
        report["ram_omitted_execute"]["stdout"] == report["ram_explicit_z_execute"]["stdout"],
        "RAM omitted versus explicit-Z traces differ"
    );
    report["ram_omission_differential_pass"] = json!(true);
    Ok(())
}

fn run_live(report: &mut Value, profile: &Profile) -> Result<()> {
    qemu_build::unpack("/compiler.tar.gz", "/work")?;
    let cvc = profile.cvc.is_some();
    if cvc {
        cvc_engine::vectors(
            report,
            &profile
                .files
                .iter()
                .map(|f| f.guest_path.clone())
                .collect::<Vec<_>>(),
        )?;
    }
    let mut compiler_args = [
        "--sysroot=/work/toolchain",
        "-B/work/toolchain/usr/bin",
        "-std=c11",
        "-O2",
        "-fPIC",
        "-shared",
        "-nostdlib",
        "-fno-builtin",
        "-Wall",
        "-Wextra",
        "-Werror",
        "-fsanitize=undefined",
        "-fsanitize-undefined-trap-on-error",
        "-fno-sanitize-recover=all",
        "-Wl,-z,noexecstack",
        "-I/work/iverilog-install/include/iverilog",
        "/vpi-shim.c",
        "-o",
        "/work/compiled/mdsim_live.vpi",
    ]
    .map(str::to_owned)
    .to_vec();
    if cvc {
        let header = compiler_args
            .iter_mut()
            .find(|s| s.as_str() == "-I/work/iverilog-install/include/iverilog")
            .context("VPI header flag")?;
        *header = "-I/work/cvc-install/pli_incs".into();
        compiler_args.insert(0, "-DMD_SIM_CVC".into());
    }
    let mut compiler = Command::new("/work/toolchain/usr/bin/gcc");
    compiler
        .args(&compiler_args)
        .env("LD_LIBRARY_PATH", "/work/toolchain/usr/lib")
        .current_dir("/work");
    let built = qemu_build::step(report, "vpi_compile", compiler)?;
    report["vpi_compile"]["argv"] = json!(compiler_args);
    report["vpi_compile"]["environment_override"] =
        json!({"LD_LIBRARY_PATH":"/work/toolchain/usr/lib"});
    ensure!(built, "VPI adapter compile failed");
    ensure!(
        report["vpi_compile"]["stdout_truncated"] == false
            && report["vpi_compile"]["stderr_truncated"] == false,
        "VPI compiler output incomplete"
    );
    clean_diagnostics(&report["vpi_compile"])?;
    report["vpi_adapter"] = json!(hash_file(Path::new("/work/compiled/mdsim_live.vpi"))?);
    // This isolated compiler tree is temporary. The pinned archive and full
    // compile diagnostics remain inputs/evidence; the actual plugin is retained.
    if !cvc {
        fs::remove_dir_all("/work/toolchain")?;
        report["temporary_compiler_removed_after_build"] = json!(true);
    }
    run_vpi_oracles(report, cvc)?;
    let (source, inventory) = live_wrapper::generate(
        &fs::read_to_string("/selected.v")?,
        &fs::read("/axi-ports.json")?,
    )?;
    report["live_wrapper"] = inventory;
    report["live_wrapper"]["source"] = json!(source);
    fs::write("/work/compiled/live-probe.v", source)?;
    let model = if cvc {
        "/work/compiled/live-cvc"
    } else {
        "/work/compiled/live.vvp"
    };
    let args = if cvc {
        cvc_engine::compile_args(
            &["/work/compiled/live-probe.v", "/selected.v"],
            model,
            &profile
                .files
                .iter()
                .map(|f| f.guest_path.clone())
                .collect::<Vec<_>>(),
            &[],
            true,
        )
    } else {
        let mut args = compile_args(
            profile,
            "/work/compiled/live-probe.v",
            "axi_probe",
            Some(model),
        );
        args.push("/selected.v".into());
        args
    };
    ensure!(
        run_step(report, "live_compile", &args)?,
        "live selected compilation failed"
    );
    clean_live(&report["live_compile"], cvc)?;
    report["selected_program"] = json!(hash_file(Path::new(model))?);
    report["selected_program_format"] = json!(if cvc { "CVC native ELF" } else { "Icarus VVP" });
    report["selected_program_compile_pass"] = json!(true);
    report["selected_vvp_compile_pass"] = json!(!cvc);
    if cvc {
        fs::remove_dir_all("/work/toolchain")?;
        report["temporary_compiler_removed_after_build"] = json!(true);
    }
    let command = live_command(model, cvc);
    report["live_child_argv"] = json!(std::iter::once(
        command.get_program().to_string_lossy().into_owned()
    )
    .chain(command.get_args().map(|a| a.to_string_lossy().into_owned()))
    .collect::<Vec<_>>());
    let mut child = live_child::LiveChild::launch(
        command,
        live_child::ChildLimits {
            command_timeout_ms: 10_000,
            shutdown_timeout_ms: 5_000,
            stdout_limit: 1 << 20,
            stderr_limit: 1 << 20,
            frame_limit: 8192,
        },
        Arc::new(AtomicBool::new(false)),
    )?;
    let contract = serde_json::from_slice(&fs::read("/axi-contract.json")?)?;
    let result = live_scenario::selected(&mut child, &contract);
    if let Err(error) = &result {
        let _ = child.abort(&format!("selected live assertion: {error:#}"));
    }
    report["live_child_evidence"] = serde_json::to_value(child.evidence())?;
    match result {
        Ok(scenario) => {
            let evidence = &report["live_child_evidence"];
            ensure!(
                evidence["stderr_hex"] == "",
                "unexpected live VPI/runtime diagnostic"
            );
            let stdout = String::from_utf8(hex::decode(
                evidence["stdout_hex"].as_str().context("live stdout")?,
            )?)?;
            live_scenario::verify_stdout(live_body(&stdout, cvc)?, &scenario)?;
            report["live_scenario"] = scenario;
            report["selected_live_pass"] = json!(true);
            report["selected_axi_pass"] = json!(true);
        }
        Err(error) => {
            report["live_scenario_error"] = json!(format!("{error:#}"));
            return Err(error);
        }
    }

    Ok(())
}

fn clean_live(row: &Value, cvc: bool) -> Result<()> {
    if cvc {
        cvc_engine::clean(row)
    } else {
        clean_diagnostics(row)
    }
}
fn live_body(stdout: &str, cvc: bool) -> Result<&str> {
    if cvc {
        cvc_engine::body(stdout)
    } else {
        Ok(stdout)
    }
}
fn live_command(model: &str, cvc: bool) -> Command {
    if cvc {
        return cvc_engine::command(model);
    }
    let mut command = Command::new("/work/iverilog-install/bin/vvp");
    command
        .args(["-M/work/compiled", "-mmdsim_live", model])
        .current_dir("/work");
    command
}
fn fixture_args(cvc: bool, source: &str, top: &str, output: &str, defines: &[&str]) -> Vec<String> {
    if cvc {
        return cvc_engine::compile_args(&[source], output, &[], defines, true);
    }
    let mut args = [
        "/work/iverilog-install/bin/iverilog",
        "-g2012",
        "-Wall",
        "-s",
        top,
        "-o",
        output,
        source,
    ]
    .map(str::to_owned)
    .to_vec();
    for define in defines {
        args.insert(1, format!("-D{define}"));
    }
    args
}
fn empty_live_output(evidence: &Value, cvc: bool) -> Result<()> {
    ensure!(evidence["stderr_hex"] == "", "live fixture diagnostic");
    let stdout = String::from_utf8(hex::decode(
        evidence["stdout_hex"].as_str().context("fixture stdout")?,
    )?)?;
    ensure!(
        live_body(&stdout, cvc)?.is_empty(),
        "live fixture unexpected output"
    );
    Ok(())
}

fn verify_duplicate_failure(evidence: &Value, cvc: bool) -> Result<()> {
    let error = String::from_utf8(hex::decode(
        evidence["stderr_hex"]
            .as_str()
            .context("negative fixture stderr")?,
    )?)?;
    ensure!(
        error == "MDPL_FAILURE sequence=1 time_ps=1000 reason=unexpected-completion-task\n",
        "negative fixture diagnostic differs"
    );
    let stdout = String::from_utf8(hex::decode(
        evidence["stdout_hex"]
            .as_str()
            .context("negative fixture stdout")?,
    )?)?;
    ensure!(
        live_body(&stdout, cvc)?.is_empty(),
        "negative fixture unexpected stdout"
    );
    ensure!(
        evidence["cleanup_error"].is_null()
            && evidence["exit"]["code"] == 1
            && evidence["exit"]["signal"].is_null(),
        "negative VPI failure or cleanup differs"
    );
    Ok(())
}

fn run_vpi_oracles(report: &mut Value, cvc: bool) -> Result<()> {
    const FIXTURE: &str = include_str!("../../../simulator/backends/icarus/vpi-order-fixture.v");
    fs::write("/work/compiled/vpi-order-fixture.v", FIXTURE)?;
    report["vpi_fixture_source_sha256"] =
        json!(md_sim_firmware::prepare::hash_bytes(FIXTURE.as_bytes()));
    for (label, negative) in [("event_regions", false), ("duplicate_completion", true)] {
        let model = format!(
            "/work/compiled/{label}.{}",
            if cvc { "native" } else { "vvp" }
        );
        let args = fixture_args(
            cvc,
            "/work/compiled/vpi-order-fixture.v",
            "vpi_order",
            &model,
            if negative {
                &["MD_DUPLICATE_COMPLETE"]
            } else {
                &[]
            },
        );
        let compile = format!("{label}_compile");
        ensure!(
            run_step(report, &compile, &args)?,
            "VPI event fixture compile failed"
        );
        clean_live(&report[&compile], cvc)?;
        let mut child = live_child::LiveChild::launch(
            live_command(&model, cvc),
            live_child::ChildLimits {
                command_timeout_ms: 5_000,
                shutdown_timeout_ms: 5_000,
                stdout_limit: 64 << 10,
                stderr_limit: 64 << 10,
                frame_limit: 256,
            },
            Arc::new(AtomicBool::new(false)),
        )?;
        if negative {
            child.ready()?;
            let response = child.transact(&live_protocol::Request {
                sequence: 1,
                at_ps: 1000,
                operation: live_protocol::Op::Advance,
                args: [0; 12],
            });
            if response.is_ok() {
                let _ = child.abort("duplicate completion was incorrectly accepted");
            }
            let evidence = serde_json::to_value(child.evidence())?;
            report[label] = json!({"expected_rejection":true,"request_failed":response.is_err(),"evidence":evidence});
            ensure!(
                response.is_err(),
                "VPI duplicate completion incorrectly passed"
            );
            verify_duplicate_failure(&evidence, cvc)?;
            report[label]["passed"] = json!(true);
        } else {
            let result = live_scenario::event_regions(&mut child);
            if let Err(error) = &result {
                let _ = child.abort(&format!("VPI event assertion: {error:#}"));
            }
            report[label] = json!({"evidence":child.evidence()});
            report[label]["assessment"] = result?;
            let evidence = &report[label]["evidence"];
            empty_live_output(evidence, cvc)?;
        }
    }
    run_clock_oracle(report, cvc)
}

fn run_clock_oracle(report: &mut Value, cvc: bool) -> Result<()> {
    let source = live_wrapper::clock_fixture();
    fs::write("/work/compiled/clock-order-fixture.v", &source)?;
    report["clock_fixture_source_sha256"] =
        json!(md_sim_firmware::prepare::hash_bytes(source.as_bytes()));
    let model = if cvc {
        "/work/compiled/clock-order.native"
    } else {
        "/work/compiled/clock-order.vvp"
    };
    let unknown_model = if cvc {
        "/work/compiled/clock-unknown.native"
    } else {
        "/work/compiled/clock-unknown.vvp"
    };
    for (key, output, defines) in [
        ("clock_compile", model, &[][..]),
        (
            "clock_unknown_compile",
            unknown_model,
            &["MD_UNKNOWN_CLOCK"][..],
        ),
    ] {
        let args = fixture_args(
            cvc,
            "/work/compiled/clock-order-fixture.v",
            "clock_order",
            output,
            defines,
        );
        ensure!(
            run_step(report, key, &args)?,
            "clock fixture compile failed"
        );
        clean_live(&report[key], cvc)?;
    }
    for (label, split, unknown_start) in [
        ("clock_unsplit", false, false),
        ("clock_split", true, false),
        ("clock_unknown", false, true),
    ] {
        let mut child = live_child::LiveChild::launch(
            live_command(if unknown_start { unknown_model } else { model }, cvc),
            live_child::ChildLimits {
                command_timeout_ms: 5_000,
                shutdown_timeout_ms: 5_000,
                stdout_limit: 64 << 10,
                stderr_limit: 64 << 10,
                frame_limit: 256,
            },
            Arc::new(AtomicBool::new(false)),
        )?;
        let result = live_scenario::clock_boundary(&mut child, split, unknown_start);
        if let Err(error) = &result {
            let _ = child.abort(&format!("clock boundary assertion: {error:#}"));
        }
        report[label] = json!({"evidence":child.evidence()});
        report[label]["assessment"] = result?;
        empty_live_output(&report[label]["evidence"], cvc)?;
    }
    ensure!(
        report["clock_unsplit"]["assessment"]["events"]
            == report["clock_split"]["assessment"]["events"],
        "clock/input behavior changes with Advance partitioning"
    );
    report["clock_chunk_invariant"] = json!(true);
    Ok(())
}

fn inner() -> Result<()> {
    confinement::require_netlist_preparation()?;
    let raw = fs::read("/profile.json")?;
    ensure!(
        PROFILES.contains(&md_sim_firmware::prepare::hash_bytes(&raw).as_str()),
        "HDL profile differs"
    );
    let profile: Profile = serde_json::from_slice(&raw)?;
    for (path, id) in [
        ("/tools.tar.gz", &profile.tools.object),
        ("/selected.v", &profile.netlist.object),
        ("/vectors.v", &profile.vectors.object),
    ] {
        ensure!(
            hash_file(Path::new(path))? == *id,
            "selected HDL input differs"
        );
    }
    if let Some(axi) = &profile.axi {
        for (path, id) in [
            ("/axi-ports.json", &axi.ports.object),
            ("/axi-contract.json", &axi.contract.object),
        ] {
            ensure!(
                hash_file(Path::new(path))? == *id,
                "selected AXI input differs"
            );
        }
    }
    if let Some(live) = &profile.live {
        for (path, id) in [
            ("/vpi-shim.c", &live.shim.object),
            ("/compiler.tar.gz", &live.compiler_archive.object),
            ("/compiler-manifest.json", &live.compiler_manifest.object),
        ] {
            ensure!(hash_file(Path::new(path))? == *id, "live PL input differs");
        }
    }
    if let Some(cvc) = &profile.cvc {
        ensure!(
            profile.live.is_some() && profile.axi.is_some(),
            "CVC profile requires live AXI contract"
        );
        ensure!(
            hash_file(Path::new("/cvc-tools.tar.gz"))? == cvc.object,
            "CVC tool differs"
        );
        qemu_build::unpack("/cvc-tools.tar.gz", "/work")?;
    }
    for input in profile.files.iter().chain(&profile.provenance) {
        ensure!(
            hash_file(Path::new(&input.guest_path))? == input.object,
            "vendor input differs"
        );
    }
    let assessment: Value =
        serde_json::from_slice(&fs::read("/provenance/conversion-assessment.json")?)?;
    let structure: Value =
        serde_json::from_slice(&fs::read("/provenance/structure-comparison.json")?)?;
    ensure!(
        assessment["converted"] == true && structure["passed"] == true,
        "selected converter preservation evidence failed"
    );
    ensure!(
        assessment["manifest"]
            == serde_json::to_value(hash_file(Path::new("/provenance/converter-inputs.json"))?)?,
        "converter evidence manifest differs"
    );
    qemu_build::unpack("/tools.tar.gz", "/work")?;
    let mut report = json!({"schema":1,"profile_sha256":md_sim_firmware::prepare::hash_bytes(&raw),"selected_axi_pass":false,"primitive_vectors_pass":false,"ram_omission_differential_pass":false,"selected_elaboration_pass":false,"selected_vvp_compile_pass":false,"original_board_gate":false,"diagnostic_policy":"Every nonempty selected compiler output is unresolved and prevents structural pass; no warning whitelist. Timing/model warnings are not timing acceptance.","min_typ_max_policy":"Explicit Icarus -Ttyp selects its existing default; selected PS7 values are100:100:100. No XIL_TIMING or physical calibration claim.","limits":["Vendor PS7 has no behavioral CPU model; compiler resolution is not AXI execution.","Missing monitor feature table and33 electrical ambiguities remain open.","Digital simulation delays are vendor model semantics, not physical timing calibration."]});
    report["structural_checker"] = json!("Pinned Icarus, unchanged strict diagnostic oracles");
    report["behavioral_engine"] = json!(if let Some(cvc) = &profile.cvc {
        format!(
            "CVC {}; additional actual primitive/RAM/live execution",
            cvc.object.sha256
        )
    } else {
        "Icarus".to_owned()
    });
    let mut result = (|| -> Result<()> {
        diagnostic_oracles(&mut report)?;
        ram_differential(&mut report, &profile)?;
        ensure!(
            run_step(
                &mut report,
                "primitive_compile",
                &compile_args(
                    &profile,
                    "/vectors.v",
                    "primitive_vectors",
                    Some("/work/vectors.vvp")
                )
            )?,
            "vendor primitive compile failed"
        );
        clean_diagnostics(&report["primitive_compile"])?;
        ensure!(
            run_step(
                &mut report,
                "primitive_execute",
                &[
                    "/work/iverilog-install/bin/vvp".into(),
                    "/work/vectors.vvp".into()
                ]
            )?,
            "vendor primitive vectors failed"
        );
        ensure!(
            report["primitive_execute"]["stderr"]
                .as_str()
                .context("primitive runtime stderr")?
                .trim()
                .is_empty(),
            "unresolved primitive runtime diagnostic"
        );
        ensure!(
            report["primitive_execute"]["stdout"]
                .as_str()
                .context("primitive stdout")?
                .lines()
                .any(|l| l == "MD_SIM_VENDOR_PRIMITIVES_PASS cases=582"),
            "complete primitive vector result absent"
        );
        report["primitive_vectors_pass"] = json!(true);
        ensure!(
            run_step(
                &mut report,
                "selected_elaborate",
                &compile_args(&profile, "/selected.v", "top", None)
            )?,
            "selected netlist elaboration failed"
        );
        clean_diagnostics(&report["selected_elaborate"])?;
        report["selected_elaboration_pass"] = json!(true);
        fs::create_dir("/work/compiled")?;
        if profile.axi.is_some() {
            axi::verify_notice_source(&fs::read_to_string("/vendor/PS7.v")?)?;
            axi::verify_contract(
                &fs::read("/axi-contract.json")?,
                &fs::read("/provenance/bosminer.elf")?,
                &fs::read("/provenance/bosminer-board.dtb")?,
            )?;
        }
        if profile.live.is_none() {
            let selected_args = if profile.axi.is_some() {
                let (source, inventory) = axi::generate(
                    &fs::read_to_string("/selected.v")?,
                    &fs::read("/axi-ports.json")?,
                    &fs::read("/axi-contract.json")?,
                )?;
                report["axi_vectors"] = inventory;
                report["axi_vectors"]["source"] = json!(source);
                fs::write("/work/compiled/axi-probe.v", source)?;
                let mut args = compile_args(
                    &profile,
                    "/work/compiled/axi-probe.v",
                    "axi_probe",
                    Some("/work/compiled/selected.vvp"),
                );
                args.push("/selected.v".into());
                args
            } else {
                compile_args(
                    &profile,
                    "/selected.v",
                    "top",
                    Some("/work/compiled/selected.vvp"),
                )
            };
            ensure!(
                run_step(&mut report, "selected_compile", &selected_args)?,
                "selected VVP compile failed"
            );
            clean_diagnostics(&report["selected_compile"])?;
            report["selected_program"] =
                serde_json::to_value(hash_file(Path::new("/work/compiled/selected.vvp"))?)?;
            qemu_build::pack_dir(
                Path::new("/work/compiled"),
                "compiled",
                Path::new("/work/selected-vvp.tar.gz"),
            )?;
            let archive = hash_file(Path::new("/work/selected-vvp.tar.gz"))?;
            ensure!(archive.size <= 32 << 20, "compiled model archive bound");
            report["compiled_archive"] = serde_json::to_value(archive)?;
            report["compiled_archive_hex"] =
                json!(hex::encode(fs::read("/work/selected-vvp.tar.gz")?));
            report["selected_vvp_compile_pass"] = json!(true);
            report["selected_program_compile_pass"] = json!(true);
            if profile.axi.is_some() {
                ensure!(
                    run_step(
                        &mut report,
                        "selected_axi_execute",
                        &[
                            "/work/iverilog-install/bin/vvp".into(),
                            "/work/compiled/selected.vvp".into()
                        ]
                    )?,
                    "selected AXI execution failed"
                );
                ensure!(
                    report["selected_axi_execute"]["stderr"]
                        .as_str()
                        .context("AXI stderr")?
                        .is_empty(),
                    "selected AXI runtime diagnostic"
                );
                let trace = report["selected_axi_execute"]["stdout"]
                    .as_str()
                    .context("AXI stdout")?;
                report["axi_trace_validation"] =
                    axi::check_trace(trace, &fs::read("/axi-contract.json")?)?;
                report["selected_axi_pass"] = json!(true);
            }
        }
        if profile.live.is_some() {
            ensure!(
                profile.axi.is_some(),
                "live selected mode requires checked AXI contract"
            );
            report["selected_execution_mode"] = json!("live-private-vpi");
            report["native_baseline_reused"] = json!(false);
            run_live(&mut report, &profile)?;
        }
        Ok(())
    })();
    if profile.live.is_some() && Path::new("/work/compiled").is_dir() {
        let retention = (|| -> Result<()> {
            qemu_build::pack_dir(
                Path::new("/work/compiled"),
                "compiled",
                Path::new("/work/selected-vvp.tar.gz"),
            )?;
            let archive = hash_file(Path::new("/work/selected-vvp.tar.gz"))?;
            ensure!(archive.size <= 32 << 20, "compiled model archive bound");
            report["compiled_archive"] = json!(archive);
            report["compiled_archive_hex"] =
                json!(hex::encode(fs::read("/work/selected-vvp.tar.gz")?));
            Ok(())
        })();
        if let Err(error) = retention {
            report["live_retention_error"] = json!(format!("{error:#}"));
            if result.is_ok() {
                result = Err(error);
            }
        }
    }
    if let Err(error) = &result {
        report["error"] = json!(format!("{error:#}"));
    }
    serde_json::to_writer(std::io::stdout(), &report)?;
    result
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        return inner();
    }
    ensure!(
        args.len() == 4,
        "usage: s9_hdl_validate REPO EXACT_PROFILE NEW_OUTPUT"
    );
    let repo = PathBuf::from(&args[1]).canonicalize()?;
    let profile_path = PathBuf::from(&args[2]);
    let out = PathBuf::from(&args[3]);
    ensure!(!out.exists(), "new output required");
    let raw = fs::read(&profile_path)?;
    ensure!(
        PROFILES.contains(&md_sim_firmware::prepare::hash_bytes(&raw).as_str()),
        "profile differs"
    );
    let profile: Profile = serde_json::from_slice(&raw)?;
    fs::create_dir_all(&out)?;
    let store = Store::open(
        &out.join("inputs"),
        if profile.live.is_some() {
            512 << 20
        } else {
            128 << 20
        },
    )?;
    // Capture explicit candidate runtime libraries. The selected isolated
    // compilation and primitive execution are the actual closure test.
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/guard".into(), std::env::current_exe()?),
            ("/bin/sh".into(), PathBuf::from("/usr/bin/bash")),
            (
                "/usr/lib/libstdc++.so.6".into(),
                PathBuf::from("/usr/lib/libstdc++.so.6"),
            ),
            (
                "/usr/lib/libgcc_s.so.1".into(),
                PathBuf::from("/usr/lib/libgcc_s.so.1"),
            ),
            (
                "/usr/lib/libreadline.so.8".into(),
                PathBuf::from("/usr/lib/libreadline.so.8"),
            ),
            (
                "/usr/lib/libz.so.1".into(),
                PathBuf::from("/usr/lib/libz.so.1"),
            ),
            (
                "/usr/lib/libbz2.so.1.0".into(),
                PathBuf::from("/usr/lib/libbz2.so.1.0"),
            ),
        ],
    )?;
    let mut add = |path: PathBuf, guest: String, expected: Option<&ObjectId>| -> Result<()> {
        let object = hash_file(&path)?;
        ensure!(expected.is_none_or(|x| *x == object), "HDL input changed");
        ensure!(
            !inputs.iter().any(|m| m.guest_path == guest),
            "duplicate input"
        );
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest,
        });
        Ok(())
    };
    add(profile_path, "/profile.json".into(), None)?;
    for (name, input) in [
        ("tools.tar.gz", &profile.tools),
        ("selected.v", &profile.netlist),
        ("vectors.v", &profile.vectors),
    ] {
        add(
            repo.join(&input.path),
            format!("/{name}"),
            Some(&input.object),
        )?;
    }
    if let Some(axi) = &profile.axi {
        for (name, input) in [
            ("axi-ports.json", &axi.ports),
            ("axi-contract.json", &axi.contract),
        ] {
            add(
                repo.join(&input.path),
                format!("/{name}"),
                Some(&input.object),
            )?;
        }
    }
    if let Some(live) = &profile.live {
        for (name, input) in [
            ("vpi-shim.c", &live.shim),
            ("compiler.tar.gz", &live.compiler_archive),
            ("compiler-manifest.json", &live.compiler_manifest),
        ] {
            add(
                repo.join(&input.path),
                format!("/{name}"),
                Some(&input.object),
            )?;
        }
    }
    if let Some(cvc) = &profile.cvc {
        add(
            repo.join(&cvc.path),
            "/cvc-tools.tar.gz".into(),
            Some(&cvc.object),
        )?;
    }
    for input in profile.files.iter().chain(&profile.provenance) {
        add(
            repo.join(&input.path),
            input.guest_path.clone(),
            Some(&input.object),
        )?;
    }
    let mut environment = BTreeMap::from([
        ("PATH".into(), "/work/iverilog-install/bin".into()),
        ("LC_ALL".into(), "C".into()),
    ]);
    if profile.cvc.is_some() {
        environment.insert(
            "PATH".into(),
            "/work/toolchain/usr/bin:/work/iverilog-install/bin".into(),
        );
        environment.insert("LD_LIBRARY_PATH".into(), "/work/toolchain/usr/lib".into());
    }
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
            wall_seconds: 900,
            cpu_seconds: 600,
            memory_bytes: 4 << 30,
            processes: 32,
            file_bytes: 1 << 30,
            trace_bytes: 96 << 20,
            tmp_bytes: 1 << 30,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let raw = hex::decode(&result.stdout)?;
    fs::write(out.join("hdl-envelope.json"), &raw)?;
    let mut report: Value =
        serde_json::from_slice(&raw).context("HDL report absent; raw evidence retained")?;
    if let Some(data) = report["compiled_archive_hex"].as_str() {
        let data = hex::decode(data)?;
        let id: ObjectId = serde_json::from_value(report["compiled_archive"].clone())?;
        ensure!(
            data.len() as u64 == id.size
                && md_sim_firmware::prepare::hash_bytes(&data) == id.sha256,
            "compiled model output differs"
        );
        fs::write(
            out.join(if profile.cvc.is_some() {
                "selected-native.tar.gz"
            } else {
                "selected-vvp.tar.gz"
            }),
            data,
        )?;
    }
    report
        .as_object_mut()
        .context("HDL report object")?
        .remove("compiled_archive_hex");
    save_json(&out.join("hdl-report.json"), &report)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "selected HDL check failed; bounded evidence retained"
    );
    ensure!(
        report["ram_omission_differential_pass"] == true
            && report["primitive_vectors_pass"] == true
            && report["selected_elaboration_pass"] == true
            && report["selected_program_compile_pass"] == true,
        "incomplete HDL report"
    );
    ensure!(
        profile.axi.is_none() || report["selected_axi_pass"] == true,
        "selected AXI gate incomplete"
    );
    ensure!(
        profile.live.is_none() || report["selected_live_pass"] == true,
        "selected live PL gate incomplete"
    );
    ensure!(
        profile.cvc.is_none()
            || (report["cvc_primitive_vectors_pass"] == true
                && report["cvc_ram_omission_differential_pass"] == true),
        "CVC behavior gates incomplete"
    );
    for m in &spec.inputs {
        store.verified_path(&m.object)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_completion_rejects_extra_diagnostics_and_wrong_banner() {
        let stderr = "MDPL_FAILURE sequence=1 time_ps=1000 reason=unexpected-completion-task\n";
        let mut row = json!({"stderr_hex":hex::encode(stderr),"stdout_hex":hex::encode(cvc_engine::BANNER),"exit":{"code":1,"signal":null},"cleanup_error":null});
        verify_duplicate_failure(&row, true).unwrap();
        row["stderr_hex"] = json!(hex::encode(format!("{stderr}extra warning\n")));
        assert!(verify_duplicate_failure(&row, true).is_err());
        row["stderr_hex"] = json!(hex::encode(stderr));
        row["stdout_hex"] = json!("");
        assert!(verify_duplicate_failure(&row, true).is_err());
        verify_duplicate_failure(&row, false).unwrap();
        row["exit"]["code"] = json!(0);
        assert!(verify_duplicate_failure(&row, false).is_err());
    }
    #[test]
    fn success_exit_cannot_hide_semantic_diagnostics() {
        // Icarus12.0 elaborate.cc lines1871–1904 prints the port-width warning
        // and padding/pruning details. An exit-zero compiler result is not a
        // clean structural check when these diagnostics are present.
        for text in [
            "selected.v:1: warning: Port 1 (value) of child expects 4 bits, got 2.",
            "selected.v:1:        : Padding 2 high bits of the port.",
            "warning: implicit wire",
            "warning: parameter ABSENT not found",
        ] {
            assert!(clean_diagnostics(&json!({"code":0,"stdout":"","stderr":text})).is_err());
        }
        assert!(clean_diagnostics(&json!({"code":1,"stdout":"","stderr":""})).is_err());
        clean_diagnostics(&json!({"code":0,"stdout":"","stderr":""})).unwrap();
    }
}
