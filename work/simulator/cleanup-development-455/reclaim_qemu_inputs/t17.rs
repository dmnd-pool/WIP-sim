//! Finished T17 oracle evidence. This authorizes duplicate CAS reclamation,
//! not firmware compatibility, a successful oracle, or hardware acceptance.
use super::{component_bytes, real_directories};
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{ProcessResult, ProcessSpec},
    store::ObjectId,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};

const FILE_BYTES: u64 = 1 << 20;
const ARCHIVE_BYTES: u64 = 4 << 20;
const MEMBERS: &[&str] = &[
    "assessment.json",
    "oracle",
    "compiler-version.stdout",
    "compiler-version.stderr",
    "linker-version.stdout",
    "linker-version.stderr",
    "emulator-version.stdout",
    "emulator-version.stderr",
    "compile.stdout",
    "compile.stderr",
    "compile-arguments.json",
    "execution.stdout",
    "execution.stderr",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(super) enum Mode {
    Packing,
    Readback,
}

pub(super) fn selected<'a>(path: &'a Path, repository: &Path) -> Option<(&'a Path, Mode)> {
    let run = path.parent()?;
    if path.file_name()? != "inputs" || run.parent()? != repository.join("work/simulator/bitaxe") {
        return None;
    }
    let name = run.file_name()?.to_str()?;
    for (prefix, mode) in [
        ("t17-core-oracle-", Mode::Packing),
        ("t17-readback-oracle-", Mode::Readback),
    ] {
        if name.strip_prefix(prefix).is_some_and(super::checkpoint) {
            return Some((run, mode));
        }
    }
    None
}

pub(super) fn directories(path: &Path, repository: &Path) -> Result<()> {
    ensure!(
        selected(path, repository).is_some(),
        "oracle cleanup path is outside its exact scope"
    );
    real_directories(path, repository)?;
    real_directories(&path.join("objects"), repository)
}

