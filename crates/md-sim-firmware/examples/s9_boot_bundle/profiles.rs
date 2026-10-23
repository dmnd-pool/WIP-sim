//! Exact archive inputs for the S9 boot diagnostic. These records do not
//! establish controller, hashboard, or FPGA compatibility.
use anyhow::{bail, Result};
use md_sim_runtime::store::ObjectId;

pub struct Image {
    pub name: &'static str,
    pub artifact_id: &'static str,
    pub path: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    /// Separately inspected boot members. None permits inspection only.
    pub boot_members: Option<(&'static str, &'static str)>,
}

const IMAGES: &[Image] = &[
    Image {
        name: "sd2022",
        artifact_id: "braiins-feed-4366569e66ab54bc0d15",
        path: "artifacts/files/firmware/braiins-feed-4366569e66ab54bc0d15/braiins-os_am1-s9_sd_2022-04-29-0-5eb2c5fe-22.02.4-plus.img",
        sha256: "97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef",
        size: 112_197_632,
        boot_members: Some((
            "950d19c6e5bfd9d8107def130b19fc9a5f64d824b26c0c493d5fdfe8503edac4",
            "80565aa18fdbdb655dcc5210ad48d036052ac94ac946efece6e4b54b19d0f550",
        )),
    },
    Image {
        name: "sd2020",
        artifact_id: "braiins-feed-0f081f628a7021a9149e",
        path: "artifacts/files/firmware/braiins-feed-0f081f628a7021a9149e/braiins-os-plus_am1-s9_sd_2020-03-29-1-6b4a0f46.img",
        sha256: "a195af100de6200be308f016bf786d32c77df7e0094875ce882545849c49a574",
        size: 84_934_656,
        // Isolated sd2020-inspection-001; exact whole-image hash checked.
        boot_members: Some((
            "bd4a29928779078ec5d6e4876f65db25823ced39ffe19f53083121683790c18f",
            "e50ca9b2fb0f6fdba3eb9689e837e58f7611240ae2c1f61172af3fab82876e72",
        )),
    },
];

impl Image {
    pub fn object(&self) -> ObjectId {
        ObjectId {
            sha256: self.sha256.into(),
            size: self.size,
        }
    }
}

pub fn select(name: &str) -> Result<&'static Image> {
    if let Some(image) = IMAGES
        .iter()
        .find(|image| image.name == name || image.artifact_id == name)
    {
        return Ok(image);
    }
    bail!("unregistered exact SD image: {name}")
}
