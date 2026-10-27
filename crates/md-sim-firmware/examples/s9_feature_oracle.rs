//! Compare every selected FPGA predicate with unchanged Project X-Ray code.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::{
    bitstream::series7::{
        features::{decode, FeatureDatabase},
        frames::Part,
        tilegrid::TileGrid,
    },
    prepare::hash_bytes,
};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store, StoreLimits},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

#[path = "support/fpga_feature_comparison.rs"]
mod comparison;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[allow(dead_code)]
#[path = "support/fpga_feature_inputs.rs"]
mod inputs;
#[path = "support/python_tool_inputs.rs"]
mod python;
#[path = "../tests/support/series7.rs"]
mod vectors;
use python::{mount_file, PYTHON};

const UPSTREAM: &str = "artifacts/files/source/source-prjxray-oracle-c9f02d";
const COMMIT: &str = "c9f02d8576042325425824647ab5555b1bc77833";
const FRAME_MAP: &str = "work/simulator/qemu-s9/frames-selected-291/header.json";
const FRAME_HASH: &str = "a5e5a889a42bc929ff5c67cce88d5d60d2ef05c5682a6f1339cf1ec47128acbe";
const SOURCES: &[(&str, u64, &str)] = &[
    (
        "__init__.py",
        0,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    ),
    (
        "tile_segbits.py",
        5362,
        "e7e43973d9e5f0c72c4f2fe23b0541a2b3be9441bfa473250294c8fec7c556db",
    ),
    (
        "tile_segbits_alias.py",
        4450,
        "0cbb861e32baaf11cc242241ee1f6388c604c9e2dd1074b159bb4101de6c4202",
    ),
    (
        "bitstream.py",
        3552,
        "c87a2dce80b5c4edc8b418fb1722d5621c6ae214430517190ca308e5498a6d0f",
    ),
    (
        "grid_types.py",
        943,
        "3476c18faeb0169b4b6216051086f0e7055975d42492c7cb6a082906409875ea",
    ),
    (
        "util.py",
        12609,
        "a5b3677e39d53a57f6d694c5acb77c0eff98fbfea102a6b83e5e9011423ac1a3",
    ),
    (
        "roi.py",
        1988,
        "e67b6beb88624d73e0d084130f4a1c606178bf7e16a2a4959460767c9f55b70b",
    ),
];

