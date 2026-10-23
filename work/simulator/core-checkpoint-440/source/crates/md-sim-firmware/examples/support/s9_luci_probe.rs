//! Exercise the archived LuCI handlers through their advertised Boser route.
//! Test invalid uploads, signed validation/cancellation, or an explicit native
//! installation request. Installation still needs separate reboot/readback evidence.
use super::{advancing_uptime, maintenance, object, update_validation, Client, UPTIME};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, io::Read, path::Component};

const FLASH: &str = "/cgi-bin/luci/admin/system/flashops";
const BACKUP: &str = "/cgi-bin/luci/admin/system/flashops/backup";
const UPGRADE: &str = "/cgi-bin/luci/admin/system/flashops/sysupgrade";
const INVALID_IMAGE: &[u8] = b"md-sim invalid firmware image; upload validation test only\n";
const BOUNDARY: &str = "mdSimLuciValidationBoundaryV1";

fn header(client: &Client, name: &str) -> Result<String> {
    let record = client
        .observations
        .last()
        .context("HTTP observation missing")?;
    let bytes = hex::decode(record["headers_hex"].as_str().context("headers missing")?)?;
    let headers = std::str::from_utf8(&bytes)?;
    let values: Vec<_> = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
        .collect();
    ensure!(
        values.len() == 1,
        "missing or duplicate native header: {name}"
    );
    Ok(values[0].into())
}

fn token(body: &[u8]) -> Result<String> {
    let html = std::str::from_utf8(body)?;
    ensure!(
        html.contains("Flash operations")
            && html.contains(&format!("action=\"{BACKUP}\""))
            && html.contains(&format!("action=\"{UPGRADE}\"")),
        "native LuCI flash form is absent"
    );
    // This is the exact pinned flashops.htm template, not a general HTML parser.
    // Each form has the same session token. Reject a changed or ambiguous form.
    let mut tokens = BTreeSet::new();
    for suffix in html.split("name=\"token\" value=\"").skip(1) {
        let value = suffix.split_once('"').context("unterminated CSRF token")?.0;
        // Direct LuCI creates a 16-byte token. The archived Boser service's
        // authenticated proxy returned literal0000 in original058. Preserve
        // that native value and test rejection of a different value below;
        // do not claim token randomness or change the firmware's behavior.
        ensure!(
            value == "0000"
                || (value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())),
            "native CSRF token format differs"
        );
        tokens.insert(value);
    }
    ensure!(
        tokens.len() == 1,
        "missing or conflicting native CSRF tokens"
    );
    Ok(tokens.first().context("CSRF token absent")?.to_string())
}

fn form(client: &mut Client, path: &str, body: &str, expected: u16) -> Result<Vec<u8>> {
    client.exchange(
        path,
        Some(("application/x-www-form-urlencoded", body.as_bytes())),
        Some(json!({"kind":"urlencoded-form", "body":object(body.as_bytes())})),
        &[expected],
    )
}

fn invalid_upload(token: &str) -> Vec<u8> {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"token\"\r\n\r\n{token}\r\n\
         --{BOUNDARY}\r\nContent-Disposition: form-data; name=\"image\"; filename=\"md-sim-invalid.bin\"\r\n\
         Content-Type: application/octet-stream\r\n\r\n"
    ).into_bytes();
    body.extend_from_slice(INVALID_IMAGE);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

