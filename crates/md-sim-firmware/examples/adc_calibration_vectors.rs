//! Generate and check an independent original-C calibration arithmetic oracle.
//! No physical calibration, firmware boot or mining gate is established.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::{
    esp32s3_adc_calibration::{Calibration, IMAGE_SIZE},
    prepare::hash_bytes,
};
use md_sim_runtime::isolation::save_json;
use serde_json::{json, Value};
use std::{fs, io::Read, path::Path};
#[path = "support/bounded_file.rs"]
mod bounded_file;

const PREFIX: &[u8] =
    include_bytes!("../../../simulator/firmware/fixtures/esp32s3-adc-oracle-prefix.c");
const SUFFIX: &[u8] =
    include_bytes!("../../../simulator/firmware/fixtures/esp32s3-adc-oracle-suffix.c");
const SOURCE_ROOT: &str = "artifacts/files/source/esp-idf-v553-adc-reference/components";

fn images() -> Vec<[u8; IMAGE_SIZE]> {
    let mut result = Vec::new();
    for fill in [0, 255, 0x55, 0xaa] {
        let mut image = [fill; IMAGE_SIZE];
        image[64] = (image[64] & !3) | 1;
        result.push(image);
    }
    for (block, bit) in (0..256)
        .filter(|b| ![128, 129].contains(b))
        .map(|b| (48, b))
        .chain((186..192).map(|b| (24, b)))
    {
        let mut image = [0; IMAGE_SIZE];
        image[64] = 1;
        image[block + bit / 8] |= 1 << (bit % 8);
        result.push(image);
    }
    result
}

fn parent(root: &Path, name: &str, expected: &str) -> Result<(String, Value)> {
    let path = root.join(SOURCE_ROOT).join(name);
    let captured = bounded_file::capture(&path, 128 << 10)?;
    ensure!(
        captured.object.sha256 == expected,
        "original ADC source differs: {name}"
    );
    Ok((
        String::from_utf8(captured.bytes)?,
        json!({"path":path,"object":captured.object}),
    ))
}

fn without_includes(raw: &str) -> String {
    raw.split_inclusive('\n')
        .filter(|line| !line.starts_with("#include "))
        .collect()
}

fn exact_function<'a>(raw: &'a str, start: &str) -> Result<&'a str> {
    ensure!(
        raw.matches(start).count() == 1,
        "function marker not unique"
    );
    let begin = raw.find(start).unwrap();
    let end = raw[begin..].find("\n}\n").context("function end absent")? + begin + 3;
    Ok(&raw[begin..end])
}

fn generated(root: &Path) -> Result<(String, Value)> {
    let root = root.canonicalize()?;
    let (table, table_id) = parent(
        &root,
        "efuse/esp32s3/esp_efuse_table.c",
        "ebbff31657f49a24c10e1c98947f2f0d00190c985cef2f4714a1c92aa00bcb5f",
    )?;
    let (rtc, rtc_id) = parent(
        &root,
        "efuse/esp32s3/esp_efuse_rtc_calib.c",
        "449b9c4559a08cae21aacd5230390abb2966e25df9333b80fbcd2faa688128f1",
    )?;
    let (coeff, coeff_id) = parent(
        &root,
        "esp_adc/esp32s3/curve_fitting_coefficients.c",
        "9995bb0e8b76caf0c41268391fcad62f94b6adf7dcdd76bbfb1e7dbb770a7191",
    )?;
    let (curve, curve_id) = parent(
        &root,
        "esp_adc/adc_cali_curve_fitting.c",
        "c5298fde12b7d019148c9d26be894b4837bf99248a1ca5ebb9cfe5de2d8f47b9",
    )?;
    let starts=[
        "int esp_efuse_rtc_calib_get_ver(void)\n{",
        "uint32_t esp_efuse_rtc_calib_get_init_code(int version, uint32_t adc_unit, int atten)\n{",
        "esp_err_t esp_efuse_rtc_calib_get_cal_voltage(int version, uint32_t adc_unit, int atten, uint32_t *out_digi, uint32_t *out_vol_mv)\n{",
    ];
    let mut source = String::from_utf8(PREFIX.to_vec())?;
    source.push_str(&without_includes(&table));
    let mut ranges = Vec::new();
    for start in starts {
        let text = exact_function(&rtc, start)?;
        let at = rtc.find(text).unwrap();
        ranges.push(json!({"parent":rtc_id,"offset":at,"size":text.len(),"sha256":hash_bytes(text.as_bytes())}));
        source.push_str(text);
    }
    source.push_str(&without_includes(&coeff));
    let text=exact_function(&curve,"static int32_t get_reading_error(uint64_t v_cali_1, const cali_chars_second_step_t *param)\n{")?;
    ranges.push(json!({"parent":curve_id,"offset":curve.find(text).unwrap(),"size":text.len(),"sha256":hash_bytes(text.as_bytes())}));
    source.push_str(text);
    source.push_str("\nstatic const uint8_t images[][336]={\n");
    for image in images() {
        source.push('{');
        for byte in image {
            source.push_str(&format!("{byte},"));
        }
        source.push_str("},\n");
    }
    source.push_str("};\n");
    source.push_str(std::str::from_utf8(SUFFIX)?);
    let manifest = json!({"schema":"md-sim-adc-source-oracle/1",
        "parents":[table_id,rtc_id,coeff_id,curve_id],"retained_function_ranges":ranges,
        "include_removal":"Only lines beginning #include are removed from the complete table and coefficient source; other bytes remain unchanged.",
        "prefix_sha256":hash_bytes(PREFIX),"suffix_sha256":hash_bytes(SUFFIX),"source_sha256":hash_bytes(source.as_bytes()),
        "adaptations":"Explicit test images; fixed-width host C ABI; SDK field-reader, assert and log callbacks; original get_reading_error executes unchanged. No ADC, physical silicon or full firmware execution.",
        "images":images().len(),"full_sample_profiles":4,"samples_per_profile":32768,
        "physical_calibration_gate":false,"original_board_gate":false});
    Ok((source, manifest))
}

