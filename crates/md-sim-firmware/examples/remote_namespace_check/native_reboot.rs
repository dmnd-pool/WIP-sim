//! Full guest reset through the selected published firmware endpoint.
use super::*;

const BOOT_ID: &str = "cat /proc/sys/kernel/random/boot_id";
const REBOOT: &str =
    "(sleep 5; dmesg -n 8; sync; /sbin/reboot -f) </dev/null >/tmp/native-guest-reboot.log 2>&1 &";
const CONFIG: &str = "/etc/config/uhttpd";

fn boot_id(value: &Value) -> Result<&str> {
    let id = value.as_str().context("guest boot ID absent")?.trim();
    ensure!(
        id.len() == 36
            && id.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit() && !b.is_ascii_uppercase()
                }
            }),
        "guest boot ID malformed"
    );
    Ok(id)
}

fn value_for(identity: &ObjectId) -> Result<String> {
    Ok((101 + u32::from_str_radix(&identity.sha256[..8], 16)? % 900).to_string())
}

fn asset(http: &mut Http) -> Result<()> {
    let (code, bytes, _) = http.request("/luci-static/bootstrap/cascade.css", None, "")?;
    ensure!(
        code == 200
            && hex::encode(Sha256::digest(&bytes))
                == "8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477",
        "native reboot asset differs"
    );
    Ok(())
}

pub(super) fn check(identity: &ObjectId, events: &mut Vec<Value>) -> Result<()> {
    let base = format!("https://{WORKER}:8443");
    let mut before = Http::new(&base, "native-reboot-before", true)?;
    let old_session = before.login()?;
    asset(&mut before)?;
    let original = before.rpc(&old_session, "fs", "readfile", json!([CONFIG]))?;
    ensure!(original.is_string(), "original configuration absent");
    let old_boot = before.rpc(&old_session, "sys", "exec", json!([BOOT_ID]))?;
    boot_id(&old_boot)?;
    let written = value_for(identity)?;
    ensure!(
        before.rpc(
            &old_session,
            "uci",
            "get",
            json!(["uhttpd", "main", "max_connections"])
        )? == "100",
        "pre-reboot baseline differs"
    );
    ensure!(
        before.rpc(
            &old_session,
            "uci",
            "set",
            json!(["uhttpd", "main", "max_connections", written])
        )? == true,
        "pre-reboot set failed"
    );
    ensure!(
        before.rpc(&old_session, "uci", "commit", json!(["uhttpd"]))? == true,
        "pre-reboot commit failed"
    );
    ensure!(
        before.rpc(
            &old_session,
            "uci",
            "get",
            json!(["uhttpd", "main", "max_connections"])
        )? == written,
        "pre-reboot readback differs"
    );
    before.rpc(&old_session, "sys", "exec", json!([REBOOT]))?;
    let mut probe = Http::new(&base, "native-reboot-probe", true)?;
    let began = Instant::now();
    let mut unavailable = false;
    thread::sleep(Duration::from_secs(5));
    for attempt in 0..40 {
        ensure!(
            began.elapsed() < Duration::from_secs(240),
            "native reboot readiness deadline"
        );
        let Some((code, body, headers)) = probe.reboot_probe()? else {
            unavailable = true;
            ensure!(
                attempt < 39,
                "native guest did not restart within probe budget"
            );
            continue;
        };
        if code == 403 && unavailable {
            ensure!(
                std::str::from_utf8(&body)?.contains("luci_password"),
                "reboot login form absent"
            );
            let ids = headers
                .lines()
                .filter(|line| line.to_ascii_lowercase().starts_with("x-md-sim-run:"))
                .collect::<Vec<_>>();
            ensure!(
                ids.len() == 1
                    && ids[0].split_whitespace().nth(1) == Some(identity.sha256.as_str()),
                "reboot reached another published run"
            );
            break;
        }
        ensure!(
            matches!(code, 403 | 502 | 503 | 504),
            "unexpected reboot HTTP status {code}"
        );
        unavailable |= matches!(code, 502..=504);
        ensure!(
            attempt < 39,
            "native guest did not restart within probe budget"
        );
        thread::sleep(Duration::from_secs(2));
    }
    let mut after = Http::new(&base, "native-reboot-after", true)?;
    let session = after.login()?;
    asset(&mut after)?;
    let new_boot = after.rpc(&session, "sys", "exec", json!([BOOT_ID]))?;
    ensure!(
        boot_id(&old_boot)? != boot_id(&new_boot)?,
        "native reboot retained kernel boot ID"
    );
    ensure!(
        after.rpc(
            &session,
            "uci",
            "get",
            json!(["uhttpd", "main", "max_connections"])
        )? == written,
        "native reboot lost committed value"
    );
    ensure!(
        after.rpc(&session, "fs", "writefile", json!([CONFIG, original]))? == 1,
        "post-reboot restoration failed"
    );
    ensure!(
        after.rpc(&session, "fs", "readfile", json!([CONFIG]))? == original,
        "post-reboot restored bytes differ"
    );
    ensure!(
        after.rpc(
            &session,
            "uci",
            "get",
            json!(["uhttpd", "main", "max_connections"])
        )? == "100",
        "post-reboot restored value differs"
    );
    after.rpc(&session, "sys", "exec", json!(["sync"]))?;
    events.push(json!({"native_guest_reboot":true,"selected_run":identity.sha256,
        "before":before.observations,"after":after.observations,"probe":probe.observations,
        "boot_id_before":old_boot,"boot_id_after":new_boot,"configuration_written":written,
        "configuration_restored":"100","original_config_rpc_encoding":"base64",
        "original_config_rpc_sha256":hex::encode(Sha256::digest(original.as_str().unwrap().as_bytes())),
        "exact_restoration":true,"elapsed_ms":began.elapsed().as_millis(),
        "unclean_power_loss":false,"original_board_gate":false}));
    Ok(())
}

