//! Preparation-component fixtures only. No fixture program or firmware boots.
use super::*;
use flate2::{read::MultiGzDecoder, write::GzEncoder, Compression};
use md_sim_runtime::store::Store;
use serde_json::json;
use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

const BASE_NAME: &str = "base-marker.txt";
const BROWSER_NAME: &str = "browser-marker.txt";
const BASE_BYTES: &[u8] = b"base archive only\n";
const BROWSER_BYTES: &[u8] = b"browser payload in the second gzip/newc archive\n";

fn object(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(bytes),
        size: bytes.len() as u64,
    }
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}

fn cpio(directory: &Path, arguments: &[&str], input: &[u8]) -> Vec<u8> {
    // Installed prerequisite, not an acquired archive program. The tiny input
    // is controlled here; --to-stdout extraction never writes archive paths.
    let mut child = Command::new("/usr/bin/cpio")
        .args(arguments)
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("installed /usr/bin/cpio is required for the newc fixture");
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "cpio fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.len() <= 8192 && output.stderr.len() <= 1024);
    output.stdout
}

fn archive(directory: &Path, name: &str, bytes: &[u8]) -> Vec<u8> {
    fs::create_dir(directory).unwrap();
    fs::write(directory.join(name), bytes).unwrap();
    cpio(
        directory,
        &["-o", "--format=newc", "--owner=0:0", "--quiet"],
        format!("{name}\n").as_bytes(),
    )
}

fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

struct Fixture {
    root: tempfile::TempDir,
    preparation: PathBuf,
    scenario: Value,
    access: Value,
    kernel: ObjectId,
    payload: ObjectId,
    mounts: Vec<Mount>,
    base: Vec<u8>,
    extra: Vec<u8>,
}

impl Fixture {
    fn new(browser: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let preparation = root.path().join("prepared");
        fs::create_dir(&preparation).unwrap();
        let base = archive(&root.path().join("base"), BASE_NAME, BASE_BYTES);
        let extra = archive(&root.path().join("payload"), BROWSER_NAME, BROWSER_BYTES);
        let base_gzip = gzip(&base);
        let combined = [base_gzip.as_slice(), gzip(&extra).as_slice()].concat();
        fs::write(preparation.join("initramfs.cpio.gz"), &base_gzip).unwrap();
        fs::write(preparation.join("combined-initramfs.cpio.gz"), &combined).unwrap();
        let original = root.path().join("original-payload.json");
        write_json(
            &original,
            &json!({"schema":"md-sim-vm-payload/1","files":{},"symlinks":{},"fixture":"selector marker archive"}),
        );
        let payload = hash_file(&original).unwrap();
        let kernel = object(b"kernel identity fixture; never executed");
        let driver = object(b"driver identity fixture; never executed");
        let mounts = if browser {
            vec![
                Mount {
                    guest_path: "/s9-browser-check".into(),
                    object: driver.clone(),
                },
                Mount {
                    guest_path: "/carrier-browser-payload.json".into(),
                    object: payload.clone(),
                },
            ]
        } else {
            Vec::new()
        };
        write_json(&preparation.join("root-manifest.json"), &json!(mounts));
        let selected = if browser { &combined } else { &base_gzip };
        write_json(
            &preparation.join("prepared-image.json"),
            &json!({
                "schema":"md-sim-private-vm-preparation/1", "image":object(selected), "kernel":kernel,
                "root_manifest":hash_file(&preparation.join("root-manifest.json")).unwrap()
            }),
        );
        let mut scenario = json!({"environment":{},"files":{}});
        if browser {
            scenario["environment"]["MD_SIM_CARRIER_BROWSER"] = json!("1");
            scenario["payload_manifest"] = json!(original);
            scenario["files"]["/carrier-browser-payload.json"] =
                json!({"path":original,"mode":292});
            fs::copy(&original, preparation.join("payload-manifest.json")).unwrap();
            write_json(
                &preparation.join("payload-report.json"),
                &json!({
                    "schema":"md-sim-vm-payload-result/1", "manifest":payload, "combined_initramfs":object(&combined)
                }),
            );
        }
        let access = json!({"browser_requested":browser,
            "browser_payload":browser.then_some(&payload), "browser_driver":browser.then_some(driver)});
        Self {
            root,
            preparation,
            scenario,
            access,
            kernel,
            payload,
            mounts,
            base,
            extra,
        }
    }

    fn select(&self, browser: bool) -> Result<Selected> {
        select(
            &self.preparation,
            &self.scenario,
            &self.kernel,
            browser.then_some(&self.payload),
        )
    }
}