fn prepare(root: &Path, out: &Path) -> Result<()> {
    let (source, manifest) = generated(root)?;
    fs::create_dir(out)?;
    fs::write(out.join("oracle.c"), source)?;
    save_json(&out.join("source.json"), &manifest)
}

fn require_canonical(
    source: &[u8],
    manifest: &Value,
    expected_source: &str,
    expected_manifest: &Value,
) -> Result<()> {
    ensure!(
        source == expected_source.as_bytes() && manifest == expected_manifest,
        "prepared source or manifest differs from canonical pinned original-C oracle"
    );
    Ok(())
}

fn verify_words(bytes: &[u8]) -> Result<Value> {
    let cases = images();
    let expected_words = 4 + cases.len() * (2 + 24) + 4 * 32768;
    ensure!(bytes.len() == expected_words * 4, "oracle output length");
    let mut words = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    ensure!(
        words.next() == Some(0x41444331)
            && words.next() == Some(1)
            && words.next() == Some(cases.len() as u32)
            && words.next() == Some(4),
        "oracle header"
    );
    for (index, image) in cases.iter().enumerate() {
        ensure!(
            words.next() == Some(index as u32) && words.next() == Some(1),
            "oracle case identity"
        );
        let cal = Calibration::decode(image).map_err(|e| anyhow::anyhow!("decoder error:{e:?}"))?;
        for unit in 0..2 {
            for attenuation in 0..4 {
                let point = cal.point(unit, attenuation).unwrap();
                ensure!(
                    words.next() == Some(point.init_code.into())
                        && words.next() == Some(point.reference_digit.into())
                        && words.next() == Some(point.reference_mv.into()),
                    "oracle point mismatch:{index}/{unit}/{attenuation}"
                );
                if index < 4 {
                    for raw in 0..=4095 {
                        let mv = cal.raw_to_mv(unit, attenuation, raw).unwrap();
                        ensure!(
                            words.next() == Some(mv as u32),
                            "original-C conversion mismatch:{index}/{unit}/{attenuation}/{raw}"
                        );
                    }
                }
            }
        }
    }
    ensure!(words.next().is_none(), "oracle trailing words");
    Ok(
        json!({"cases":cases.len(),"points":cases.len()*8,"original_c_sample_comparisons":4*32768,
        "output_sha256":hash_bytes(bytes),"scope":"Exact pinned source arithmetic with replaced SDK callbacks and controlled images. Not measured ADC transfer, physical calibration or full firmware acceptance."}),
    )
}

