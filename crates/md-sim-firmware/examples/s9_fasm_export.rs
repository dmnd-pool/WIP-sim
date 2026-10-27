//! Export original FPGA frames with pinned tools and an explicit grammar correction.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::prepare::hash_bytes;
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store, StoreLimits},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

#[path = "support/fasm_comparison.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[allow(dead_code)]
#[path = "support/fpga_feature_inputs.rs"]
mod inputs;
#[path = "support/python_tool_inputs.rs"]
mod python;
#[path = "support/fasm_tool_inputs.rs"]
mod tool;

const FRAME_MAP: &str = "work/simulator/qemu-s9/frames-selected-291/header.json";
const FRAME_HASH: &str = "a5e5a889a42bc929ff5c67cce88d5d60d2ef05c5682a6f1339cf1ec47128acbe";

#[derive(Deserialize)]
struct Export {
    schema: String,
    passed: bool,
    part: String,
    frame_count: u64,
    configuration_sha256: String,
    raw_names: Vec<String>,
    removed_default_names: Vec<String>,
    merged_names_sha256: String,
    merged_name_count: u64,
    unknown_ecc_bits: u64,
    unknown_non_ecc: Vec<Value>,
    annotations: Vec<Value>,
    decode_warnings: Vec<String>,
    fasm: String,
    fasm_sha256: String,
    roundtrip: RoundTrip,
    parser_fixture: ParserStats,
    parser_mode: String,
    harness_adaptation: String,
    tool_adaptations: Value,
    scope: String,
}
#[derive(Deserialize, Serialize)]
struct ParserStats {
    chunks: u64,
    lines: u64,
    maximum_chunk_bytes: u64,
    chunk_limit_bytes: u64,
    consumed_sha256: String,
    consumed_bytes: u64,
}
#[derive(Deserialize)]
struct RoundTrip {
    passed: bool,
    error: Option<String>,
    error_trace: Option<Vec<Value>>,
    original_set_bits: Option<u64>,
    regenerated_set_bits: Option<u64>,
    original_bits_sha256: Option<String>,
    regenerated_bits_sha256: Option<String>,
    missing_count: Option<u64>,
    extra_count: Option<u64>,
    parsed_names_sha256: Option<String>,
    parsed_name_count: Option<u64>,
    parser: Option<ParserStats>,
}