#[test]
fn selected_store_bytes_include_the_second_archive_only_for_browser_request() {
    for browser in [false, true] {
        let fixture = Fixture::new(browser);
        let selected = fixture.select(browser).unwrap();
        assert_eq!(
            selected.path.file_name().unwrap(),
            if browser {
                "combined-initramfs.cpio.gz"
            } else {
                "initramfs.cpio.gz"
            }
        );
        let store = Store::open(&fixture.root.path().join("store"), 1 << 20).unwrap();
        store.import(&selected.path, &selected.object).unwrap();
        let imported = store
            .read_chunk(&selected.object, 0, selected.object.size as usize)
            .unwrap();
        assert_eq!(object(&imported), selected.object);
        let mut unpacked = Vec::new();
        MultiGzDecoder::new(imported.as_slice())
            .read_to_end(&mut unpacked)
            .unwrap();
        assert_eq!(&unpacked[..fixture.base.len()], fixture.base);
        let base_listing = cpio(
            fixture.root.path(),
            &["-it", "--quiet"],
            &unpacked[..fixture.base.len()],
        );
        assert_eq!(base_listing, format!("{BASE_NAME}\n").as_bytes());
        if browser {
            let second = &unpacked[fixture.base.len()..];
            assert_eq!(second, fixture.extra);
            assert_eq!(
                cpio(fixture.root.path(), &["-it", "--quiet"], second),
                format!("{BROWSER_NAME}\n").as_bytes()
            );
            assert_eq!(
                cpio(
                    fixture.root.path(),
                    &["-i", "--to-stdout", "--quiet", BROWSER_NAME],
                    second
                ),
                BROWSER_BYTES
            );
            assert_ne!(
                selected.object,
                hash_file(&fixture.preparation.join("initramfs.cpio.gz")).unwrap()
            );
        } else {
            assert_eq!(unpacked.len(), fixture.base.len());
        }
    }
}

#[test]
fn prepared_image_kernel_root_and_selected_bytes_must_agree() {
    let fixture = Fixture::new(true);
    let path = fixture.preparation.join("prepared-image.json");
    let original = read(&path).unwrap();
    for field in ["image", "kernel", "root_manifest"] {
        let mut changed = original.clone();
        changed[field] = json!(object(b"different input"));
        write_json(&path, &changed);
        assert!(fixture.select(true).is_err(), "changed {field}");
    }
    let mut changed = original.clone();
    changed["schema"] = json!("other/1");
    write_json(&path, &changed);
    assert!(fixture.select(true).is_err());
    write_json(&path, &original);
    assert!(select(
        &fixture.preparation,
        &fixture.scenario,
        &object(b"other kernel"),
        Some(&fixture.payload)
    )
    .is_err());
    let root = fixture.preparation.join("root-manifest.json");
    let saved = fs::read(&root).unwrap();
    fs::write(&root, b"[]\n").unwrap();
    assert!(fixture.select(true).is_err());
    fs::write(&root, saved).unwrap();
    fs::copy(
        fixture.preparation.join("initramfs.cpio.gz"),
        fixture.preparation.join("combined-initramfs.cpio.gz"),
    )
    .unwrap();
    assert!(
        fixture.select(true).is_err(),
        "base bytes cannot replace the selected combined image"
    );
}

#[test]
fn original_request_outer_flag_and_prepared_mode_cannot_be_substituted() {
    let fixture = Fixture::new(true);
    let enabled = BTreeMap::from([("MD_SIM_CARRIER_BROWSER_SUPPORT".into(), "1".into())]);
    assert_eq!(
        browser_request(&enabled, &fixture.access).unwrap(),
        Some(fixture.payload.clone())
    );
    assert_eq!(
        browser_request(&BTreeMap::new(), &Value::Null).unwrap(),
        None
    );
    assert!(browser_request(&enabled, &Value::Null).is_err());
    assert!(browser_request(&BTreeMap::new(), &fixture.access).is_err());
    for value in ["", "0", "true", "2"] {
        let bad = BTreeMap::from([("MD_SIM_CARRIER_BROWSER_SUPPORT".into(), value.into())]);
        assert!(browser_request(&bad, &fixture.access).is_err());
    }
    let mut bad = fixture.access.clone();
    bad["browser_requested"] = json!(false);
    assert!(browser_request(&enabled, &bad).is_err());
    assert!(browser_request(&BTreeMap::new(), &bad).is_err());
    bad["browser_requested"] = json!(true);
    bad["browser_payload"]["sha256"] = json!("not-a-hash");
    assert!(browser_request(&enabled, &bad).is_err());
    for value in [Value::Null, json!("0"), json!(1), json!(true)] {
        let mut scenario = fixture.scenario.clone();
        scenario["environment"]["MD_SIM_CARRIER_BROWSER"] = value;
        assert!(select(
            &fixture.preparation,
            &scenario,
            &fixture.kernel,
            Some(&fixture.payload)
        )
        .is_err());
    }
    assert!(fixture.select(false).is_err());
    let plain = Fixture::new(false);
    assert!(plain.select(true).is_err());
}

