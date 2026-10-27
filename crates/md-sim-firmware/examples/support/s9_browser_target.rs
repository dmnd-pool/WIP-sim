//! Exact selected execution and origin for the native Boser browser suite.
use anyhow::{ensure, Context, Result};
use md_sim_access::carrier::{CarrierEndpoint, ProviderBinding, ProviderLease, ProviderState};
use md_sim_coordinator::LockedRun;
use md_sim_runtime::{dashboard::Binding, store::ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::net::SocketAddr;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub schema: String,
    pub selected_run: String,
    pub run: LockedRun,
    pub binding: Binding,
    pub http: SocketAddr,
    pub publication: Option<Value>,
}

/// A private-carrier selection is independent of runtime dashboard Binding.
/// The parent obtains publication through authenticated SSH before publishing
/// this record. Browser HTTP responses still require the actual selected run.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CarrierSelection {
    pub schema: String,
    pub selected_run: String,
    pub run: LockedRun,
    pub policy: ObjectId,
    pub policy_bytes: String,
    pub provider: ObjectId,
    pub provider_bytes: String,
    pub endpoint: CarrierEndpoint,
    pub http: SocketAddr,
    pub publication: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Selected {
    Binding(Box<Selection>),
    Carrier(Box<CarrierSelection>),
}

impl Selected {
    /// The selected process, not a separately supplied file, declares whether
    /// native and browser wall clocks must agree for this execution.
    pub fn clock_policy(&self) -> Result<Option<Value>> {
        let run = match self {
            Self::Binding(selected) => &selected.run,
            Self::Carrier(selected) => &selected.run,
        };
        let environment = &run
            .process
            .as_ref()
            .context("selected process absent")?
            .environment;
        let host = environment.get("MD_SIM_S9_HOST_REALTIME_CLOCK");
        let fixed = environment.get("MD_SIM_S9_WALL_CLOCK_EPOCH");
        ensure!(
            host.is_none() || fixed.is_none(),
            "conflicting selected clock policies"
        );
        if let Some(value) = host {
            ensure!(value == "1", "invalid selected host clock policy");
            return Ok(Some(serde_json::json!({
                "schema":"md-sim-native-browser-clock-check/1",
                "source":"execution-host-realtime",
                "selected_environment":{"MD_SIM_S9_HOST_REALTIME_CLOCK":"1"},
                "request_interval_tolerance_ms":5000
            })));
        }
        fixed.map(|epoch| {
            let epoch: u64 = epoch.parse().context("invalid selected fixed clock epoch")?;
            ensure!((1_231_006_505..=2_147_482_000).contains(&epoch), "selected fixed clock epoch outside original userspace range");
            Ok(serde_json::json!({"schema":"md-sim-private-wall-clock/1","epoch_unix_seconds":epoch}))
        }).transpose()
    }

    pub fn targets(&self) -> Result<Vec<Target>> {
        match self {
            Self::Binding(selected) => selected.targets(),
            Self::Carrier(selected) => selected.targets(),
        }
    }
}

/// Compare the native observation with the actual browser request interval.
/// Five seconds allow RTC and whole-second resolution plus serial delivery.
/// This is used by both the live producer and the retained-result verifier.
pub fn verify_clock_observation(observation: &Value, expected: &Value) -> Result<()> {
    ensure!(
        observation["clock_policy"] == *expected,
        "native browser clock policy differs from selected execution"
    );
    let samples = observation["telemetry"]
        .as_array()
        .context("native clock observations absent")?;
    ensure!(
        (2..=16).contains(&samples.len()),
        "native clock observation count differs"
    );
    for sample in samples {
        let uptime = &sample["body"]["data"]["bos"]["uptime"];
        let server = uptime["since"]
            .as_u64()
            .context("native boot epoch absent")?
            .checked_add(
                uptime["durationS"]
                    .as_u64()
                    .context("native uptime absent")?,
            )
            .context("native time overflow")?
            .checked_mul(1000)
            .context("native time overflow")?;
        let sent = sample["browser_sent_ms"]
            .as_u64()
            .context("browser request time absent")?;
        let received = sample["browser_received_ms"]
            .as_u64()
            .context("browser response time absent")?;
        ensure!(sent <= received && server >= sent.saturating_sub(5000) && server <= received.saturating_add(5000),
            "native wall clock is outside the browser request interval: server={server}, sent={sent}, received={received}");
    }
    Ok(())
}

