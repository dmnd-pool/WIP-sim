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
#[allow(dead_code)]
#[path = "support/qemu_build.rs"]
mod qemu_build;
#[path = "support/s9_ram_vectors.rs"]
mod ram;
const PROFILES: [&str; 2] = [
    "d6d730231227a2f6d299f4900b6868b858768459f9cceec07aa675269731658c",
    "f137f1e433e80074ecb539b8173bd993ce825ca5f1ccfe7f892f90eb0512f25b",
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
    let result = (|| -> Result<()> {
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
        let selected_args = if profile.axi.is_some() {
            axi::verify_notice_source(&fs::read_to_string("/vendor/PS7.v")?)?;
            axi::verify_contract(
                &fs::read("/axi-contract.json")?,
                &fs::read("/provenance/bosminer.elf")?,
                &fs::read("/provenance/bosminer-board.dtb")?,
            )?;
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
        report["compiled_archive_hex"] = json!(hex::encode(fs::read("/work/selected-vvp.tar.gz")?));
        report["selected_vvp_compile_pass"] = json!(true);
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
        Ok(())
    })();
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
    let store = Store::open(&out.join("inputs"), 128 << 20)?;
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
    for input in profile.files.iter().chain(&profile.provenance) {
        add(
            repo.join(&input.path),
            input.guest_path.clone(),
            Some(&input.object),
        )?;
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
        environment: BTreeMap::from([
            ("PATH".into(), "/work/iverilog-install/bin".into()),
            ("LC_ALL".into(), "C".into()),
        ]),
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
        fs::write(out.join("selected-vvp.tar.gz"), data)?;
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
            && report["selected_vvp_compile_pass"] == true,
        "incomplete HDL report"
    );
    ensure!(
        profile.axi.is_none() || report["selected_axi_pass"] == true,
        "selected AXI gate incomplete"
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
