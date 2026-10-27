//! Invoke an existing pinned converter on the checked original FPGA feature file.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::prepare::hash_bytes;
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store, StoreLimits},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[allow(dead_code)]
#[path = "support/fpga_feature_inputs.rs"]
mod inputs;
#[path = "support/netlist_inputs.rs"]
mod netlist;
#[path = "support/python_tool_inputs.rs"]
mod python;
#[path = "support/fasm_tool_inputs.rs"]
mod tool;

#[derive(Deserialize)]
struct Summary {
    schema: String,
    passed: bool,
    part: String,
    configuration_sha256: String,
    merged_names_sha256: String,
    merged_name_count: u64,
    unknown_ecc_bits: u64,
    unknown_non_ecc: Vec<Value>,
    roundtrip: RoundTrip,
}
#[derive(Deserialize)]
struct RoundTrip {
    passed: bool,
    error: Option<String>,
    original_set_bits: u64,
    regenerated_set_bits: u64,
    original_bits_sha256: String,
    regenerated_bits_sha256: String,
    missing_count: u64,
    extra_count: u64,
}
#[derive(Deserialize)]
struct Output {
    schema: String,
    converted: bool,
    error: Option<Value>,
    graph_error: Option<Value>,
    graph: Option<Graph>,
    files: BTreeMap<String, Artifact>,
    parser: Option<Parser>,
    parser_fixture: Option<Parser>,
    parser_calls: u64,
    parser_exhausted: bool,
    parsed_names_sha256: Option<String>,
    parsed_names_count: Option<u64>,
    tool_adaptations: Value,
    converter_adaptation: Value,
    parser_adaptation: String,
    converter_argv: Option<Vec<String>>,
    scope: String,
}
#[derive(Deserialize)]
struct Artifact {
    object: ObjectId,
    text: String,
}
#[derive(Deserialize)]
struct Parser {
    chunks: u64,
    lines: u64,
    maximum_chunk_bytes: u64,
    chunk_limit_bytes: u64,
    consumed_sha256: String,
    consumed_bytes: u64,
}
#[derive(Deserialize)]
struct Graph {
    sha256: String,
    size: u64,
    quick_check: Vec<String>,
    counts: BTreeMap<String, u64>,
    unassigned_wires: u64,
    missing_node_references: u64,
    missing_tile_references: u64,
}

