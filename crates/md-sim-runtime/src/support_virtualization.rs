//! One explicit host capability for a trusted support VM, never a worker job.
use crate::store::ObjectId;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::CStr,
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const DEVICE: &str = "/dev/kvm";
const KVM_GET_API_VERSION: libc::c_ulong = 0xae00;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SupportVirtualization {
    Kvm,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupportVirtualizationReceipt {
    pub kind: SupportVirtualization,
    pub device: String,
    pub major: u32,
    pub minor: u32,
    pub api_version: i32,
    pub kernel: BTreeMap<String, String>,
    pub logical_processors: usize,
    pub cpu_signatures: Vec<BTreeMap<String, String>>,
    pub cpuinfo_identity: ObjectId,
    pub boot_id_sha256: String,
    pub observed_unix_nanoseconds: u128,
    /// Support launches probe this host anew. This receipt grants no reuse.
    pub cache_eligible: bool,
}

pub(crate) struct Grant {
    device: File,
    pub receipt: SupportVirtualizationReceipt,
}

fn bounded(path: &str, limit: u64) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "support host context exceeds bound"
    );
    Ok(bytes)
}

fn cpu_signatures(bytes: &[u8]) -> Result<(usize, Vec<BTreeMap<String, String>>)> {
    let text = std::str::from_utf8(bytes)?;
    let mut signatures = BTreeSet::new();
    let mut processors = 0usize;
    for section in text.split("\n\n") {
        let mut signature = BTreeMap::new();
        for line in section.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim();
            if key == "processor" {
                processors += 1;
            }
            if matches!(
                key,
                "vendor_id"
                    | "cpu family"
                    | "model"
                    | "model name"
                    | "stepping"
                    | "microcode"
                    | "flags"
                    | "bugs"
                    | "CPU implementer"
                    | "CPU architecture"
                    | "CPU variant"
                    | "CPU part"
                    | "CPU revision"
                    | "Features"
            ) {
                signature.insert(key.to_owned(), value.trim().to_owned());
            }
        }
        if !signature.is_empty() {
            signatures.insert(signature);
        }
    }
    ensure!(
        processors > 0 && processors <= 65536 && !signatures.is_empty(),
        "support host CPU context absent or outside bound"
    );
    Ok((processors, signatures.into_iter().collect()))
}

impl Grant {
    pub fn open() -> Result<Self> {
        // This fixed device is the only capability. There is no caller path,
        // host directory, inherited guest FD, or alternative device fallback.
        let device = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(DEVICE)
            .context("trusted support VM requires /dev/kvm read/write access")?;
        let metadata = device.metadata()?;
        let major = libc::major(metadata.rdev());
        let minor = libc::minor(metadata.rdev());
        ensure!(
            metadata.file_type().is_char_device() && major == 10 && minor == 232,
            "support virtualization device must be character 10:232"
        );
        let api_version =
            unsafe { libc::ioctl(device.as_raw_fd(), KVM_GET_API_VERSION, 0 as libc::c_ulong) };
        if api_version < 0 {
            return Err(std::io::Error::last_os_error()).context("KVM_GET_API_VERSION failed");
        }
        ensure!(
            api_version == 12,
            "KVM API version must be 12 (observed {api_version})"
        );
        let mut uname: libc::utsname = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { libc::uname(&mut uname) } == 0,
            "host kernel identity unavailable"
        );
        let kernel = [
            ("sysname", &uname.sysname),
            ("release", &uname.release),
            ("version", &uname.version),
            ("machine", &uname.machine),
        ]
        .into_iter()
        .map(|(key, value)| {
            (
                key.to_owned(),
                unsafe { CStr::from_ptr(value.as_ptr()) }
                    .to_string_lossy()
                    .into_owned(),
            )
        })
        .collect();
        let cpuinfo = bounded("/proc/cpuinfo", 4 << 20)?;
        let (logical_processors, cpu_signatures) = cpu_signatures(&cpuinfo)?;
        let boot = bounded("/proc/sys/kernel/random/boot_id", 128)?;
        ensure!(!boot.is_empty(), "host boot identity absent");
        let receipt = SupportVirtualizationReceipt {
            kind: SupportVirtualization::Kvm,
            device: DEVICE.into(),
            major,
            minor,
            api_version,
            kernel,
            logical_processors,
            cpu_signatures,
            cpuinfo_identity: ObjectId {
                sha256: hex::encode(Sha256::digest(&cpuinfo)),
                size: cpuinfo.len() as u64,
            },
            boot_id_sha256: hex::encode(Sha256::digest(&boot)),
            observed_unix_nanoseconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
            cache_eligible: false,
        };
        let grant = Self { device, receipt };
        grant.verify_path()?;
        Ok(grant)
    }

    pub fn verify_path(&self) -> Result<()> {
        let expected = self.device.metadata()?;
        let actual = fs::symlink_metadata(DEVICE)?;
        ensure!(
            actual.file_type().is_char_device()
                && actual.dev() == expected.dev()
                && actual.ino() == expected.ino()
                && actual.rdev() == expected.rdev(),
            "fixed KVM device changed during support launch"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_context_retains_distinct_cpu_features_without_clock_noise() {
        let input = b"processor : 0\nvendor_id : V\nflags : a b\ncpu MHz : 1\n\nprocessor : 1\nvendor_id : V\nflags : a b\ncpu MHz : 9\n\nprocessor : 2\nvendor_id : V\nflags : a c\n";
        let (count, signatures) = cpu_signatures(input).unwrap();
        assert_eq!(count, 3);
        assert_eq!(signatures.len(), 2);
        assert!(signatures
            .iter()
            .all(|value| !value.contains_key("cpu MHz")));
        assert!(cpu_signatures(b"flags: a b\n").is_err());
    }
}