fn assert_input(tool: &tool::Manifest, guest: &str, object: &ObjectId) -> Result<()> {
    ensure!(
        tool.inputs
            .iter()
            .any(|input| input.guest_path == guest && input.object == *object),
        "tool database input differs: {guest}"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        ensure!(args.len() == 2, "invalid FASM guard arguments");
        confinement::require(true)?;
        return Err(Command::new(python::PYTHON)
            .args(["-I", "-S", "-B", "/export.py"])
            .exec()
            .into());
    }
    ensure!(
        args.len() == 5,
        "usage: s9_fasm_export FEATURE_INPUTS TOOL_INPUTS FEATURE_REPORT NEW_OUTPUT"
    );
    let (manifest, manifest_id) = inputs::read(Path::new(&args[1]))?;
    let tool_bytes = inputs::bounded(Path::new(&args[2]), 1 << 20)?;
    let tool_id = ObjectId {
        sha256: hash_bytes(&tool_bytes),
        size: tool_bytes.len() as u64,
    };
    let tool = tool::parse(&tool_bytes)?;
    assert_input(
        &tool,
        "/db/xc7z010clg400-1/part.json",
        &manifest.part.object,
    )?;
    assert_input(
        &tool,
        "/db/xc7z010/tilegrid.json",
        &manifest.tilegrid.object,
    )?;
    for table in &manifest.tables {
        ensure!(
            matches!(table.bus.as_str(), "CLB_IO_CLK" | "BLOCK_RAM"),
            "FASM tool bus unsupported"
        );
        let guest = format!(
            "/db/segbits_{}{}.db",
            table.tile_type.to_ascii_lowercase(),
            if table.bus == "BLOCK_RAM" {
                ".block_ram"
            } else {
                ""
            }
        );
        assert_input(&tool, &guest, &table.input.object)?;
    }
    let bytes = inputs::bounded(Path::new(&args[3]), 64 << 20)?;
    let report_id = ObjectId {
        sha256: hash_bytes(&bytes),
        size: bytes.len() as u64,
    };
    let report: comparison::Probe = serde_json::from_slice(&bytes)?;
    drop(bytes);
    ensure!(
        report.schema == "md-sim-fpga-feature-inspection/1"
            && report.passed
            && report.error.is_none()
            && report.input_manifest == manifest_id
            && report.decoding.unexplained.is_empty(),
        "feature report input or status differs"
    );
    report.inspector.validate()?;
    ensure!(
        report.decoding.decoded.asserted_bit_count
            == report.decoding.configuration_set_bits_excluding_ecc,
        "Rust asserted-bit union differs from original set-bit count"
    );
    let out = PathBuf::from(&args[4]);
    fs::create_dir(&out)?;
    let store = Store::open_with_limits(
        &out.join("inputs"),
        StoreLimits {
            max_object_bytes: 32 << 20,
            max_total_bytes: 256 << 20,
            max_objects: 2048,
        },
    )?;
    // Pinned prjxray.db -> tile -> lib imports csv at module scope.
    let mut mounts = python::capture(&store, &["_csv"])?;
    inputs::capture(
        &store,
        &mut mounts,
        Path::new(&args[1]),
        &manifest,
        &manifest_id,
    )?;
    tool::capture(&store, &mut mounts, Path::new(&args[2]), &tool_id, &tool)?;
    let frame_id = ObjectId {
        sha256: FRAME_HASH.into(),
        size: 649924,
    };
    store.import(Path::new(FRAME_MAP), &frame_id)?;
    mounts.push(Mount {
        object: frame_id.clone(),
        guest_path: "/frame-map.json".into(),
    });
    for (name, guest, bytes) in [
        (
            "export.py",
            "/export.py",
            include_bytes!("support/prjxray_fasm_export.py").as_slice(),
        ),
        (
            "prjxray_feature_oracle.py",
            "/support/prjxray_feature_oracle.py",
            include_bytes!("support/prjxray_feature_oracle.py").as_slice(),
        ),
    ] {
        let path = out.join(name);
        fs::write(&path, bytes)?;
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
            wall_seconds: 180,
            cpu_seconds: 120,
            memory_bytes: 1 << 30,
            processes: 16,
            file_bytes: 32 << 20,
            trace_bytes: 64 << 20,
            tmp_bytes: 64 << 20,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("comparison-inputs.json"),
        &json!({"feature_manifest":manifest_id,"tool_manifest":tool_id,
        "feature_report":report_id,"frame_map":frame_id,"runner":hash_file(&std::env::current_exe()?)?,
        "unresolved_predicate_segments":report.decoding.unresolved}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    fs::write(out.join("export.json"), &stdout)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let mut details = Value::Null;
    let compared = (|| -> Result<()> {
        ensure!(!result.trace_truncated, "FASM output truncated");
        let export: Export =
            serde_json::from_slice(&stdout).context("isolated FASM output absent or invalid")?;
        ensure!(
            export.schema == "md-sim-prjxray-fasm-export/1"
                && export.part == "xc7z010clg400-1"
                && export.frame_count == 5144
                && !export.scope.is_empty()
                && export.parser_mode == "bounded-complete-formatter-lines-v1"
                && !export.harness_adaptation.is_empty()
                && export.tool_adaptations == serde_json::to_value(&tool.adaptations)?
                && export.configuration_sha256 == report.decoding.configuration_sha256,
            "FASM output identity"
        );
        ensure!(
            export.fasm.len() <= 32 << 20
                && hash_bytes(export.fasm.as_bytes()) == export.fasm_sha256,
            "FASM text hash differs"
        );
        fs::write(out.join("selected.fasm"), &export.fasm)?;
        ensure!(
            export.parser_fixture.chunks > 1
                && export.parser_fixture.lines == 5
                && export.parser_fixture.consumed_bytes == 155
                && export.parser_fixture.maximum_chunk_bytes <= 64
                && export.parser_fixture.chunk_limit_bytes == 64
                && export.parser_fixture.consumed_sha256
                    == "9b62f598b602a2d077fc7e70f05293cfad52a3abe805fa34ce927e993830c963",
            "bounded parser fixture differs"
        );
        let comparison = comparison::compare(
            &report.decoding.decoded.names,
            &export.raw_names,
            &export.removed_default_names,
            &export.merged_names_sha256,
            export.merged_name_count,
        );
        let round = &export.roundtrip;
        details = json!({"configuration_sha256":export.configuration_sha256,"fasm":hash_file(&out.join("selected.fasm"))?,
            "raw_features":export.raw_names.len(),"formatted_features":export.merged_name_count,
            "removed_zero_defaults":export.removed_default_names.len(),"unknown_ecc_bits":export.unknown_ecc_bits,
            "unknown_non_ecc":export.unknown_non_ecc,"annotations":export.annotations,"decode_warnings":export.decode_warnings,
            "roundtrip_passed":round.passed,"roundtrip_error":round.error,"roundtrip_error_trace":round.error_trace,
            "original_set_bits":round.original_set_bits,
            "regenerated_set_bits":round.regenerated_set_bits,"original_bits_sha256":round.original_bits_sha256,
            "regenerated_bits_sha256":round.regenerated_bits_sha256,
            "rust_asserted_bits_sha256":report.decoding.decoded.asserted_bits_sha256,
            "rust_asserted_bit_count":report.decoding.decoded.asserted_bit_count,"unresolved_predicate_segments":report.decoding.unresolved,
            "parser_mode":export.parser_mode,"harness_adaptation":export.harness_adaptation,
            "tool_adaptations":export.tool_adaptations,
            "parser_fixture":export.parser_fixture,"parser":round.parser,
            "predicate_comparison_error":comparison.as_ref().err().map(|e|format!("{e:#}"))});
        comparison?;
        ensure!(
            round.passed
                && round.error.is_none()
                && round.missing_count == Some(0)
                && round.extra_count == Some(0)
                && round.original_set_bits == round.regenerated_set_bits
                && round.original_set_bits
                    == Some(report.decoding.configuration_set_bits_excluding_ecc)
                && round.original_bits_sha256 == round.regenerated_bits_sha256
                && round.original_bits_sha256.as_ref()
                    == Some(&report.decoding.decoded.asserted_bits_sha256)
                && round.parsed_names_sha256.as_ref() == Some(&export.merged_names_sha256)
                && round.parsed_name_count == Some(export.merged_name_count)
                && round.parser.as_ref().is_some_and(|parser| parser.chunks > 0
                    && parser.lines > 0
                    && parser.maximum_chunk_bytes <= 256 << 10
                    && parser.chunk_limit_bytes == 256 << 10
                    && parser.consumed_sha256 == export.fasm_sha256
                    && parser.consumed_bytes == export.fasm.len() as u64),
            "FASM round trip failed"
        );
        ensure!(
            result.status == "exited"
                && result.exit_code == Some(0)
                && export.passed
                && export.unknown_non_ecc.is_empty(),
            "isolated FASM export failed"
        );
        Ok(())
    })();
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":compared.is_ok(),"error":compared.as_ref().err().map(|e|format!("{e:#}")),
        "feature_report":report_id,"export":hash_file(&out.join("export.json"))?,"details":details,
        "scope":"Static FASM export, predicate comparison and non-ECC bit round trip only. Missing tables remain explicit. No netlist, physical timing or firmware execution gate."}),
    )?;
    compared?;
    println!("Pinned Project X-Ray export with recorded FASM grammar adaptation and configuration round trip passed. No netlist or firmware execution claimed.");
    Ok(())
}