fn fixture_digest() -> Result<(String, String)> {
    let part_bytes = vectors::small_database();
    let part = Part::from_pinned_json(&part_bytes, &hash_bytes(&part_bytes))?;
    let grid_bytes = serde_json::to_vec(&json!({"TILE":{"type":"TYPE","grid_x":0,
        "grid_y":0,"sites":{},"bits":{"CLB_IO_CLK":{"baseaddr":"0x00000000",
        "frames":1,"offset":0,"words":2,"alias":{"type":"PARENT","start_offset":2,
        "sites":{"REAL_SITE":"ALIAS_SITE"}}}}}}))?;
    let grid = TileGrid::from_pinned_json(&grid_bytes, &hash_bytes(&grid_bytes), &part)?;
    let text = b"PARENT.ALIAS_SITE.POSITIVE 00_64 !00_65\nPARENT.ALIAS_SITE.DEFAULT !00_65 !00_66\nPARENT.ALIAS_SITE.OUTSIDE 00_64 !00_01\nPARENT.ALIAS_SITE.FALSE 00_65\n";
    let table_hash = hash_bytes(text);
    let database = FeatureDatabase::from_pinned_text(text, &table_hash, "PARENT", "CLB_IO_CLK")?;
    let decoded = decode(
        &vectors::stream(&vectors::payload(), 0, 0x6c39381b),
        &part,
        &grid,
        &[database],
    )?;
    ensure!(
        decoded.decoded.len() == 1
            && decoded.matched_features == 2
            && decoded.decoded[0].features_outside_alias_range == 1,
        "alias fixture result"
    );
    let mut matches: Vec<comparison::Matched> =
        serde_json::from_value(serde_json::to_value(&decoded.decoded[0].matches)?)?;
    Ok((comparison::digest_matches(&mut matches)?, table_hash))
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|value| value == "__inside") {
        ensure!(args.len() == 2, "invalid oracle guard arguments");
        confinement::require(true)?;
        return Err(Command::new(PYTHON)
            .args(["-I", "-S", "-B", "/oracle.py"])
            .exec()
            .into());
    }
    ensure!(
        args.len() == 4,
        "usage: s9_feature_oracle PINNED_INPUT_MANIFEST FEATURE_REPORT NEW_OUTPUT"
    );
    let manifest_path = Path::new(&args[1]);
    let (manifest, manifest_identity) = inputs::read(manifest_path)?;
    let bytes = inputs::bounded(Path::new(&args[2]), 64 << 20)?;
    let report_identity = ObjectId {
        sha256: hash_bytes(&bytes),
        size: bytes.len() as u64,
    };
    let mut report: comparison::ProbeReport = serde_json::from_slice(&bytes)?;
    drop(bytes);
    ensure!(
        report.schema == "md-sim-fpga-feature-inspection/1"
            && report.passed
            && report.error.is_none()
            && report.input_manifest == manifest_identity
            && !report.scope.is_empty(),
        "feature report or input identity differs"
    );
    report.inspector.validate()?;
    ensure!(
        report.decoding.other.remove("limit").is_some(),
        "feature report limit absent"
    );
    let expected = serde_json::to_value(&report.decoding)?;
    let out = PathBuf::from(&args[3]);
    fs::create_dir(&out)?;
    let store = Store::open_with_limits(
        &out.join("inputs"),
        StoreLimits {
            max_object_bytes: 32 << 20,
            max_total_bytes: 128 << 20,
            max_objects: 2048,
        },
    )?;
    let mut mounts = python::capture(&store, &[])?;
    inputs::capture(
        &store,
        &mut mounts,
        manifest_path,
        &manifest,
        &manifest_identity,
    )?;
    let frame_identity = ObjectId {
        sha256: FRAME_HASH.into(),
        size: 649924,
    };
    store.import(Path::new(FRAME_MAP), &frame_identity)?;
    mounts.push(Mount {
        object: frame_identity.clone(),
        guest_path: "/frame-map.json".into(),
    });
    for &(name, size, sha256) in SOURCES {
        let identity = ObjectId {
            sha256: sha256.into(),
            size,
        };
        store.import(&Path::new(UPSTREAM).join(name), &identity)?;
        mounts.push(Mount {
            object: identity,
            guest_path: format!("/upstream/prjxray/{name}"),
        });
    }
    let harness_path = out.join("oracle.py");
    fs::write(
        &harness_path,
        include_bytes!("support/prjxray_feature_oracle.py"),
    )?;
    mount_file(&store, &mut mounts, &harness_path, "/oracle.py".into())?;
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
            wall_seconds: 90,
            cpu_seconds: 60,
            memory_bytes: 512 << 20,
            processes: 16,
            file_bytes: 1 << 20,
            trace_bytes: 8 << 20,
            tmp_bytes: 1 << 20,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("comparison-inputs.json"),
        &json!({"upstream_commit":COMMIT,
        "feature_manifest":manifest_identity,"feature_report":report_identity,
        "feature_report_path":Path::new(&args[2]),"inspector":report.inspector,
        "frame_map":frame_identity,"runner":hash_file(&std::env::current_exe()?)?}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    fs::write(out.join("oracle.json"), &stdout)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let compared = (|| -> Result<()> {
        ensure!(
            result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
            "isolated upstream oracle failed; original output retained"
        );
        let oracle: Value = serde_json::from_slice(&stdout)?;
        ensure!(
            oracle["schema"] == "md-sim-prjxray-feature-oracle/1",
            "oracle schema"
        );
        // Equality includes every digest, segment identity, unresolved segment,
        // unexplained bit, table identity, and aggregate count.
        ensure!(
            oracle["decoding"] == expected,
            "upstream feature results differ"
        );
        let (digest, table_hash) = fixture_digest()?;
        let fixture = &oracle["alias_fixture"];
        ensure!(
            fixture["table_sha256"] == table_hash
                && fixture["outside_alias_rules"] == 1
                && fixture["cases"]
                    .as_array()
                    .context("alias cases absent")?
                    .len()
                    == 4
                && fixture["cases"][0]["matches_sha256"] == digest
                && fixture["cases"][0]["match_count"] == 2,
            "independent alias fixture differs"
        );
        Ok(())
    })();
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":compared.is_ok(),
        "error":compared.as_ref().err().map(|error|format!("{error:#}")),
        "decoded_segments":report.decoding.decoded.len(),"counts":report.decoding.other,
        "feature_report":report_identity,"oracle_output":hash_file(&out.join("oracle.json"))?,
        "scope":"Independent static feature predicates and alias fixture only. No complete netlist, firmware execution, physical timing or exact-board gate."}),
    )?;
    compared?;
    println!("Unchanged Project X-Ray predicates match the full selected feature report. No netlist or firmware execution claimed.");
    Ok(())
}
