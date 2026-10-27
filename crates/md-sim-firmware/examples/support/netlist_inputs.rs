//! Exact selected converter inputs and separately recorded tool adaptations.
use super::{inputs, tool};
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::Mount,
    store::{ObjectId, Store},
};
use serde::Deserialize;
use std::{collections::BTreeSet, path::Path};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub converter_revision: String,
    pub fasm_tools: tool::Reference,
    pub inputs: Vec<tool::Input>,
    pub adaptation: Adaptation,
    pub export: Export,
    pub expected: Expected,
    pub parser_adaptation: String,
    pub scope: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adaptation {
    pub id: String,
    pub component: String,
    pub reason: String,
    pub source_revision: String,
    pub files: Vec<ChangeFile>,
    pub patch: tool::Reference,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeFile {
    pub parent: tool::Reference,
    pub result: tool::Reference,
    pub changes: Vec<Change>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub from: String,
    pub to: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub assessment: tool::Reference,
    pub summary: tool::Reference,
    pub comparison_inputs: tool::Reference,
    pub fasm: tool::Reference,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub part: String,
    pub configuration_sha256: String,
    pub non_ecc_bits_sha256: String,
    pub non_ecc_set_bits: u64,
    pub formatted_features: u64,
    pub formatted_names_sha256: String,
    pub unknown_ecc_bits: u64,
}

pub fn parse(bytes: &[u8]) -> Result<Manifest> {
    ensure!(
        bytes.len() == 17522
            && md_sim_firmware::prepare::hash_bytes(bytes)
                == "ceaff7522bb00b6914a8ee409405b15bc81b9ae80f82c740bf069e7eafb534fc",
        "netlist manifest differs from the complete reviewed execution identity"
    );
    let manifest: Manifest = serde_json::from_slice(bytes)?;
    validate(&manifest)?;
    Ok(manifest)
}
fn validate(m: &Manifest) -> Result<()> {
    ensure!(
        m.schema == "md-sim-fasm-netlist-inputs/1"
            && m.converter_revision == "bafbcd8727e0807d6a620d1cf5dd151111c4a322"
            && m.adaptation.source_revision == m.converter_revision,
        "netlist source identity differs"
    );
    ensure!(
        m.inputs.len() == 40
            && m.adaptation.files.len() == 2
            && m.adaptation.id == "fasm2bels-lazy-optional-outputs-v1"
            && !m.adaptation.component.is_empty()
            && !m.adaptation.reason.is_empty()
            && !m.parser_adaptation.is_empty()
            && !m.scope.is_empty(),
        "netlist adaptation contract differs"
    );
    let mut paths = BTreeSet::new();
    let mut size = 0;
    for input in &m.inputs {
        input.object.validate()?;
        ensure!(
            input.object.size <= 1 << 20
                && input.guest_path.starts_with("/converter/")
                && input.guest_path.len() < 512
                && input.guest_path[1..].split('/').all(|part| !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)))
                && paths.insert(&input.guest_path),
            "netlist unsafe or duplicate source path"
        );
        size += input.object.size;
    }
    ensure!(size <= 2 << 20, "netlist source bound");
    for file in &m.adaptation.files {
        ensure!(
            file.changes.len() <= 8
                && !file.changes.is_empty()
                && m.inputs
                    .iter()
                    .any(|input| input.path == file.result.path
                        && input.object == file.result.object),
            "netlist adapted source differs"
        );
        for change in &file.changes {
            ensure!(
                !change.from.is_empty() && change.from.len() < 1024 && change.to.len() < 1024,
                "netlist derivation change bound"
            );
        }
    }
    Ok(())
}

fn derive(original: &[u8], changes: &[Change]) -> Result<Vec<u8>> {
    ensure!(original.len() < 1 << 20, "netlist parent bound");
    let mut result = std::str::from_utf8(original)?.to_owned();
    for change in changes {
        ensure!(
            result.matches(&change.from).count() == 1,
            "netlist derivation point absent or ambiguous"
        );
        result = result.replacen(&change.from, &change.to, 1);
    }
    Ok(result.into_bytes())
}

fn mount(
    store: &Store,
    mounts: &mut Vec<Mount>,
    input: &tool::Reference,
    guest: String,
) -> Result<()> {
    store.import(&input.path, &input.object)?;
    mounts.push(Mount {
        object: input.object.clone(),
        guest_path: guest,
    });
    Ok(())
}

pub fn capture(store: &Store, mounts: &mut Vec<Mount>, m: &Manifest) -> Result<()> {
    validate(m)?;
    for input in &m.inputs {
        store.import(&input.path, &input.object)?;
        mounts.push(Mount {
            object: input.object.clone(),
            guest_path: input.guest_path.clone(),
        });
    }
    mount(
        store,
        mounts,
        &m.adaptation.patch,
        "/converter-provenance/lazy-imports.patch".into(),
    )?;
    for (i, file) in m.adaptation.files.iter().enumerate() {
        mount(
            store,
            mounts,
            &file.parent,
            format!("/converter-provenance/original-{i}.py"),
        )?;
        let original =
            store.read_chunk(&file.parent.object, 0, file.parent.object.size as usize)?;
        let result = store.read_chunk(&file.result.object, 0, file.result.object.size as usize)?;
        ensure!(
            derive(&original, &file.changes)? == result,
            "netlist source does not match the declared import relocation"
        );
    }
    for (reference, guest) in [
        (&m.export.assessment, "/export-assessment.json"),
        (&m.export.summary, "/export-summary.json"),
        (&m.export.comparison_inputs, "/export-comparison.json"),
        (&m.export.fasm, "/selected.fasm"),
    ] {
        mount(store, mounts, reference, guest.into())?;
    }
    Ok(())
}

pub fn read_fasm_tools(m: &Manifest) -> Result<(tool::Manifest, ObjectId)> {
    let bytes = inputs::bounded(Path::new(&m.fasm_tools.path), 1 << 20)?;
    let object = ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(&bytes),
        size: bytes.len() as u64,
    };
    ensure!(object == m.fasm_tools.object, "FASM tool profile changed");
    Ok((tool::parse(&bytes)?, object))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_relocation_requires_unique_points_and_preserves_other_bytes() {
        let changes = vec![
            Change {
                from: "import optional\n".into(),
                to: "".into(),
            },
            Change {
                from: "def use():\n".into(),
                to: "def use():\n    import optional\n".into(),
            },
        ];
        assert_eq!(
            derive(
                b"import optional\ndef use():\n    optional.call()\n",
                &changes
            )
            .unwrap(),
            b"def use():\n    import optional\n    optional.call()\n"
        );
        assert!(derive(b"def use():\n    optional.call()\n", &changes).is_err());
        assert!(derive(b"import optional\nimport optional\ndef use():\n", &changes).is_err());
    }
    #[test]
    fn changed_source_under_the_same_revision_is_rejected() {
        let original = include_bytes!("../../../../simulator/firmware/s9-netlist-tool-inputs.json");
        let m = parse(original).unwrap();
        let text = std::str::from_utf8(original).unwrap();
        let changed = text.replacen(&m.inputs[0].object.sha256, &"0".repeat(64), 1);
        assert_eq!(changed.len(), original.len());
        assert!(parse(changed.as_bytes()).is_err());
    }
}
