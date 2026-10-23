//! Pinned installed libslirp inputs for the isolated QEMU build.
//! Package identity does not claim a reproducible upstream source build.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::Mount,
    store::{hash_file, Store},
};
use serde_json::json;
use std::{fs, os::unix::fs::symlink, path::Path};

const FILES: &[(&str, &str, &str)] = &[
    (
        "usr/lib/libslirp.so.0",
        "libslirp.so.0",
        "91ebce3a6429ccb3bf5331fdb06360ad2229d28c59cf0d43ae48e251535b4068",
    ),
    (
        "usr/include/slirp/libslirp.h",
        "libslirp.h",
        "2c95e3ae9780c1fdc9f6558f07d6d85a218c7f1342a0a36e44d653da37e41832",
    ),
    (
        "usr/include/slirp/libslirp-version.h",
        "libslirp-version.h",
        "42466da661fde948bc9c870dc2be08adcf7bc5b7b13cb98e5b883a3a5d28839b",
    ),
    (
        "usr/lib/pkgconfig/slirp.pc",
        "slirp.pc",
        "668e90a6bef5b05e592e15268789774fd0534aa81ab8a7b09aa4f4a133e18d7e",
    ),
];

pub fn capture(store: &Store, inputs: &mut Vec<Mount>) -> Result<()> {
    for (source, name, expected) in FILES {
        let source = Path::new("/").join(source);
        let object = hash_file(&source)?;
        ensure!(
            object.sha256 == *expected,
            "installed libslirp input changed"
        );
        if *name == "libslirp.so.0" {
            for input in
                md_sim_firmware::lab::elf_closure(store, vec![(format!("/slirp/{name}"), source)])?
            {
                if let Some(existing) = inputs.iter().find(|v| v.guest_path == input.guest_path) {
                    ensure!(existing.object == input.object, "library closure conflict");
                } else {
                    inputs.push(input);
                }
            }
        } else {
            store.import(&source, &object)?;
            inputs.push(Mount {
                object,
                guest_path: format!("/slirp/{name}"),
            });
        }
    }
    Ok(())
}

/// Call only after confinement and toolchain extraction.
pub fn stage() -> Result<serde_json::Value> {
    let mut identities = serde_json::Map::new();
    for (destination, name, expected) in FILES {
        let source = Path::new("/slirp").join(name);
        let object = hash_file(&source)?;
        ensure!(
            object.sha256 == *expected,
            "captured libslirp input changed"
        );
        let destination = Path::new("/work/toolchain").join(destination);
        ensure!(
            !destination.exists(),
            "toolchain already has a libslirp input"
        );
        fs::create_dir_all(destination.parent().context("destination parent")?)?;
        fs::copy(source, &destination)?;
        ensure!(
            hash_file(&destination)? == object,
            "staged libslirp input changed"
        );
        identities.insert((*name).into(), serde_json::to_value(object)?);
    }
    symlink("libslirp.so.0", "/work/toolchain/usr/lib/libslirp.so")?;
    Ok(json!({
        "library":"libslirp","version":"4.9.5","installed_package":"libslirp-4.9.5-1",
        "files":identities,
        "origin":"Installed Arch package; upstream https://gitlab.freedesktop.org/slirp/libslirp",
        "source_build_reproduced":false,
        "scope":"Pinned library, headers and pkg-config input. Runtime must retain its own network isolation and explicit forwarding policy."
    }))
}