fn verify(root: &Path, prepared: &Path, execution: &Path, out: &Path) -> Result<()> {
    let source = bounded_file::capture(&prepared.join("oracle.c"), 1 << 20)?;
    let source_manifest = bounded_file::capture(&prepared.join("source.json"), 1 << 20)?;
    let manifest: Value = serde_json::from_slice(&source_manifest.bytes)?;
    let (expected_source, expected_manifest) = generated(root)?;
    require_canonical(
        &source.bytes,
        &manifest,
        &expected_source,
        &expected_manifest,
    )?;
    ensure!(
        manifest["source_sha256"] == source.object.sha256,
        "prepared source mismatch"
    );
    let result = bounded_file::capture(&execution.join("result.json"), 16 << 20)?;
    let process: Value = serde_json::from_slice(&result.bytes)?;
    ensure!(
        process["exit_code"] == 0 && process["trace_truncated"] == false,
        "isolated oracle process failed"
    );
    let raw = bounded_file::capture(&execution.join("evidence.tar"), 8 << 20)?;
    ensure!(
        hex::decode(
            process["stdout"]
                .as_str()
                .context("process stdout absent")?
        )? == raw.bytes,
        "archive differs from process stdout"
    );
    let mut archive = tar::Archive::new(&raw.bytes[..]);
    let mut files = std::collections::BTreeMap::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        ensure!(
            entry.header().entry_type().is_file() && entry.size() <= 2 << 20,
            "oracle entry bound/type"
        );
        let name = entry.path()?.into_owned();
        ensure!(
            name.components().count() == 1 && !name.is_absolute(),
            "oracle entry name"
        );
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        ensure!(
            files.insert(name, bytes).is_none(),
            "duplicate oracle entry"
        );
    }
    let assessment: Value = serde_json::from_slice(
        files
            .get(Path::new("assessment.json"))
            .context("assessment absent")?,
    )?;
    ensure!(
        assessment["passed"] == true
            && assessment["source"] == serde_json::to_value(&source.object)?,
        "oracle source/result mismatch"
    );
    let words = files
        .get(Path::new("execution.stdout"))
        .context("oracle words absent")?;
    let mut report = verify_words(words)?;
    report["passed"] = json!(true);
    report["verifier"] = serde_json::to_value(md_sim_runtime::store::hash_file(Path::new(
        "/proc/self/exe",
    ))?)?;
    report["decoder_source_sha256"] = json!(hash_bytes(include_bytes!(
        "../src/esp32s3_adc_calibration.rs"
    )));
    report["inputs"] = json!({"prepared_source":source.object,"source_manifest":source_manifest.object,"process_result":result.object,"evidence_archive":raw.object});
    fs::create_dir(out)?;
    save_json(&out.join("verified.json"), &report)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    match args.get(1).and_then(|s|s.to_str()) {
        Some("prepare") if args.len()==4 => prepare(Path::new(&args[2]),Path::new(&args[3])),
        Some("verify") if args.len()==6 => verify(Path::new(&args[2]),Path::new(&args[3]),Path::new(&args[4]),Path::new(&args[5])),
        _=>anyhow::bail!("usage: adc_calibration_vectors prepare ROOT NEW_OUTPUT | verify ROOT PREPARED EXECUTION NEW_OUTPUT"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn function_selection_requires_exact_unique_marker() {
        assert!(exact_function("missing", "int f()\n{").is_err());
        assert!(exact_function("int f()\n{\n}\nint f()\n{\n}\n", "int f()\n{").is_err());
        assert_eq!(
            exact_function("comment\nint f()\n{\n}\nend", "int f()\n{").unwrap(),
            "int f()\n{\n}\n"
        );
    }
    #[test]
    fn malformed_oracle_words_are_rejected() {
        for bytes in [
            vec![],
            vec![0; 16],
            vec![0; (4 + images().len() * 26 + 4 * 32768) * 4],
        ] {
            assert!(verify_words(&bytes).is_err());
        }
    }

    #[test]
    fn substituted_c_with_self_consistent_hash_is_rejected() {
        let expected = "original independent C arithmetic";
        let manifest = json!({"source_sha256":hash_bytes(expected.as_bytes())});
        require_canonical(expected.as_bytes(), &manifest, expected, &manifest).unwrap();
        let substitute = b"program that prints precomputed Rust answers";
        let fake_manifest = json!({"source_sha256":hash_bytes(substitute)});
        assert!(require_canonical(substitute, &fake_manifest, expected, &manifest).is_err());
        assert!(
            require_canonical(expected.as_bytes(), &fake_manifest, expected, &manifest).is_err()
        );
    }
}