fn checked_json<T: for<'de> Deserialize<'de>>(
    reference: &tool::Reference,
    bound: u64,
) -> Result<T> {
    let bytes = inputs::bounded(&reference.path, bound)?;
    ensure!(
        reference.object
            == ObjectId {
                sha256: hash_bytes(&bytes),
                size: bytes.len() as u64
            },
        "verified export input changed: {}",
        reference.path.display()
    );
    Ok(serde_json::from_slice(&bytes)?)
}
fn check_export(m: &netlist::Manifest) -> Result<Value> {
    let a: Value = checked_json(&m.export.assessment, 64 << 10)?;
    let s: Summary = checked_json(&m.export.summary, 32 << 20)?;
    let e = &m.expected;
    ensure!(
        a["passed"] == true
            && a["details"]["roundtrip_passed"] == true
            && a["export"] == serde_json::to_value(&m.export.summary.object)?
            && a["details"]["fasm"] == serde_json::to_value(&m.export.fasm.object)?
            && a["details"]["rust_asserted_bits_sha256"] == e.non_ecc_bits_sha256
            && a["details"]["rust_asserted_bit_count"] == e.non_ecc_set_bits
            && s.schema == "md-sim-prjxray-fasm-export/1"
            && s.passed
            && s.part == e.part
            && s.configuration_sha256 == e.configuration_sha256
            && s.merged_names_sha256 == e.formatted_names_sha256
            && s.merged_name_count == e.formatted_features
            && s.unknown_ecc_bits == e.unknown_ecc_bits
            && s.unknown_non_ecc.is_empty()
            && s.roundtrip.passed
            && s.roundtrip.error.is_none()
            && s.roundtrip.original_set_bits == e.non_ecc_set_bits
            && s.roundtrip.regenerated_set_bits == e.non_ecc_set_bits
            && s.roundtrip.original_bits_sha256 == e.non_ecc_bits_sha256
            && s.roundtrip.regenerated_bits_sha256 == e.non_ecc_bits_sha256
            && s.roundtrip.missing_count == 0
            && s.roundtrip.extra_count == 0,
        "selected input has no matching complete export round-trip evidence"
    );
    let comparison: Value = checked_json(&m.export.comparison_inputs, 64 << 10)?;
    ensure!(
        comparison["tool_manifest"] == serde_json::to_value(&m.fasm_tools.object)?,
        "export used different parser or database inputs"
    );
    Ok(a["details"]["unresolved_predicate_segments"].clone())
}
fn check_output(
    output: &Output,
    m: &netlist::Manifest,
    manifest_value: &Value,
    tool: &tool::Manifest,
) -> Result<()> {
    ensure!(
        output.schema == "md-sim-fasm-netlist-conversion/1"
            && output.tool_adaptations == serde_json::to_value(&tool.adaptations)?
            && output.converter_adaptation == manifest_value["adaptation"]
            && output.parser_adaptation == m.parser_adaptation
            && output.scope == m.scope,
        "converter output identity differs"
    );
    ensure!(
        output.converted && output.error.is_none() && output.graph_error.is_none(),
        "converter failed: {:?}; graph: {:?}",
        output.error,
        output.graph_error
    );
    let parser = output
        .parser
        .as_ref()
        .context("converter did not parse FASM")?;
    let fixture = output
        .parser_fixture
        .as_ref()
        .context("parser fixture absent")?;
    ensure!(
        output.parser_calls == 1
            && output.parser_exhausted
            && output.parsed_names_count == Some(m.expected.formatted_features)
            && output.parsed_names_sha256.as_ref() == Some(&m.expected.formatted_names_sha256)
            && parser.chunks == 19
            && parser.lines == 115981
            && parser.maximum_chunk_bytes <= 256 << 10
            && parser.chunk_limit_bytes == 256 << 10
            && parser.consumed_sha256 == m.export.fasm.object.sha256
            && parser.consumed_bytes == m.export.fasm.object.size
            && fixture.chunks == 3
            && fixture.lines == 5
            && fixture.maximum_chunk_bytes <= 64
            && fixture.chunk_limit_bytes == 64
            && fixture.consumed_bytes == 155
            && fixture.consumed_sha256
                == "9b62f598b602a2d077fc7e70f05293cfad52a3abe805fa34ce927e993830c963",
        "converter FASM consumption differs"
    );
    let expected_args = [
        "fasm2bels",
        "--db_root",
        "/db",
        "--part",
        &m.expected.part,
        "--connection_database",
        "/tmp/connections.db",
        "--fasm_file",
        "/selected.fasm",
        "--top",
        "top",
        "--verilog_file",
        "/tmp/selected.v",
        "--xdc_file",
        "/tmp/selected.xdc",
    ];
    ensure!(
        output.converter_argv.as_deref() == Some(expected_args.map(String::from).as_slice()),
        "converter options differ; no sink or clock relaxations are allowed"
    );
    let graph = output.graph.as_ref().context("connection graph absent")?;
    ObjectId {
        sha256: graph.sha256.clone(),
        size: graph.size,
    }
    .validate()?;
    ensure!(
        graph.size > 0
            && graph.size <= 1 << 30
            && graph.quick_check == ["ok"]
            && graph.counts.len() == 9
            && graph.counts.get("phy_tile") == Some(&13440)
            && graph.counts.get("tile_type") == Some(&90)
            && graph
                .counts
                .get("wire")
                .is_some_and(|count| *count > 0 && *count <= 3607758)
            && graph.counts.get("node").is_some_and(|count| *count > 0)
            && graph.unassigned_wires == 0
            && graph.missing_node_references == 0
            && graph.missing_tile_references == 0,
        "connection graph consistency checks failed"
    );
    ensure!(
        output.files.len() == 2
            && output.files.contains_key("selected.v")
            && output.files.contains_key("selected.xdc"),
        "converter output files missing or unexpected"
    );
    for artifact in output.files.values() {
        ensure!(
            artifact.object.size > 0
                && artifact.object.size <= 32 << 20
                && artifact.object.size == artifact.text.len() as u64
                && artifact.object.sha256 == hash_bytes(artifact.text.as_bytes()),
            "converter output content hash differs"
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "__inside") {
        ensure!(args.len() == 2, "invalid netlist guard arguments");
        confinement::require_netlist_preparation()?;
        return Err(Command::new(python::PYTHON)
            .args(["-I", "-S", "-B", "/convert.py"])
            .exec()
            .into());
    }
    ensure!(
        args.len() == 3,
        "usage: s9_netlist_reconstruct NETLIST_INPUTS NEW_OUTPUT"
    );
    let bytes = inputs::bounded(Path::new(&args[1]), 1 << 20)?;
    let identity = ObjectId {
        sha256: hash_bytes(&bytes),
        size: bytes.len() as u64,
    };
    let m = netlist::parse(&bytes)?;
    let manifest_value: Value = serde_json::from_slice(&bytes)?;
    let unresolved = check_export(&m)?;
    let (tool, tool_id) = netlist::read_fasm_tools(&m)?;
    let out = PathBuf::from(&args[2]);
    fs::create_dir(&out)?;
    let store = Store::open_with_limits(
        &out.join("inputs"),
        StoreLimits {
            max_object_bytes: 32 << 20,
            max_total_bytes: 256 << 20,
            max_objects: 2048,
        },
    )?;
    // Exact converter imports sqlite3 and subprocess; selected database imports csv.
    let mut mounts = python::capture(&store, &["_csv", "_sqlite3", "_posixsubprocess", "select"])?;
    tool::capture(&store, &mut mounts, &m.fasm_tools.path, &tool_id, &tool)?;
    netlist::capture(&store, &mut mounts, &m)?;
    store.import(Path::new(&args[1]), &identity)?;
    mounts.push(Mount {
        object: identity.clone(),
        guest_path: "/netlist-inputs.json".into(),
    });
    for (name, guest, body) in [
        (
            "convert.py",
            "/convert.py",
            include_bytes!("support/fasm_netlist.py").as_slice(),
        ),
        (
            "prjxray_fasm_export.py",
            "/support/prjxray_fasm_export.py",
            include_bytes!("support/prjxray_fasm_export.py").as_slice(),
        ),
        (
            "prjxray_feature_oracle.py",
            "/support/prjxray_feature_oracle.py",
            include_bytes!("support/prjxray_feature_oracle.py").as_slice(),
        ),
    ] {
        let path = out.join(name);
        fs::write(&path, body)?;
        python::mount_file(&store, &mut mounts, &path, guest.into())?;
    }
    let spec = ProcessSpec {
        support_virtualization: None,
        schema_version: "2.0".into(),
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        argv: vec!["/guard".into(), "__inside".into()],
        inputs: mounts,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 900,
            cpu_seconds: 600,
            memory_bytes: 4 << 30,
            processes: 16,
            file_bytes: 1 << 30,
            trace_bytes: 128 << 20,
            tmp_bytes: 1 << 30,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("comparison-inputs.json"),
        &json!({"manifest":identity,
        "runner":hash_file(&std::env::current_exe()?)?,"unresolved_predicate_segments":unresolved}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    fs::write(out.join("conversion.json"), &stdout)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let compared = (|| -> Result<()> {
        ensure!(!result.trace_truncated, "netlist trace truncated");
        let output: Output =
            serde_json::from_slice(&stdout).context("converter output absent or invalid")?;
        // Retain complete or partial artifacts before checking the conversion result.
        for name in ["selected.v", "selected.xdc"] {
            if let Some(artifact) = output.files.get(name) {
                ensure!(
                    artifact.text.len() <= 32 << 20
                        && artifact.object.size == artifact.text.len() as u64
                        && artifact.object.sha256 == hash_bytes(artifact.text.as_bytes()),
                    "retained artifact identity differs"
                );
                fs::write(out.join(name), &artifact.text)?;
            }
        }
        check_output(&output, &m, &manifest_value, &tool)?;
        ensure!(
            result.status == "exited" && result.exit_code == Some(0),
            "isolated converter did not exit successfully"
        );
        Ok(())
    })();
    save_json(
        &out.join("assessment.json"),
        &json!({"converted":compared.is_ok(),
        "error":compared.as_ref().err().map(|error|format!("{error:#}")),
        "conversion":hash_file(&out.join("conversion.json"))?,"manifest":identity,
        "unresolved_predicate_segments":unresolved,"scope":m.scope,
        "hdl_validation":"pending independent parsing, primitive semantics and execution"}),
    )?;
    compared?;
    println!("Pinned converter produced Verilog/XDC. HDL validation, physical timing and firmware gates remain open.");
    Ok(())
}
