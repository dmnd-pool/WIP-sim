//! Bounded source, database and wheel inputs for the existing FASM tools.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::Mount,
    store::{ObjectId, Store},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub prjxray_revision: String,
    pub database_revision: String,
    pub fasm_revision: String,
    pub inputs: Vec<Input>,
    pub adaptations: Vec<Adaptation>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub path: PathBuf,
    pub object: ObjectId,
    pub guest_path: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub path: PathBuf,
    pub object: ObjectId,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Adaptation {
    pub id: String,
    pub component: String,
    pub reason: String,
    pub parent: Reference,
    pub patch: Reference,
    pub result: Reference,
    pub failure_evidence: Reference,
}

fn corrected_grammar(original: &[u8]) -> Result<Vec<u8>> {
    let text = std::str::from_utf8(original)?;
    let old = r#"NonEscapeCharacters: /[^\"]/ ;"#;
    let corrected = r#"NonEscapeCharacters: /[^\\"]/ ;"#;
    ensure!(
        original.len() <= 64 << 10 && text.matches(old).count() == 1,
        "original FASM grammar correction point absent or ambiguous"
    );
    Ok(text.replacen(old, corrected, 1).into_bytes())
}

pub fn parse(bytes: &[u8]) -> Result<Manifest> {
    ensure!(
        bytes.len() == 75840
            && md_sim_firmware::prepare::hash_bytes(bytes)
                == "d97ec0ffa30bce77cc88655b13d599a6e5610cdb2cd770f503163b06edc8c8d7",
        "FASM tool manifest differs from the complete reviewed source and database identity"
    );
    let manifest = serde_json::from_slice(bytes)?;
    validate(&manifest)?;
    Ok(manifest)
}

fn validate(manifest: &Manifest) -> Result<()> {
    ensure!(
        manifest.schema == "md-sim-fasm-tool-inputs/2"
            && manifest.prjxray_revision == "8146f1f2adc5aa2de8fe41886e18a73b57082086"
            && manifest.database_revision == "8372b58fe62c67f8f5896d78cd1611d01621dfae"
            && manifest.fasm_revision == "ffafe821bae68637fe46e36bcfd2a01b97cdf6f2",
        "unsupported FASM source identities"
    );
    ensure!(
        !manifest.inputs.is_empty() && manifest.inputs.len() <= 512,
        "FASM input count bound"
    );
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for input in &manifest.inputs {
        input.object.validate()?;
        let path = &input.guest_path;
        ensure!(
            path.len() <= 1024
                && ["/db/", "/upstream/", "/parser/", "/wheels/"]
                    .iter()
                    .any(|prefix| path.starts_with(prefix)),
            "FASM guest path prefix"
        );
        ensure!(
            path[1..].split('/').all(|component| !component.is_empty()
                && !matches!(component, "." | "..")
                && component
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)))
                && names.insert(path),
            "unsafe or duplicate FASM guest path"
        );
        ensure!(input.object.size <= 32 << 20, "FASM input size bound");
        total += input.object.size;
    }
    ensure!(total <= 128 << 20, "FASM aggregate input bound");
    ensure!(
        manifest.adaptations.len() == 1,
        "FASM grammar adaptation count"
    );
    let adaptation = &manifest.adaptations[0];
    ensure!(
        adaptation.id == "fasm-textx-escaped-quotes-v1"
            && adaptation.component == "FASM textX grammar"
            && !adaptation.reason.is_empty()
            && adaptation.reason.len() <= 2048,
        "unsupported FASM grammar adaptation"
    );
    for input in [
        &adaptation.parent,
        &adaptation.patch,
        &adaptation.result,
        &adaptation.failure_evidence,
    ] {
        input.object.validate()?;
        ensure!(
            input.object.size > 0 && input.object.size <= 64 << 10,
            "FASM adaptation input bound"
        );
    }
    ensure!(
        manifest
            .inputs
            .iter()
            .any(|input| input.guest_path == "/parser/fasm/parser/fasm.tx"
                && input.path == adaptation.result.path
                && input.object == adaptation.result.object),
        "adapted grammar identity differs from tool input"
    );
    Ok(())
}

pub fn capture(
    store: &Store,
    mounts: &mut Vec<Mount>,
    path: &Path,
    identity: &ObjectId,
    manifest: &Manifest,
) -> Result<()> {
    validate(manifest)?;
    store.import(path, identity)?;
    mounts.push(Mount {
        object: identity.clone(),
        guest_path: "/tool-inputs.json".into(),
    });
    for input in &manifest.inputs {
        store.import(&input.path, &input.object)?;
        mounts.push(Mount {
            object: input.object.clone(),
            guest_path: input.guest_path.clone(),
        });
    }
    let adaptation = &manifest.adaptations[0];
    for (input, name) in [
        (&adaptation.parent, "original-fasm.tx"),
        (&adaptation.patch, "escaped-quotes.patch"),
        (&adaptation.failure_evidence, "original-parser-failure.log"),
    ] {
        store.import(&input.path, &input.object)?;
        mounts.push(Mount {
            object: input.object.clone(),
            guest_path: format!("/tool-provenance/{name}"),
        });
    }
    let parent = store.read_chunk(
        &adaptation.parent.object,
        0,
        adaptation.parent.object.size as usize,
    )?;
    let result = store.read_chunk(
        &adaptation.result.object,
        0,
        adaptation.result.object.size as usize,
    )?;
    ensure!(
        corrected_grammar(&parent)? == result,
        "adapted grammar is not the recorded one-byte correction"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        serde_json::from_slice(include_bytes!(
            "../../../../simulator/firmware/s9-fasm-tool-inputs-escaped-quotes.json"
        ))
        .unwrap()
    }
    #[test]
    fn grammar_correction_is_exact_and_rejects_missing_or_repeated_points() {
        let original = b"NonEscapeCharacters: /[^\\\"]/ ;";
        let expected = b"NonEscapeCharacters: /[^\\\\\"]/ ;";
        assert_eq!(corrected_grammar(original).unwrap(), expected);
        assert!(corrected_grammar(b"other rule").is_err());
        assert!(corrected_grammar(&[original.as_slice(), original.as_slice()].concat()).is_err());
    }
    #[test]
    fn changing_a_source_hash_under_the_same_revision_is_rejected() {
        let bytes = include_bytes!(
            "../../../../simulator/firmware/s9-fasm-tool-inputs-escaped-quotes.json"
        );
        parse(bytes).unwrap();
        let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let original = value["inputs"][0]["object"]["sha256"].as_str().unwrap();
        assert_ne!(original, "0".repeat(64));
        let changed = std::str::from_utf8(bytes)
            .unwrap()
            .replacen(original, &"0".repeat(64), 1);
        assert_eq!(changed.len(), bytes.len());
        assert!(parse(changed.as_bytes()).is_err());
    }
    #[test]
    fn tool_manifest_cannot_override_guard_or_escape_its_prefixes() {
        let mut m = manifest();
        validate(&m).unwrap();
        for bad in [
            "/guard",
            "/usr/bin/python3.14",
            "/upstream/../guard",
            "/upstream//a",
            "/upstream/./a",
            "/wheels/a\nb",
        ] {
            m.inputs[0].guest_path = bad.into();
            assert!(validate(&m).is_err(), "{bad}");
        }
        let mut m = manifest();
        m.database_revision = "unknown".into();
        assert!(validate(&m).is_err());
        let mut m = manifest();
        m.inputs[0].object.size = (32 << 20) + 1;
        assert!(validate(&m).is_err());
    }
}
