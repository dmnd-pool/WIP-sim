//! Metadata checks are component regressions. Real worker/SSH/TLS/native
//! integration is assessed by its separately retained selected execution.
use super::*;
use crate::{
    carrier::tests::{fixture, policy},
    gateway::BrowserPolicy,
};
use std::os::unix::fs::PermissionsExt;

pub(crate) fn gateway_fixture() -> Result<(ProviderBinding, CarrierEndpoint, GatewaySpec)> {
    let (_, _, _, binding) = fixture(&policy())?;
    let endpoint = CarrierEndpoint {
        schema: "md-sim-carrier-dashboard-endpoint/1".into(),
        identity: binding.selection().identity().clone(),
        provider_configuration: binding.configuration().clone(),
        policy: binding.selection().policy().clone(),
        listen: "127.0.0.1:18090".parse()?,
        local_socket: "/work/access/bridge/http.sock".into(),
        firmware_readiness_established: false,
    };
    let spec = GatewaySpec {
        run_id: endpoint.identity.run_id.clone(),
        execution_identity: endpoint.identity.locked_run.clone(),
        dashboard_source: "adapted-native".into(),
        listen: "192.0.2.1:9443".parse()?,
        upstream: endpoint.listen,
        upstream_unix: Some(endpoint.local_socket.clone()),
        upstream_tls: None,
        browser_policy: BrowserPolicy::Preserve,
        certificate: "/credentials/server.pem".into(),
        private_key: "/credentials/server.key".into(),
        client_ca: "/credentials/client-ca.pem".into(),
        allow_clients: vec!["192.0.2.2/32".into()],
        browser_context_id: "ab".repeat(32),
        nginx: "/usr/sbin/nginx".into(),
        openssl: "/usr/bin/openssl".into(),
    };
    Ok((binding, endpoint, spec))
}

#[test]
fn publication_requires_the_actual_selected_carrier_without_host_paths() -> Result<()> {
    let (binding, endpoint, spec) = gateway_fixture()?;
    verify_spec(&binding, &endpoint, &spec)?;
    // These paths intentionally do not exist on the client. Metadata identity
    // checks must not pretend that the client can inspect server sockets/keys.
    let baseline = serde_json::to_value(&spec)?;
    for (field, value) in [
        ("run_id", json!("run-other")),
        ("execution_identity", json!(carrier::object(b"other lock"))),
        ("dashboard_source", json!("native")),
        ("upstream", json!("127.0.0.1:18091")),
        ("upstream_unix", json!("/other/http.sock")),
        (
            "upstream_tls",
            json!({"server_name":"other","ca_file":"/other/ca"}),
        ),
    ] {
        let mut changed = baseline.clone();
        changed[field] = value;
        assert!(
            verify_spec(&binding, &endpoint, &serde_json::from_value(changed)?).is_err(),
            "accepted {field}"
        );
    }
    let mut changed = endpoint;
    changed.provider_configuration = carrier::object(b"another carrier owner");
    assert!(verify_spec(&binding, &changed, &spec).is_err());
    Ok(())
}

#[test]
fn atomic_ready_record_binds_exact_endpoint_bytes_and_rejects_overclaims() -> Result<()> {
    let (binding, _, _) = gateway_fixture()?;
    let root = tempfile::tempdir()?;
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))?;
    let path = root.path().join("ready.json");
    let endpoint = root.path().join("bridge/endpoint.json");
    let object = carrier::object(b"actual endpoint bytes fixture");
    let value = json!({"schema":"md-sim-carrier-dashboard-ready/1",
        "identity":binding.selection().identity(), "provider_configuration":binding.configuration(),
        "endpoint_record":object,"endpoint_path":endpoint,"firmware_readiness_established":false});
    save_json(&path, &value)?;
    let (_, raw) = ready_record(&path, &binding, &endpoint, &object)?;
    assert_eq!(raw, carrier::object(&fs::read(&path)?));
    for (field, changed) in [
        (
            "provider_configuration",
            json!(carrier::object(b"another provider")),
        ),
        (
            "endpoint_record",
            json!(carrier::object(b"another endpoint")),
        ),
        ("endpoint_path", json!("/other/endpoint.json")),
        ("firmware_readiness_established", json!(true)),
        ("unrecorded_override", json!(true)),
    ] {
        let mut bad = value.clone();
        bad[field] = changed;
        fs::write(&path, serde_json::to_vec(&bad)?)?;
        assert!(
            ready_record(&path, &binding, &endpoint, &object).is_err(),
            "accepted {field}"
        );
    }
    fs::write(&path, serde_json::to_vec(&value)?)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666))?;
    assert!(ready_record(&path, &binding, &endpoint, &object).is_err());
    Ok(())
}

#[test]
fn cancelled_start_never_reads_control_or_opens_gateway() -> Result<()> {
    let (binding, _, spec) = gateway_fixture()?;
    let root = tempfile::tempdir()?;
    let output = root.path().join("never-created");
    let error = CarrierGateway::start_with_cancel(
        Path::new("/absent/worker.sock"),
        binding,
        Path::new("/absent/ready.json"),
        Path::new("/absent/endpoint.json"),
        &spec,
        &output,
        &AtomicBool::new(true),
    )
    .err()
    .context("cancelled start unexpectedly succeeded")?;
    assert!(error.to_string().contains("cancelled"));
    assert!(!output.exists());
    Ok(())
}
