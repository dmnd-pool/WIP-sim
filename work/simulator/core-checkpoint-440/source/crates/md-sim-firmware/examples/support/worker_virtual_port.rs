//! Open exactly one named private virtual port and verify its device identity.
use anyhow::{ensure, Context, Result};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    path::Path,
};

pub fn open(expected_name: &str) -> Result<File> {
    let mut selected = Vec::new();
    for (index, entry) in fs::read_dir("/sys/class/virtio-ports")?.enumerate() {
        ensure!(index < 16, "virtual channel count bound");
        let entry = entry?;
        if fs::read_to_string(entry.path().join("name"))?.trim() == expected_name {
            let name = entry.file_name();
            let name = name.to_str().context("virtual port name is not text")?;
            ensure!(
                name.starts_with("vport")
                    && name.len() <= 32
                    && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.'),
                "virtual port device name differs"
            );
            let device = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                .open(Path::new("/dev").join(name))?;
            let numbers = fs::read_to_string(entry.path().join("dev"))?;
            let (major, minor) = numbers.trim().split_once(':').context("device numbers")?;
            let expected = libc::makedev(major.parse()?, minor.parse()?);
            ensure!(
                device.metadata()?.file_type().is_char_device()
                    && device.metadata()?.rdev() == expected,
                "virtual device identity"
            );
            selected.push(device);
        }
    }
    ensure!(
        selected.len() == 1,
        "selected private virtual channel absent"
    );
    Ok(selected.remove(0))
}