impl CarrierSelection {
    pub fn targets(&self) -> Result<Vec<Target>> {
        ensure!(
            self.schema == "md-sim-selected-carrier-boser-browser/1",
            "carrier browser selection schema differs"
        );
        let token = self
            .selected_run
            .strip_prefix("run-")
            .context("carrier browser requires an actual worker run")?;
        let selected = md_sim_access::carrier::Selection::new(
            &self.run,
            self.policy.clone(),
            self.policy_bytes.as_bytes(),
            token,
        )?;
        let binding = ProviderBinding::from_config(
            selected,
            self.provider.clone(),
            self.provider_bytes.as_bytes(),
        )?;
        self.endpoint.verify(&binding)?;
        ensure!(
            self.endpoint.policy.source == md_sim_runtime::dashboard::Source::AdaptedNative
                && self.http == "127.0.0.1:18090".parse::<SocketAddr>()?,
            "carrier browser source or actual SSH tunnel differs"
        );
        let publication = &self.publication;
        let identity = binding.selection().identity();
        ensure!(
            publication["schema"] == "md-sim-selected-carrier-gateway/1"
                && publication["run_id"] == self.selected_run
                && publication["locked_run"] == serde_json::to_value(&identity.locked_run)?
                && publication["carrier_endpoint"] == serde_json::to_value(&self.endpoint)?
                && publication["endpoint"]["run_id"] == self.selected_run
                && publication["endpoint"]["execution_identity"]
                    == serde_json::to_value(&identity.locked_run)?
                && publication["endpoint"]["dashboard_source"] == "adapted-native"
                && publication["endpoint"]["upstream_unix"]
                    == serde_json::to_value(&self.endpoint.local_socket)?
                && publication["endpoint"]["url"] == "https://192.0.2.1:9443/"
                && publication["endpoint"]["browser_policy"] == "upgrade-insecure-requests",
            "carrier browser TLS publication differs from selected native endpoint"
        );
        let authorization = &publication["authorization"];
        let lease: ProviderLease = serde_json::from_value(authorization["provider"].clone())?;
        ensure!(
            lease.schema == "md-sim-carrier-provider-lease/1"
                && lease.identity == *identity
                && lease.configuration == self.provider
                && lease.sequence > 0
                && lease.state == ProviderState::Active
                && lease.challenge.len() == 64
                && lease
                    .challenge
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                && authorization["worker_lookup"]["found"] == true
                && authorization["worker_lookup"]["token"] == identity.token
                && authorization["worker_lookup"]["run_id"] == self.selected_run
                && authorization["worker_lookup"]["token_process_identity"]
                    == serde_json::to_value(&identity.process)?
                && authorization["worker_lookup"]["state"] == "booting"
                && authorization["worker_status"]["run_id"] == self.selected_run
                && authorization["worker_status"]["state"]["state"] == "booting"
                && authorization["worker_status"].get("dashboard") == Some(&Value::Null),
            "carrier browser publication lacks selected live worker/provider observation"
        );
        Ok(vec![
            Target {
                origin: format!("http://{}", self.http),
                selected_run: Some(self.selected_run.clone()),
                execution: Some(identity.locked_run.clone()),
                tls: false,
            },
            Target {
                origin: "https://192.0.2.1:9443".into(),
                selected_run: Some(self.selected_run.clone()),
                execution: Some(identity.locked_run.clone()),
                tls: true,
            },
        ])
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Target {
    pub origin: String,
    pub selected_run: Option<String>,
    pub execution: Option<ObjectId>,
    pub tls: bool,
}

impl Target {
    pub fn reference() -> Self {
        Self {
            origin: "http://10.0.2.15".into(),
            selected_run: None,
            execution: None,
            tls: false,
        }
    }

    pub fn check_response(&self, path: &str, response: &Value) -> Result<()> {
        ensure!(
            response["url"] == format!("{}{path}", self.origin),
            "native browser response left the selected origin or path"
        );
        if self.tls {
            ensure!(
                response["selected_run"].as_str() == self.selected_run.as_deref(),
                "native browser response selected another published run"
            );
        }
        Ok(())
    }
}

impl Selection {
    pub fn targets(&self) -> Result<Vec<Target>> {
        ensure!(
            self.schema == "md-sim-selected-boser-browser/1",
            "browser selection schema"
        );
        let process = self
            .run
            .process
            .as_ref()
            .context("selected process absent")?;
        process.validate()?;
        self.binding.verify(
            process
                .dashboard
                .as_ref()
                .context("selected dashboard absent")?,
            process,
        )?;
        ensure!(
            self.binding.source == md_sim_runtime::dashboard::Source::AdaptedNative,
            "selected browser requires adapted native Boser"
        );
        ensure!(
            self.http.ip().is_loopback() && self.http.port() >= 1024,
            "selected browser HTTP must use the local binding or SSH tunnel"
        );
        let object = self.run.object()?;
        let local = self.selected_run == format!("local-{}", object.sha256);
        ensure!(
            local
                || (self.selected_run.len() == 68
                    && self.selected_run.starts_with("run-")
                    && self.selected_run[4..]
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
            "selected browser run identity invalid"
        );
        if local {
            ensure!(
                self.publication.is_none() && self.http == self.binding.listen,
                "local browser endpoint differs from the selected binding"
            );
        } else {
            ensure!(
                self.http == "127.0.0.1:18090".parse::<SocketAddr>()?,
                "selected browser SSH endpoint differs"
            );
        }
        let mut targets = vec![Target {
            origin: format!("http://{}", self.http),
            selected_run: Some(self.selected_run.clone()),
            execution: Some(object.clone()),
            tls: false,
        }];
        if !local {
            let publication = self
                .publication
                .as_ref()
                .context("selected TLS publication absent")?;
            ensure!(
                publication["run_id"] == self.selected_run
                    && publication["locked_run"] == serde_json::to_value(&object)?
                    && publication["binding"] == serde_json::to_value(&self.binding)?
                    && publication["endpoint"]["upstream_unix"]
                        == serde_json::to_value(&self.binding.local_socket)?
                    && publication["endpoint"]["url"] == "https://192.0.2.1:9443/",
                "selected browser publication identity or endpoint differs"
            );
            ensure!(
                publication["endpoint"]["browser_policy"] == "upgrade-insecure-requests",
                "selected Boser browser adaptation absent"
            );
            targets.push(Target {
                origin: "https://192.0.2.1:9443".into(),
                selected_run: Some(self.selected_run.clone()),
                execution: Some(object),
                tls: true,
            });
        }
        Ok(targets)
    }
}

/// Shared live and retained-result oracle. A top-level passed flag cannot
/// substitute for selected origin coverage, actual WebSockets or live mining.
#[allow(dead_code)] // The producer also imports these selection types; parents run this oracle.
pub fn verify_report(report: &Value, selection: &Value, driver: &ObjectId) -> Result<()> {
    let selected: Selected = serde_json::from_value(selection.clone())?;
    let targets = selected.targets()?;
    let clock_policy = selected.clock_policy()?;
    ensure!(
        report["schema"] == "md-sim-original-s9-browser/2"
            && report["passed"] == true
            && report["error"].is_null()
            && report["helper"] == serde_json::to_value(driver)?
            && report["selection"] == *selection
            && report["fresh_profile"] == true
            && report["separate_uid"] == 1001
            && report["cleanup"]["empty"] == true
            && report["isolation_cleanup"]["empty"] == true,
        "native browser did not pass for the selected process and helper"
    );
    let origins = report["origins"]
        .as_array()
        .context("browser origins absent")?;
    ensure!(
        origins.len() == targets.len(),
        "selected browser origin coverage differs"
    );
    for (origin, target) in origins.iter().zip(targets) {
        if let Some(policy) = &clock_policy {
            verify_clock_observation(origin, policy)?;
        } else {
            ensure!(
                origin["clock_policy"].is_null(),
                "undeclared selected browser clock policy"
            );
        }
        ensure!(
            origin["origin"] == target.origin
                && origin["target"] == serde_json::to_value(&target)?
                && origin["passed"] == true
                && origin["error"].is_null()
                && origin["isolation"]["passed"] == true
                && origin["websocket"]["reason"] == "two samples"
                && origin["native_view_verified"]["boards"] == 3
                && origin["native_view_verified"]["private_pool_rows_with_accepted_work"] == 1
                && origin["live_mining_telemetry"]
                    .as_array()
                    .is_some_and(|a| a.len() == 2),
            "selected browser native origin checks failed"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_coordinator::{ExecutionScope, SuiteIdentity};
    use md_sim_runtime::{
        dashboard::{Source, Spec},
        isolation::{Limits, Mount, ProcessSpec},
    };
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};

    fn selected() -> Result<Selection> {
        let object = ObjectId {
            sha256: "a".repeat(64),
            size: 1,
        };
        let process = ProcessSpec {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            schema_version: "2.0".into(),
            argv: vec!["/program".into()],
            inputs: vec![Mount {
                guest_path: "/program".into(),
                object: object.clone(),
            }],
            stdin_object: None,
            environment: BTreeMap::new(),
            loopback: true,
            dashboard: Some(Spec {
                source: Source::AdaptedNative,
                guest_port: 18080,
                listen_port: 0,
            }),
            limits: Limits {
                memory_bytes: 512 << 20,
                processes: 32,
                ..Limits::default()
            },
        };
        let bytes = serde_json::to_vec(&process)?;
        let binding = Binding {
            process_identity: ObjectId {
                sha256: md_sim_firmware::prepare::hash_bytes(&bytes),
                size: bytes.len() as u64,
            },
            source: Source::AdaptedNative,
            listen: "127.0.0.1:18088".parse()?,
            local_socket: Some("/tmp/md-sim-dashboard-browser-test/http.sock".into()),
            guest_port: 18080,
            connection_limit: 8,
            connection_byte_limit: 64 << 20,
            run_byte_limit: 256 << 20,
            firmware_readiness_established: false,
        };
        let run = LockedRun {
            name: "browser-identity-regression".into(),
            scope: ExecutionScope::ProcessPrimitive {
                description: "metadata test only".into(),
            },
            process: Some(process),
            suite: SuiteIdentity {
                id: "test".into(),
                version: "1".into(),
                suite: object.clone(),
                verifier: object.clone(),
                required_gates: BTreeSet::from(["identity".into()]),
            },
            inputs: BTreeMap::from([("input".into(), object.clone())]),
            backend_tools: BTreeMap::from([
                ("bubblewrap".into(), object.clone()),
                ("unshare".into(), object),
            ]),
            policies: json!({"cap":1000}),
            capability_limits: vec!["not firmware execution".into()],
        };
        let selected_run = format!("run-{}", "b".repeat(64));
        let publication = json!({"run_id":selected_run,"locked_run":run.object()?,"binding":binding,
            "endpoint":{"url":"https://192.0.2.1:9443/","upstream_unix":binding.local_socket,"browser_policy":"upgrade-insecure-requests"}});
        Ok(Selection {
            schema: "md-sim-selected-boser-browser/1".into(),
            selected_run,
            run,
            binding,
            http: "127.0.0.1:18090".parse()?,
            publication: Some(publication),
        })
    }

    fn carrier_selected() -> Result<CarrierSelection> {
        carrier_selected_with_clock(false)
    }

    fn carrier_selected_with_clock(host_clock: bool) -> Result<CarrierSelection> {
        use md_sim_access::carrier::{Policy, Selection as Carrier};
        use md_sim_runtime::private_channel;
        let mut run = selected()?.run;
        let process = run.process.as_mut().unwrap();
        if host_clock {
            process
                .environment
                .insert("MD_SIM_S9_HOST_REALTIME_CLOCK".into(), "1".into());
        }
        let support = process.inputs[0].object.clone();
        process.dashboard = None;
        process.private_channel = Some(private_channel::Spec {
            protocol: private_channel::Protocol::QemuEthernetV1,
            guest_fd: 3,
            max_frame_bytes: 1514,
            max_wire_bytes: 65536,
            max_packets: 100,
        });
        let policy_value = Policy {
            schema: "md-sim-carrier-dashboard-policy/1".into(),
            source: Source::AdaptedNative,
            support_identity: support.clone(),
            destination: "10.0.2.15:80".parse()?,
            listen_port: 0,
            connection_limit: 8,
            connection_attempt_limit: 128,
            connection_bytes: 1 << 20,
            run_bytes: 8 << 20,
            connect_milliseconds: 1000,
            idle_seconds: 10,
            wall_seconds: 360,
            lease_milliseconds: 10000,
        };
        let policy_bytes = serde_json::to_string_pretty(&policy_value)?;
        let object = |bytes: &str| ObjectId {
            sha256: md_sim_firmware::prepare::hash_bytes(bytes.as_bytes()),
            size: bytes.len() as u64,
        };
        let policy = object(&policy_bytes);
        run.inputs.insert("carrier-policy".into(), policy.clone());
        run.policies["carrier_dashboard"] = json!(policy);
        let token = "b".repeat(64);
        let selected = Carrier::new(&run, policy.clone(), policy_bytes.as_bytes(), &token)?;
        let provider_bytes = serde_json::to_string_pretty(
            &json!({"schema":"md-sim-worker-private-carrier/1",
            "process":run.process,"support_identity":support,"handoff_seconds":30,"wall_seconds":360,
            "lease":{"locked_run":run,"policy":policy}}),
        )?;
        let provider = object(&provider_bytes);
        let endpoint = CarrierEndpoint {
            schema: "md-sim-carrier-dashboard-endpoint/1".into(),
            identity: selected.identity().clone(),
            provider_configuration: provider.clone(),
            policy: policy_value,
            listen: "127.0.0.1:18088".parse()?,
            local_socket: "/work/access/bridge/http.sock".into(),
            firmware_readiness_established: false,
        };
        let identity = &endpoint.identity;
        let publication = json!({"schema":"md-sim-selected-carrier-gateway/1","run_id":identity.run_id,
            "locked_run":identity.locked_run,"carrier_endpoint":endpoint,
            "endpoint":{"run_id":identity.run_id,"execution_identity":identity.locked_run,"dashboard_source":"adapted-native",
                "upstream_unix":endpoint.local_socket,"url":"https://192.0.2.1:9443/","browser_policy":"upgrade-insecure-requests"},
            "authorization":{"provider":{"schema":"md-sim-carrier-provider-lease/1","identity":identity,
                "configuration":provider,"sequence":5,"state":"active","challenge":"c".repeat(64)},
                "worker_lookup":{"found":true,"token":token,"run_id":identity.run_id,"token_process_identity":identity.process,"state":"booting"},
                "worker_status":{"run_id":identity.run_id,"state":{"state":"booting"},"dashboard":null}}});
        Ok(CarrierSelection {
            schema: "md-sim-selected-carrier-boser-browser/1".into(),
            selected_run: identity.run_id.clone(),
            run,
            policy,
            policy_bytes,
            provider,
            provider_bytes,
            endpoint,
            http: "127.0.0.1:18090".parse()?,
            publication,
        })
    }

    #[test]
    fn carrier_browser_requires_full_original_identity_and_actual_publication_contract(
    ) -> Result<()> {
        let valid = carrier_selected()?;
        assert_eq!(valid.targets()?.len(), 2);
        let mut changed = valid.clone();
        changed.policy_bytes.push(' ');
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.provider_bytes.push(' ');
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.run.process.as_mut().unwrap().limits.wall_seconds += 1;
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.endpoint.listen.set_port(18089);
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.publication["authorization"]["provider"]["state"] = json!("closed");
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.publication["authorization"]["worker_lookup"]["token_process_identity"]["size"] =
            json!(2);
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.publication["endpoint"]["url"] = json!("https://192.0.2.1:8443/");
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.publication["endpoint"]["browser_policy"] = json!("preserve");
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.http.set_port(18091);
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.endpoint.firmware_readiness_established = true;
        assert!(changed.targets().is_err());
        for value in [
            serde_json::to_value(selected()?)?,
            serde_json::to_value(valid)?,
        ] {
            let selected: Selected = serde_json::from_value(value.clone())?;
            assert_eq!(selected.targets()?.len(), 2);
            assert_eq!(serde_json::to_value(selected)?, value);
        }
        Ok(())
    }

    #[test]
    fn retained_browser_pass_requires_both_origins_and_actual_observations() -> Result<()> {
        let selected = carrier_selected_with_clock(true)?;
        let policy = Selected::Carrier(Box::new(selected.clone()))
            .clock_policy()?
            .unwrap();
        let driver = ObjectId {
            sha256: "f".repeat(64),
            size: 10,
        };
        let selection = serde_json::to_value(&selected)?;
        let origins = selected.targets()?.into_iter().map(|target|json!({"origin":target.origin,
            "target":target,"passed":true,"error":null,"isolation":{"passed":true},
            "websocket":{"reason":"two samples"},"native_view_verified":{"boards":3,"private_pool_rows_with_accepted_work":1},
            "live_mining_telemetry":[{},{}],"clock_policy":policy,"telemetry":clock_samples()})).collect::<Vec<_>>();
        let report = json!({"schema":"md-sim-original-s9-browser/2","passed":true,"error":null,
            "helper":driver,"selection":selection,"fresh_profile":true,"separate_uid":1001,
            "cleanup":{"empty":true},"isolation_cleanup":{"empty":true},"origins":origins});
        verify_report(&report, &selection, &driver)?;
        for (pointer, value) in [
            ("/origins/0/websocket/reason", json!("closed")),
            ("/origins/1/live_mining_telemetry", json!([])),
            ("/origins/1/native_view_verified/boards", json!(2)),
            ("/origins/0/isolation/passed", json!(false)),
            ("/cleanup/empty", json!(false)),
            ("/selection/provider/size", json!(1)),
            ("/origins/1/origin", json!("https://another.invalid")),
            ("/origins/0/clock_policy", Value::Null),
            (
                "/origins/1/clock_policy/request_interval_tolerance_ms",
                json!(6000),
            ),
            (
                "/origins/1/telemetry/1/body/data/bos/uptime/since",
                json!(1_709_000_000),
            ),
        ] {
            let mut changed = report.clone();
            *changed.pointer_mut(pointer).context("test pointer")? = value;
            assert!(
                verify_report(&changed, &selection, &driver).is_err(),
                "{pointer}"
            );
        }
        let mut missing = report.clone();
        missing["origins"].as_array_mut().unwrap().pop();
        assert!(verify_report(&missing, &selection, &driver).is_err());
        Ok(())
    }

    fn clock_samples() -> Value {
        json!([
            {"body":{"data":{"bos":{"uptime":{"since":1_709_164_800u64,"durationS":30}}}},
                "browser_sent_ms":1_709_164_830_000u64,"browser_received_ms":1_709_164_830_900u64},
            {"body":{"data":{"bos":{"uptime":{"since":1_709_164_800u64,"durationS":32}}}},
                "browser_sent_ms":1_709_164_832_000u64,"browser_received_ms":1_709_164_832_900u64}
        ])
    }

    #[test]
    fn native_clock_interval_rejects_skew_missing_data_and_overflow() -> Result<()> {
        let policy = json!({"schema":"test-clock"});
        let observation = json!({"clock_policy":policy,"telemetry":clock_samples()});
        verify_clock_observation(&observation, &policy)?;
        for boundary in [1_709_164_827_000u64, 1_709_164_837_000] {
            let mut at_bound = observation.clone();
            at_bound["telemetry"][1]["browser_sent_ms"] = json!(boundary);
            at_bound["telemetry"][1]["browser_received_ms"] = json!(boundary);
            verify_clock_observation(&at_bound, &policy)?;
        }
        for outside in [1_709_164_826_999u64, 1_709_164_837_001] {
            let mut skewed = observation.clone();
            skewed["telemetry"][1]["browser_sent_ms"] = json!(outside);
            skewed["telemetry"][1]["browser_received_ms"] = json!(outside);
            assert!(verify_clock_observation(&skewed, &policy).is_err());
        }
        for (pointer, value) in [
            ("/telemetry/1/browser_sent_ms", json!(1_709_164_837_001u64)),
            (
                "/telemetry/1/browser_received_ms",
                json!(1_709_164_826_999u64),
            ),
            ("/telemetry/1/browser_sent_ms", Value::Null),
            ("/telemetry/1/body/data/bos/uptime/since", json!(u64::MAX)),
            ("/telemetry/1/body/data/bos/uptime/durationS", json!(-1)),
            ("/clock_policy", Value::Null),
        ] {
            let mut changed = observation.clone();
            *changed.pointer_mut(pointer).context("clock test pointer")? = value;
            assert!(
                verify_clock_observation(&changed, &policy).is_err(),
                "{pointer}"
            );
        }
        let mut single = observation;
        single["telemetry"].as_array_mut().unwrap().pop();
        assert!(verify_clock_observation(&single, &policy).is_err());
        Ok(())
    }

    #[test]
    fn selected_clock_is_derived_from_locked_process_and_rejects_conflicts() -> Result<()> {
        let mut selection = selected()?;
        assert!(Selected::Binding(Box::new(selection.clone()))
            .clock_policy()?
            .is_none());
        let environment = &mut selection.run.process.as_mut().unwrap().environment;
        environment.insert("MD_SIM_S9_HOST_REALTIME_CLOCK".into(), "0".into());
        assert!(Selected::Binding(Box::new(selection.clone()))
            .clock_policy()
            .is_err());
        selection
            .run
            .process
            .as_mut()
            .unwrap()
            .environment
            .insert("MD_SIM_S9_HOST_REALTIME_CLOCK".into(), "1".into());
        assert_eq!(
            Selected::Binding(Box::new(selection.clone()))
                .clock_policy()?
                .unwrap()["source"],
            "execution-host-realtime"
        );
        selection
            .run
            .process
            .as_mut()
            .unwrap()
            .environment
            .insert("MD_SIM_S9_WALL_CLOCK_EPOCH".into(), "1709164800".into());
        assert!(Selected::Binding(Box::new(selection.clone()))
            .clock_policy()
            .is_err());
        selection
            .run
            .process
            .as_mut()
            .unwrap()
            .environment
            .remove("MD_SIM_S9_HOST_REALTIME_CLOCK");
        assert_eq!(
            Selected::Binding(Box::new(selection))
                .clock_policy()?
                .unwrap()["epoch_unix_seconds"],
            1_709_164_800u64
        );
        Ok(())
    }

    #[test]
    fn selected_browser_rejects_changed_execution_binding_and_publication() -> Result<()> {
        let valid = selected()?;
        assert_eq!(valid.targets()?.len(), 2);
        let mut changed = valid.clone();
        changed
            .run
            .process
            .as_mut()
            .unwrap()
            .argv
            .push("changed".into());
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.run.policies["cap"] = json!(1001);
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.binding.listen = "127.0.0.1:18089".parse()?;
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.publication.as_mut().unwrap()["run_id"] = json!(format!("run-{}", "c".repeat(64)));
        assert!(changed.targets().is_err());
        let mut changed = valid.clone();
        changed.publication.as_mut().unwrap()["endpoint"]["url"] = json!("https://192.0.2.1:9444/");
        assert!(changed.targets().is_err());
        let mut local = valid;
        local.selected_run = format!("local-{}", local.run.hash()?);
        local.publication = None;
        local.http = local.binding.listen;
        assert_eq!(local.targets()?.len(), 1);
        local.http = "127.0.0.1:18089".parse()?;
        assert!(local.targets().is_err());
        Ok(())
    }
    #[test]
    fn responses_require_the_selected_path_and_published_run() -> Result<()> {
        let target = Target {
            origin: "https://192.0.2.1:9443".into(),
            selected_run: Some(format!("run-{}", "a".repeat(64))),
            execution: None,
            tls: true,
        };
        let valid =
            json!({"url":"https://192.0.2.1:9443/graphql", "selected_run":target.selected_run});
        target.check_response("/graphql", &valid)?;
        for url in [
            "http://192.0.2.1:9443/graphql",
            "https://192.0.2.1:9444/graphql",
            "https://192.0.2.2:9443/graphql",
            "https://192.0.2.1:9443/graphql/",
            "https://192.0.2.1:9443/login",
        ] {
            let mut changed = valid.clone();
            changed["url"] = json!(url);
            assert!(target.check_response("/graphql", &changed).is_err());
        }
        for id in [
            Value::Null,
            json!(""),
            json!(format!("run-{}", "b".repeat(64))),
            json!(format!("run-{}, run-{}", "a".repeat(64), "a".repeat(64))),
        ] {
            let mut changed = valid.clone();
            changed["selected_run"] = id;
            assert!(target.check_response("/graphql", &changed).is_err());
        }
        Ok(())
    }
}