pub(super) fn verify(event: &Value, identity: &ObjectId) -> Result<()> {
    ensure!(
        event["native_guest_reboot"] == true && event["selected_run"] == identity.sha256,
        "guest reboot selected another run"
    );
    let before = verify_http(
        event["before"]
            .as_array()
            .context("reboot requests absent")?,
        true,
    )?;
    let after = verify_http(
        event["after"]
            .as_array()
            .context("reboot responses absent")?,
        true,
    )?;
    ensure!(
        before.len() == 7 && after.len() == 6,
        "reboot RPC sequence incomplete"
    );
    let original = &before[0]["response"]["result"];
    let written = value_for(identity)?;
    let expected_before = [
        ("fs", "readfile", json!([CONFIG])),
        ("sys", "exec", json!([BOOT_ID])),
        ("uci", "get", json!(["uhttpd", "main", "max_connections"])),
        (
            "uci",
            "set",
            json!(["uhttpd", "main", "max_connections", written]),
        ),
        ("uci", "commit", json!(["uhttpd"])),
        ("uci", "get", json!(["uhttpd", "main", "max_connections"])),
        ("sys", "exec", json!([REBOOT])),
    ];
    let expected_after = [
        ("sys", "exec", json!([BOOT_ID])),
        ("uci", "get", json!(["uhttpd", "main", "max_connections"])),
        ("fs", "writefile", json!([CONFIG, original])),
        ("fs", "readfile", json!([CONFIG])),
        ("uci", "get", json!(["uhttpd", "main", "max_connections"])),
        ("sys", "exec", json!(["sync"])),
    ];
    for (observed, (service, method, params)) in before
        .iter()
        .zip(expected_before)
        .chain(after.iter().zip(expected_after))
    {
        ensure!(
            observed["rpc_service"] == service
                && observed["rpc_method"] == method
                && observed["parameters"] == params,
            "reboot RPC sequence differs"
        );
    }
    for (rpc, expected) in [
        (before[2], json!("100")),
        (before[3], json!(true)),
        (before[4], json!(true)),
        (before[5], json!(written)),
        (after[1], json!(written)),
        (after[2], json!(1)),
        (after[3], original.clone()),
        (after[4], json!("100")),
    ] {
        ensure!(
            rpc["response"]["result"] == expected,
            "reboot RPC result differs"
        );
    }
    let old = &before[1]["response"]["result"];
    let new = &after[0]["response"]["result"];
    ensure!(
        boot_id(old)? != boot_id(new)?
            && event["boot_id_before"] == *old
            && event["boot_id_after"] == *new,
        "reboot boot IDs did not change"
    );
    ensure!(
        event["configuration_written"] == written
            && event["configuration_restored"] == "100"
            && event["exact_restoration"] == true
            && event["original_config_rpc_encoding"] == "base64"
            && event["original_config_rpc_sha256"]
                == hex::encode(Sha256::digest(
                    original
                        .as_str()
                        .context("original config absent")?
                        .as_bytes()
                ))
            && event["unclean_power_loss"] == false
            && event["original_board_gate"] == false,
        "reboot summary differs from observed results"
    );
    let probes = event["probe"].as_array().context("reboot probes absent")?;
    ensure!(
        !probes.is_empty() && probes.len() <= 40,
        "reboot probe count differs"
    );
    ensure!(
        probes.last().unwrap()["status"] == 403
            && probes.iter().any(|p| p["transport_error"] == "timeout"
                || p["status"]
                    .as_u64()
                    .is_some_and(|n| (502..=504).contains(&n))),
        "native reboot outage and recovery absent"
    );
    for probe in probes {
        let timeout = if probe["transport_error"] == "timeout" {
            let stderr = hex::decode(
                probe["stderr_hex"]
                    .as_str()
                    .context("timeout stderr absent")?,
            )?;
            ensure!(
                probe["curl_exit_code"] == 28
                    && probe["status"].is_null()
                    && std::str::from_utf8(&stderr)?.contains("curl: (28)"),
                "reboot transport failure differs"
            );
            true
        } else {
            false
        };
        ensure!(
            probe["path"] == "/cgi-bin/luci/"
                && probe["tls"] == true
                && (timeout
                    || probe["status"]
                        .as_u64()
                        .is_some_and(|n| matches!(n, 403 | 502..=504))),
            "reboot probe differs"
        );
    }
    Ok(())
}
