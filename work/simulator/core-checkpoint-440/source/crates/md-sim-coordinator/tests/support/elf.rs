use md_sim_runtime::{
    isolation::Mount,
    store::{hash_file, Store},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// Capture a trusted installed or locally built test executable. Never call
/// ldd on downloaded firmware or other acquired executable files.
pub fn inputs(store: &Store, executable: &Path, guest_path: &str) -> Vec<Mount> {
    let output = std::process::Command::new("/usr/bin/ldd")
        .arg(executable)
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut files = BTreeMap::from([(guest_path.to_string(), executable.to_path_buf())]);
    for field in String::from_utf8(output.stdout).unwrap().split_whitespace() {
        if field.starts_with('/') {
            files.insert(field.to_string(), PathBuf::from(field));
        }
    }
    files
        .into_iter()
        .map(|(guest_path, source)| {
            let object = hash_file(&source).unwrap();
            store.import(&source, &object).unwrap();
            Mount { object, guest_path }
        })
        .collect()
}