fn inspect_backup(bytes: &[u8]) -> Result<Value> {
    // Decode without extracting or executing a member. Bound decompressed data
    // before the tar reader can allocate based on an acquired header.
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut decoded)?;
    ensure!(
        decoded.len() <= 4 * 1024 * 1024,
        "backup decompression bound"
    );
    let mut archive = tar::Archive::new(decoded.as_slice());
    let mut members = Vec::new();
    let mut paths = BTreeSet::new();
    let mut network = None;
    let mut public_storage = None;
    for entry in archive.entries()? {
        let mut entry = entry?;
        ensure!(members.len() < 256, "backup member count bound");
        let path = entry.path()?.into_owned();
        ensure!(
            path.components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
            "unsafe backup member path"
        );
        let normalized: std::path::PathBuf = path
            .components()
            .filter(|c| !matches!(c, Component::CurDir))
            .collect();
        let name = normalized
            .to_str()
            .context("backup path encoding")?
            .to_owned();
        ensure!(
            !name.is_empty() && paths.insert(name.clone()),
            "duplicate backup member"
        );
        let kind = entry.header().entry_type();
        if kind.is_file() {
            ensure!(entry.size() <= 1024 * 1024, "backup member size bound");
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            if name == "etc/config/network" {
                let text = std::str::from_utf8(&content)?;
                ensure!(
                    text.contains("10.0.2.15"),
                    "backup does not contain active private network"
                );
                network = Some(object(&content));
            }
            if name == "etc/web_storage.json" {
                let storage: Value = serde_json::from_slice(&content)?;
                let saved = storage["public"]
                    .as_object()
                    .context("native saved public settings absent")?;
                // The independently inspected native file uses snake_case.
                // GraphQL exposes these same fields in camelCase. Keep the
                // file values in this record. The preservation check below
                // handles the declared native theme enum separately.
                ensure!(
                    saved.len() == 3
                        && ["web_theme", "dashboard_density", "preferred_language"]
                            .iter()
                            .all(|name| saved.contains_key(*name)),
                    "native saved settings fields differ"
                );
                let public = json!({"webTheme":saved["web_theme"],
                    "dashboardDensity":saved["dashboard_density"],
                    "preferredLanguage":saved["preferred_language"]});
                super::dashboard_state::storage(&public)?;
                public_storage = Some(public);
            }
            members.push(json!({"path":name,"kind":"file","content":object(&content)}));
        } else {
            ensure!(
                kind.is_dir() || kind.is_symlink(),
                "unexpected backup member kind"
            );
            members.push(json!({"path":name,"kind":kind.as_byte(),"link":entry.link_name()?}));
        }
    }
    ensure!(
        network.is_some(),
        "native backup omits network configuration"
    );
    Ok(
        json!({"compressed":object(bytes),"decoded":object(&decoded),"members":members,
        "active_network":network,"public_storage":public_storage,
        "public_storage_encoding":"Native file values with camelCase field names. Native theme Dark/Light differs from GraphQL DARK/LIGHT.",
        "extracted":false}),
    )
}

fn require_saved_storage(backup: &Value, expected: &Value) -> Result<()> {
    super::dashboard_state::storage(expected)?;
    // Run079 observes an actual GraphQL DARK write saved as native JSON Dark.
    // Use an explicit enum mapping, not a case-insensitive comparison. An
    // unknown value, missing member or changed setting must still fail.
    let mut saved_expected = expected.clone();
    saved_expected["webTheme"] = match expected["webTheme"].as_str() {
        Some("DARK") => json!("Dark"),
        Some("LIGHT") => json!("Light"),
        None if expected["webTheme"].is_null() => Value::Null,
        _ => anyhow::bail!("unsupported GraphQL theme for native backup verification"),
    };
    ensure!(
        backup["public_storage"] == saved_expected,
        "native backup omits or changes the requested dashboard settings: saved={}, expected_native={saved_expected}; configure the native keep-file policy before this persistence test",
        backup["public_storage"]
    );
    Ok(())
}

fn require_rejection(bytes: &[u8], csrf: &str) -> Result<()> {
    let text = std::str::from_utf8(bytes)?;
    ensure!(
        text.contains("The uploaded image file does not contain a supported format.")
            && !text.contains("name=\"step\" value=\"2\"")
            && !text.contains("The system is flashing now.")
            && token(bytes)? == csrf,
        "damaged signed image was not rejected by the native handler"
    );
    Ok(())
}