#[test]
fn retained_payload_is_portable_but_manifest_and_report_substitution_fails() {
    let fixture = Fixture::new(true);
    let original_path = Path::new(fixture.scenario["payload_manifest"].as_str().unwrap());
    fs::remove_file(original_path).unwrap();
    assert!(!original_path.exists());
    fixture
        .select(true)
        .expect("audit uses captured identities, not an unavailable source host path");
    let report_path = fixture.preparation.join("payload-report.json");
    let original_report = read(&report_path).unwrap();
    for field in ["manifest", "combined_initramfs", "schema"] {
        let mut changed = original_report.clone();
        changed[field] = if field == "schema" {
            json!("other/1")
        } else {
            json!(object(b"substituted report input"))
        };
        write_json(&report_path, &changed);
        assert!(fixture.select(true).is_err(), "changed report {field}");
    }
    write_json(&report_path, &original_report);
    for replacement in [Value::Null, json!(""), json!("/different/payload.json")] {
        let mut scenario = fixture.scenario.clone();
        scenario["payload_manifest"] = replacement;
        assert!(select(
            &fixture.preparation,
            &scenario,
            &fixture.kernel,
            Some(&fixture.payload)
        )
        .is_err());
    }
    let manifest = fixture.preparation.join("payload-manifest.json");
    let saved = fs::read(&manifest).unwrap();
    fs::write(&manifest, b"{}\n").unwrap();
    assert!(fixture.select(true).is_err());
    fs::write(&manifest, saved).unwrap();
    fs::remove_file(&report_path).unwrap();
    assert!(fixture.select(true).is_err());
}

#[test]
fn absent_browser_request_rejects_payload_and_captured_browser_inputs() {
    let mut fixture = Fixture::new(false);
    assert_eq!(
        browser_request(&BTreeMap::new(), &fixture.access).unwrap(),
        None
    );
    check_browser_capture(&fixture.access, &fixture.scenario, &fixture.mounts).unwrap();
    fixture.scenario["payload_manifest"] = json!("/unexpected/payload.json");
    assert!(fixture.select(false).is_err());
    fixture.scenario["payload_manifest"] = Value::Null;
    write_json(&fixture.preparation.join("payload-report.json"), &json!({}));
    assert!(fixture.select(false).is_err());
    let extra = vec![Mount {
        guest_path: "/s9-browser-check".into(),
        object: object(b"unrequested"),
    }];
    assert!(check_browser_capture(&fixture.access, &fixture.scenario, &extra).is_err());
}

#[test]
fn captured_browser_driver_and_payload_require_exact_single_mounts() {
    let fixture = Fixture::new(true);
    check_browser_capture(&fixture.access, &fixture.scenario, &fixture.mounts).unwrap();
    for at in 0..fixture.mounts.len() {
        let mut root = fixture.mounts.clone();
        root.remove(at);
        assert!(check_browser_capture(&fixture.access, &fixture.scenario, &root).is_err());
        let mut root = fixture.mounts.clone();
        root.push(root[at].clone());
        assert!(check_browser_capture(&fixture.access, &fixture.scenario, &root).is_err());
        let mut root = fixture.mounts.clone();
        root[at].object = object(b"different captured bytes");
        assert!(check_browser_capture(&fixture.access, &fixture.scenario, &root).is_err());
    }
}

#[test]
#[ignore = "read-only retained preparation regression; set MD_SIM_PREPARED_BROWSER_RUN to the completed run directory"]
fn retained_browser_preparation_selects_exact_combined_image_without_boot() {
    let run = PathBuf::from(
        std::env::var_os("MD_SIM_PREPARED_BROWSER_RUN").expect("retained run path required"),
    );
    let preparation = run.join("client-image");
    let proof = read(&run.join("ssh-inputs.json")).unwrap();
    let scenario = read(&run.join("scenario.json")).unwrap();
    assert_eq!(
        proof["scenario"],
        json!(hash_file(&run.join("scenario.json")).unwrap())
    );
    let access = &proof["shared_support"]["carrier_dashboard"];
    let payload: ObjectId = serde_json::from_value(access["browser_payload"].clone()).unwrap();
    let prepared = read(&preparation.join("prepared-image.json")).unwrap();
    assert_eq!(
        prepared["kernel"],
        proof["shared_support"]["support_kernel"]
    );
    let kernel: ObjectId = serde_json::from_value(prepared["kernel"].clone()).unwrap();
    let mounts: Vec<Mount> =
        serde_json::from_value(read(&preparation.join("root-manifest.json")).unwrap()).unwrap();
    check_browser_capture(access, &scenario, &mounts).unwrap();
    let selected = select(&preparation, &scenario, &kernel, Some(&payload)).unwrap();
    assert_eq!(
        selected.path,
        preparation.join("combined-initramfs.cpio.gz")
    );
    assert_eq!(json!(selected.object), prepared["image"]);
    assert_ne!(
        selected.object,
        hash_file(&preparation.join("initramfs.cpio.gz")).unwrap()
    );
    assert!(select(&preparation, &scenario, &kernel, None).is_err());
    eprintln!(
        "Verified retained combined image {}:{}; no VM or firmware executed",
        selected.object.sha256, selected.object.size
    );
}
