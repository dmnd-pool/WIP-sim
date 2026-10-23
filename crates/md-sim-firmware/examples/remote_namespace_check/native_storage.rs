//! Verify the retained reset trace and run-owned storage identities.
use super::*;

pub(super) fn verify(
    server: &[Value],
    backend: &Value,
    backend_identity: &ObjectId,
    captured: &Value,
) -> Result<()> {
    let record = |name: &str| -> Result<&Value> {
        let found = server
            .iter()
            .filter(|e| e["native_storage_record"] == name)
            .collect::<Vec<_>>();
        ensure!(
            found.len() == 1,
            "native storage {name} absent or duplicated"
        );
        Ok(&found[0]["record"])
    };
    let initial = record("storage-initial")?;
    let final_image = record("storage-final")?;
    for (image, stage) in [(initial, "formatted"), (final_image, "stopped")] {
        ensure!(image["stage"] == stage, "native storage stage differs");
        let encoded = image["gzip_hex"]
            .as_str()
            .context("native storage bytes absent")?;
        ensure!(
            encoded.len() <= 2 * 1024 * 1024,
            "native storage exceeds bound"
        );
        let bytes = hex::decode(encoded)?;
        ensure!(
            image["gzip_sha256"] == hex::encode(Sha256::digest(&bytes)),
            "native compressed storage hash differs"
        );
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)?;
        ensure!(
            raw.len() == 8 * 1024 * 1024
                && image["raw"]["size"] == raw.len()
                && image["raw"]["sha256"] == hex::encode(Sha256::digest(raw)),
            "native storage identity differs"
        );
    }
    ensure!(
        initial["raw"] == backend["storage"]["initial"]
            && initial["raw"] != final_image["raw"]
            && backend["storage"]["format"]["tool"] == captured["inputs"]["/usr/bin/mke2fs"]
            && backend["storage"]["format"]["exit_code"] == 0,
        "native SD input was not prepared by captured tool"
    );
    let lifecycle = record("lifecycle")?;
    ensure!(
        serde_json::from_value::<ObjectId>(lifecycle["run_manifest"].clone())? == *backend_identity
            && lifecycle["observed_kernel_resets"] == 1
            && lifecycle["ready_boots"] == 2
            && lifecycle["native_qemu_pid"].as_u64().is_some_and(|n| n > 0),
        "native guest reset lifecycle differs"
    );
    let trace = |name: &str, identity: &Value| -> Result<Vec<u8>> {
        let path = format!("/worker/native/{name}");
        let found = server
            .iter()
            .filter(|e| e["log"] == path)
            .collect::<Vec<_>>();
        ensure!(
            found.len() == 1 && found[0]["truncated"] == false,
            "native reset trace absent or truncated"
        );
        let bytes = hex::decode(
            found[0]["hex"]
                .as_str()
                .context("native reset trace absent")?,
        )?;
        ensure!(
            identity["size"] == bytes.len()
                && identity["sha256"] == hex::encode(Sha256::digest(&bytes)),
            "native reset trace identity differs"
        );
        Ok(bytes)
    };
    let serial = trace("serial.log", &lifecycle["serial"])?;
    let positions = |needle: &[u8]| {
        serial
            .windows(needle.len())
            .enumerate()
            .filter_map(|(n, w)| (w == needle).then_some(n))
            .collect::<Vec<_>>()
    };
    let ready = positions(b"\r\nMD_SIM_NATIVE_NETWORK_READY\r\n");
    let reset = positions(b"reboot: Restarting system");
    ensure!(
        ready.len() == 2 && reset.len() == 1 && ready[0] < reset[0] && reset[0] < ready[1],
        "native guest reset did not separate two real service boots"
    );
    let startup = trace("startup.jsonl", &lifecycle["startup"])?;
    let rows = std::str::from_utf8(&startup)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(
        rows.iter()
            .filter(|r| r["actual_readiness_marker"] == true)
            .count()
            == 2
            && rows
                .iter()
                .filter(|r| r["observed_kernel_reset"] == 1)
                .count()
                == 1
            && rows
                .iter()
                .all(|r| r["native_qemu_pid"] == lifecycle["native_qemu_pid"]),
        "native reset used another emulator or omitted readiness evidence"
    );
    Ok(())
}