pub(super) fn canonical(repository: &Path) -> PathBuf {
    repository.join("work/simulator/bitaxe/t17-core-oracle-274/inputs")
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub(super) struct Evidence {
    pub run: PathBuf,
    mode: Mode,
    process_spec: ObjectId,
    process_result: ObjectId,
    evidence_tar: ObjectId,
    stderr: ObjectId,
    assessment: ObjectId,
    terminal_status: String,
    oracle_passed: bool,
    // Old packing272 has no derived-output fields. Exact tar/result binding
    // still identifies its files; do not invent those fields in its assessment.
    legacy_packing_output_fields_absent: bool,
    members: BTreeMap<String, ObjectId>,
    source_inputs: BTreeMap<String, ObjectId>,
}

fn identity(value: &Value) -> Result<ObjectId> {
    let object: ObjectId = serde_json::from_value(value.clone())?;
    object.validate()?;
    Ok(object)
}

fn captured_sources(
    run: &Path,
    spec: &ProcessSpec,
    report: &Value,
    mode: Mode,
) -> Result<BTreeMap<String, ObjectId>> {
    let mut expected = vec![
        ("/miner", "vendor_miner", None),
        ("/oracle.c", "wrapper", Some("oracle.c")),
    ];
    if mode == Mode::Readback {
        expected.extend([
            ("/oracle.ld", "link_script", Some("oracle.ld")),
            ("/oracle-abi.h", "abi_header", Some("oracle-abi.h")),
        ]);
    } else {
        ensure!(
            !spec
                .inputs
                .iter()
                .any(|m| matches!(m.guest_path.as_str(), "/oracle.ld" | "/oracle-abi.h")),
            "packing oracle has readback-only inputs"
        );
    }
    let mut result = BTreeMap::new();
    for (guest, field, adjacent) in expected {
        let mount = spec
            .inputs
            .iter()
            .find(|m| m.guest_path == guest)
            .with_context(|| format!("oracle input absent: {guest}"))?;
        ensure!(
            identity(&report[field])? == mount.object && mount.object.size <= FILE_BYTES,
            "oracle assessment input differs: {guest}"
        );
        let (_, actual) = component_bytes(
            &run.join("inputs/objects").join(&mount.object.sha256),
            FILE_BYTES,
        )?;
        ensure!(
            actual == mount.object,
            "oracle captured input bytes differ: {guest}"
        );
        if let Some(name) = adjacent {
            let (_, adjacent) = component_bytes(&run.join(name), FILE_BYTES)?;
            ensure!(adjacent == actual, "oracle source copy differs: {name}");
        }
        result.insert(guest.into(), actual);
    }
    Ok(result)
}

fn tar_members(run: &Path, bytes: &[u8]) -> Result<BTreeMap<String, ObjectId>> {
    ensure!(
        !bytes.is_empty() && bytes.len() % 512 == 0,
        "oracle tar framing differs"
    );
    let mut archive = tar::Archive::new(std::io::Cursor::new(bytes));
    let mut members = BTreeMap::new();
    let mut total = 0u64;
    for item in archive.entries()? {
        ensure!(
            members.len() < MEMBERS.len(),
            "oracle tar member count exceeds bound"
        );
        let mut item = item?;
        let path = item.path_bytes();
        let name = std::str::from_utf8(&path)?.to_owned();
        ensure!(
            MEMBERS.contains(&name.as_str()) && !members.contains_key(&name),
            "unknown or duplicate oracle tar member"
        );
        let kind = item.header().entry_type();
        ensure!(
            kind.is_file() || (name == "oracle" && kind.is_gnu_sparse()),
            "oracle tar member is not a regular file"
        );
        ensure!(
            item.size() <= FILE_BYTES,
            "oracle tar expanded member exceeds bound"
        );
        let mut decoded = Vec::new();
        (&mut item).take(FILE_BYTES + 1).read_to_end(&mut decoded)?;
        ensure!(
            decoded.len() as u64 == item.size(),
            "oracle tar member size differs"
        );
        total = total
            .checked_add(decoded.len() as u64)
            .context("oracle tar size overflow")?;
        ensure!(
            total <= ARCHIVE_BYTES,
            "oracle tar expanded size exceeds bound"
        );
        let (adjacent, object) = component_bytes(&run.join(&name), FILE_BYTES)?;
        ensure!(
            decoded == adjacent,
            "extracted oracle evidence differs: {name}"
        );
        members.insert(name, object);
    }
    // tar stops at its first zero header. Refuse another archive or nonzero
    // hidden trailer after that point; require the second terminating block.
    let position = usize::try_from(archive.into_inner().position())?;
    let trailing = bytes
        .get(position..)
        .context("oracle tar cursor outside input")?;
    ensure!(
        trailing.len() >= 512 && trailing.iter().all(|b| *b == 0),
        "oracle tar termination or trailing data differs"
    );
    ensure!(
        members.contains_key("assessment.json"),
        "oracle tar assessment absent"
    );
    Ok(members)
}

pub(super) fn finished(path: &Path, repository: &Path) -> Result<Option<Evidence>> {
    let Some((run, mode)) = selected(path, repository) else {
        return Ok(None);
    };
    directories(path, repository)?;
    let (spec_bytes, process_spec) = component_bytes(&run.join("spec.json"), 65536)?;
    let spec: ProcessSpec = serde_json::from_slice(&spec_bytes)?;
    spec.validate()?;
    let argv = if mode == Mode::Packing {
        vec!["/oracle-check", "__inside"]
    } else {
        vec!["/oracle-check", "__inside", "readback"]
    };
    ensure!(
        spec.argv == argv
            && spec.environment.is_empty()
            && !spec.loopback
            && spec.private_channel.is_none()
            && spec.dashboard.is_none()
            && spec.stdin_object.is_none()
            && spec.stdin_stream.is_none(),
        "oracle declared execution mode differs"
    );
    for guest in ["/oracle-check", "/tools/clang", "/tools/qemu-arm"] {
        ensure!(
            spec.inputs.iter().any(|m| m.guest_path == guest),
            "oracle executable input absent: {guest}"
        );
    }
    let (result_bytes, process_result) =
        component_bytes(&run.join("result.json"), (ARCHIVE_BYTES * 2) + (1 << 20))?;
    let result: ProcessResult = serde_json::from_slice(&result_bytes)?;
    ensure!(
        matches!(
            result.status.as_str(),
            "exited"
                | "cancelled"
                | "timed-out"
                | "cpu-limited"
                | "memory-limited"
                | "channel-failed"
                | "input-failed"
                | "dashboard-failed"
        ) && !result.trace_truncated,
        "oracle result is not a complete terminal record"
    );
    let (archive, evidence_tar) = component_bytes(&run.join("evidence.tar"), ARCHIVE_BYTES)?;
    let (stderr_bytes, stderr) = component_bytes(&run.join("stderr.log"), FILE_BYTES)?;
    ensure!(
        hex::decode(&result.stdout)? == archive && hex::decode(&result.stderr)? == stderr_bytes,
        "oracle traces differ from process result"
    );
    let members = tar_members(run, &archive)?;
    let (report_bytes, assessment) = component_bytes(&run.join("assessment.json"), FILE_BYTES)?;
    ensure!(
        members["assessment.json"] == assessment,
        "oracle assessment changed after tar validation"
    );
    let report: Value = serde_json::from_slice(&report_bytes)?;
    let schema = if mode == Mode::Packing {
        "md-sim-vendor-core-packing-oracle/1"
    } else {
        "md-sim-vendor-core-readback-oracle/1"
    };
    ensure!(report["schema"] == schema, "oracle assessment mode differs");
    let passed = report["passed"]
        .as_bool()
        .context("oracle result flag absent")?;
    if passed {
        ensure!(
            result.status == "exited"
                && result.exit_code == Some(0)
                && report["error"].is_null()
                && members.contains_key("oracle")
                && members.contains_key("execution.stdout"),
            "oracle success contradicts its terminal evidence"
        );
    } else {
        ensure!(
            report["error"].as_str().is_some_and(|s| !s.is_empty()),
            "failed oracle lacks its error"
        );
    }
    let legacy = mode == Mode::Packing
        && report.get("oracle_executable").is_none()
        && report.get("output_words").is_none();
    ensure!(
        !legacy
            || run
                .file_name()
                .is_some_and(|name| name == "t17-core-oracle-272"),
        "legacy packing output omission is evidenced only for checkpoint272"
    );
    if !legacy {
        for (field, name) in [
            ("oracle_executable", "oracle"),
            ("output_words", "execution.stdout"),
        ] {
            let field = report
                .get(field)
                .with_context(|| format!("oracle assessment field absent: {field}"))?;
            match members.get(name) {
                Some(expected) => ensure!(
                    identity(field)? == *expected,
                    "oracle output identity differs: {name}"
                ),
                None => ensure!(field.is_null() && !passed, "oracle output absent: {name}"),
            }
        }
    }
    if mode == Mode::Readback {
        match members.get("compile-arguments.json") {
            Some(expected) => ensure!(
                identity(&report["compile_arguments"])? == *expected,
                "oracle compiler arguments differ"
            ),
            None => ensure!(
                !passed && report["compile_arguments"].is_null(),
                "readback compiler arguments absent"
            ),
        }
    } else {
        ensure!(
            !members.contains_key("compile-arguments.json"),
            "packing tar has readback-only evidence"
        );
    }
    let source_inputs = captured_sources(run, &spec, &report, mode)?;
    Ok(Some(Evidence {
        run: run.to_path_buf(),
        mode,
        process_spec,
        process_result,
        evidence_tar,
        stderr,
        assessment,
        terminal_status: result.status,
        oracle_passed: passed,
        legacy_packing_output_fields_absent: legacy,
        members,
        source_inputs,
    }))
}

#[cfg(test)]
#[path = "t17_tests.rs"]
mod tests;