pub(super) fn check(client: &mut Client, requested_persistence: Option<&Value>) -> Result<Value> {
    let before = client.graphql(
        "query { bos { hostname uptime { since durationS } } ui { systemLinks { href label } } }",
        json!({}),
    )?;
    // Validate identity before any service or configuration change. The
    // later sample must still belong to this same native boot.
    md_sim_firmware::s9_telemetry::uptime_progress(&before["bos"], &before["bos"])?;
    let links = before["ui"]["systemLinks"]
        .as_array()
        .context("native system links absent")?;
    ensure!(
        links.iter().filter(|link| link["href"] == FLASH).count() == 1,
        "selected firmware does not advertise the LuCI flash route"
    );
    let mut page = client.exchange(FLASH, None, None, &[200, 403])?;
    let separate_login = client
        .observations
        .last()
        .context("native response absent")?["http_status"]
        == "403";
    if separate_login {
        let login = std::str::from_utf8(&page)?;
        ensure!(
            login.contains("luci_username") && login.contains("luci_password"),
            "LuCI login form absent"
        );
        form(client, FLASH, "luci_username=root&luci_password=", 302)?;
        ensure!(
            header(client, "location")? == FLASH,
            "native login redirect leaves selected route"
        );
        page = client.request(FLASH, None)?;
    }
    let csrf = token(&page)?;
    let bad = if csrf == "00000000000000000000000000000000" {
        "11111111111111111111111111111111"
    } else {
        "00000000000000000000000000000000"
    };
    form(client, BACKUP, &format!("token={bad}&backup=1"), 403)?;
    client.exchange(BACKUP, None, None, &[405])?;
    ensure!(
        header(client, "allow")? == "POST",
        "backup method guard differs"
    );
    let backup = form(client, BACKUP, &format!("token={csrf}&backup=1"), 200)?;
    ensure!(
        header(client, "content-type")? == "application/x-targz",
        "native backup type differs"
    );
    let disposition = header(client, "content-disposition")?;
    ensure!(
        disposition.contains("attachment;")
            && disposition.contains("backup-")
            && disposition.contains(".tar.gz"),
        "native backup download name absent"
    );
    let backup = inspect_backup(&backup)?;
    let upload = invalid_upload(&csrf);
    let rejected = client.exchange(UPGRADE,
        Some((&format!("multipart/form-data; boundary={BOUNDARY}"), &upload)),
        Some(json!({"kind":"multipart-invalid-image","body":object(&upload),"image":object(INVALID_IMAGE)})), &[200])?;
    ensure!(
        std::str::from_utf8(&rejected)?
            .contains("The uploaded image file does not contain a supported format."),
        "invalid image was not rejected by native validation"
    );
    ensure!(
        token(&rejected)? == csrf,
        "session changed during invalid upload"
    );
    let mut rejected_update = None;
    let valid_update = if std::path::Path::new("/dashboard-update.json").exists() {
        let contract: Value =
            serde_json::from_slice(&super::read("/dashboard-update.json", 4096)?)?;
        let action = update_validation::Action::from_schema(
            contract["schema"]
                .as_str()
                .context("native update schema absent")?,
        )?;
        let image = action.checked_image(std::path::Path::new(update_validation::GUEST_IMAGE))?;
        ensure!(
            contract["artifact_id"] == update_validation::ARTIFACT
                && contract["object"] == serde_json::to_value(&image)?
                && contract["mode"] == "sd"
                && contract["client_operation_seconds"] == update_validation::CLIENT_SECONDS
                && contract["pool_seconds"] == update_validation::POOL_SECONDS
                && contract["aggregate_trace_bytes"] == update_validation::TRACE_BYTES
                && contract["maintenance"] == action.maintenance()
                && contract["platform"] == "am1-s9",
            "signed update contract differs"
        );
        if action == update_validation::Action::RejectTampered {
            ensure!(
                contract["parent"]
                    == json!({"sha256":update_validation::SHA256,"size":update_validation::BYTES})
                    && contract["fault"]
                        == json!({"archive_offset":update_validation::FAULT_OFFSET,"xor_mask":1}),
                "damaged update parent or fault differs"
            );
        }
        ensure!(
            action != update_validation::Action::InstallKeep
                || contract["restart_observation_seconds"] == update_validation::RESTART_SECONDS,
            "native update restart policy differs"
        );
        if action == update_validation::Action::InstallKeep {
            if let Some(expected) = requested_persistence {
                // Run077's native backup omitted web_storage.json. A keep=1
                // request cannot promise preservation of a file that the
                // actual backup omits. Check the selected native policy before
                // stopping mining or committing an installation.
                require_saved_storage(&backup, expected)?;
            }
        }
        let stopped = maintenance::stop(client)?;
        let validation = (|| -> Result<(Vec<u8>, Option<Value>)> {
            let confirmation = client.update_upload(UPGRADE, &csrf, action)?;
            if action == update_validation::Action::RejectTampered {
                require_rejection(&confirmation, &csrf)?;
                return Ok((confirmation, None));
            }
            let text = std::str::from_utf8(&confirmation)?;
            ensure!(
                text.contains(update_validation::SHA256)
                    && text.contains("name=\"step\" value=\"2\"")
                    && text.contains("name=\"cancel\""),
                "native update validation did not return exact SHA256 and confirmation controls"
            );
            if action == update_validation::Action::InstallKeep {
                // This is still before the commit. A failed identity or
                // uptime check must take the miner restart path below.
                let uptime = client.graphql(UPTIME, json!({}))?;
                advancing_uptime(&before, &uptime)?;
                return Ok((confirmation, Some(uptime)));
            }
            form(client, UPGRADE, &format!("token={csrf}&cancel=1"), 302)?;
            ensure!(
                header(client, "location")? == FLASH,
                "native update cancel redirect differs"
            );
            let returned = client.request(FLASH, None)?;
            ensure!(
                token(&returned)? == csrf,
                "native session changed after update cancellation"
            );
            Ok((confirmation, None))
        })();
        if action == update_validation::Action::InstallKeep && validation.is_ok() {
            let (confirmation, uptime_at_commit) = validation?;
            // From here, failure can mean the native updater already started.
            // Do not issue a miner-start request that could race its shutdown.
            let response = form(client, UPGRADE, &format!("token={csrf}&step=2&keep=1"), 200)?;
            let text = std::str::from_utf8(&response)?;
            ensure!(
                text.contains("Flashing...") && text.contains("The system is flashing now."),
                "native update commit response differs"
            );
            let result = json!({"contract":contract,"native_confirmation":object(&confirmation),
                "reported_sha256":update_validation::SHA256,"cancelled":false,"commit_requested":true,
                "keep_settings":true,"step":2,"native_commit_response":object(&response),
                "uptime_at_commit":uptime_at_commit,"maintenance":{"stop":stopped},
                "installation_gate":"pending actual reboot and post-update boot/readback"});
            println!("MD_SIM_UPDATE_COMMIT {result}");
            Some(result)
        } else {
            // Attempt restart even if validation failed. Preserve both failures.
            let restarted = maintenance::start(client, &stopped);
            let maintenance = json!({"stop":stopped,"start":restarted.as_ref().ok(),
            "start_error":restarted.as_ref().err().map(|e|format!("{e:#}"))});
            println!("MD_SIM_UPDATE_MAINTENANCE {maintenance}");
            let (confirmation, _) = validation?;
            restarted?;
            if action == update_validation::Action::RejectTampered {
                rejected_update = Some(
                    json!({"contract":contract,"native_rejection":object(&confirmation),
                    "rejected":true,"commit_requested":false,"install_controls_present":false,
                    "maintenance":maintenance,
                    "scope":"Native HTTP handler rejected this exact damaged signed image. The response does not distinguish signature failure from another native format check."}),
                );
                None
            } else {
                Some(
                    json!({"contract":contract,"native_confirmation":object(&confirmation),
            "reported_sha256":update_validation::SHA256,"cancelled":true,"commit_requested":false,
            "maintenance":maintenance}),
                )
            }
        }
    } else {
        None
    };
    let committed = valid_update
        .as_ref()
        .is_some_and(|v| v["commit_requested"] == true);
    let after = if committed {
        None
    } else {
        let after = client.graphql(UPTIME, json!({}))?;
        advancing_uptime(&before, &after)?;
        Some(after)
    };
    Ok(
        json!({"schema":"md-sim-original-s9-luci-probe/1","route":FLASH,
        "advertised_links":links,"separate_login":separate_login,"backup":backup,"csrf_rejection":true,"method_rejection":true,
        "invalid_image":object(INVALID_IMAGE),"invalid_upload_rejected":true,"valid_update":valid_update,"rejected_update":rejected_update,"before":before,"after":after,
        "scope":"Native backup, invalid upload rejection, signed validation/cancellation or explicit native install request. Installation needs actual reboot and post-update readback. No restore, browser rendering, recovery or complete installation pass is inferred from HTTP."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damaged_image_needs_rejection_without_install_confirmation() {
        let page = format!("Flash operations action=\"{BACKUP}\" action=\"{UPGRADE}\" name=\"token\" value=\"0000\" The uploaded image file does not contain a supported format.");
        require_rejection(page.as_bytes(), "0000").unwrap();
        for rejected in [
            page.replace("supported format.", "unknown text"),
            format!("{page} name=\"step\" value=\"2\""),
            format!("{page} The system is flashing now."),
            page.replace("0000", "0123456789abcdef0123456789abcdef"),
        ] {
            assert!(require_rejection(rejected.as_bytes(), "0000").is_err());
        }
    }

    #[test]
    fn form_token_requires_native_routes_and_one_session() {
        let template = format!("Flash operations action=\"{BACKUP}\" action=\"{UPGRADE}\" name=\"token\" value=\"0123456789abcdef0123456789abcdef\"");
        assert_eq!(
            token(template.as_bytes()).unwrap(),
            "0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            token(
                template
                    .replace("0123456789abcdef0123456789abcdef", "0000")
                    .as_bytes()
            )
            .unwrap(),
            "0000"
        );
        assert!(token(
            template
                .replace("0123456789abcdef0123456789abcdef", "1234")
                .as_bytes()
        )
        .is_err());
        assert!(token(
            format!("{template} name=\"token\" value=\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"")
                .as_bytes()
        )
        .is_err());
        assert!(token(
            template
                .replace("0123456789abcdef0123456789abcdef", "&quot;injected")
                .as_bytes()
        )
        .is_err());
        assert!(token(template.replace(UPGRADE, "/unknown").as_bytes()).is_err());
        assert!(token(b"<html>Login</html>").is_err());
    }

    #[test]
    fn backup_requires_real_archive_and_active_configuration() {
        assert!(inspect_backup(b"not a gzip archive").is_err());
        let mut tar = tar::Builder::new(Vec::new());
        let contents = b"config interface 'lan'\n option ipaddr '10.0.2.15'\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        tar.append_data(&mut header, "etc/config/network", &contents[..])
            .unwrap();
        let storage = br#"{"public":{"web_theme":"Dark","dashboard_density":null,"preferred_language":null},"private":{"unit_fixture":"not exported"}}"#;
        header.set_size(storage.len() as u64);
        header.set_cksum();
        tar.append_data(&mut header, "etc/web_storage.json", &storage[..])
            .unwrap();
        let tar = tar.into_inner().unwrap();
        let mut compressed =
            flate2::read::GzEncoder::new(tar.as_slice(), flate2::Compression::default());
        let mut bytes = Vec::new();
        compressed.read_to_end(&mut bytes).unwrap();
        let result = inspect_backup(&bytes).unwrap();
        assert_eq!(result["active_network"]["size"], contents.len());
        let expected = json!({"webTheme":"DARK","dashboardDensity":null,"preferredLanguage":null});
        require_saved_storage(&result, &expected).unwrap();
        assert!(!result.to_string().contains("not exported"));
        let mut changed = result.clone();
        changed["public_storage"] = Value::Null;
        assert!(require_saved_storage(&changed, &expected).is_err());
        changed["public_storage"] = expected.clone();
        changed["public_storage"]["webTheme"] = json!("LIGHT");
        assert!(require_saved_storage(&changed, &expected).is_err());
        for invalid in ["DARK", "dark", "Dark ", "unknown"] {
            changed["public_storage"]["webTheme"] = json!(invalid);
            assert!(require_saved_storage(&changed, &expected).is_err());
        }
        assert!(require_saved_storage(&result, &json!({})).is_err());
        // Corrupt the independently checked gzip footer.
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(inspect_backup(&bytes).is_err());
    }
}
