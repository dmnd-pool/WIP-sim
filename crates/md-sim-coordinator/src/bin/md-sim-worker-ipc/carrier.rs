//! One trusted, preconnected support-VM carrier. The runtime supplies a new
//! endpoint for the exact selected job. This module never passes the installed
//! support descriptor into that job.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::LockedRun;
use md_sim_runtime::{
    isolation::ProcessSpec,
    private_channel,
    store::ObjectId,
    worker::{process_identity, PrivateChannelBinding, PrivateChannelLeaseSpec, WorkerRootLease},
};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    schema: String,
    process: ProcessSpec,
    support_identity: ObjectId,
    handoff_seconds: u64,
    wall_seconds: u64,
    #[serde(default)]
    lease: Option<LeaseConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LeaseConfig {
    locked_run: LockedRun,
    policy: ObjectId,
}

impl Config {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == "md-sim-worker-private-carrier/1",
            "unknown carrier configuration"
        );
        self.process.validate()?;
        self.support_identity.validate()?;
        ensure!(
            self.process.private_channel.is_some() && self.process.stdin_stream.is_none(),
            "carrier policy absent or unsupported streamed job input"
        );
        ensure!(
            self.process
                .inputs
                .iter()
                .any(|mount| mount.object == self.support_identity),
            "support identity is not a captured process input"
        );
        ensure!(
            (1..=86400).contains(&self.wall_seconds)
                && self.wall_seconds >= self.process.limits.wall_seconds
                && (1..=300).contains(&self.handoff_seconds),
            "provider lifetime does not cover the selected process budget"
        );
        if let Some(lease) = &self.lease {
            lease.locked_run.validate()?;
            lease.policy.validate()?;
            ensure!(
                lease.policy.size > 0
                    && serde_json::to_value(
                        lease
                            .locked_run
                            .process
                            .as_ref()
                            .context("provider lock process absent")?
                    )? == serde_json::to_value(&self.process)?
                    && lease
                        .locked_run
                        .inputs
                        .values()
                        .any(|input| *input == lease.policy)
                    && lease.locked_run.policies["carrier_dashboard"]
                        == serde_json::to_value(&lease.policy)?,
                "provider full lock process or captured dashboard policy differs"
            );
        }
        Ok(())
    }
}

pub struct Provider {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<Value>>>,
    report: PathBuf,
    root: WorkerRootLease,
}

fn publish(path: &Path, value: &Value) -> Result<()> {
    let pending = path.with_extension("pending");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&pending)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    // Both paths are in the leased root. A pre-existing receipt is never
    // replaced. A crash can leave the pending file for later inspection.
    fs::hard_link(&pending, path).context("publish provider receipt without overwriting")?;
    fs::remove_file(pending)?;
    File::open(path.parent().context("provider report parent")?)?.sync_all()?;
    Ok(())
}

impl Provider {
    pub fn start(
        config_path: &Path,
        peer: File,
        root: &Path,
    ) -> Result<(Self, PrivateChannelBinding)> {
        let mut bytes = Vec::new();
        OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(config_path)?
            .take((4 << 20) + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 4 << 20, "carrier configuration is too large");
        let config: Config = serde_json::from_slice(&bytes)?;
        config.validate()?;
        let identity = process_identity(&config.process)?;
        let policy = config
            .process
            .private_channel
            .clone()
            .context("carrier policy absent")?;
        let config_identity = ObjectId {
            sha256: hex::encode(Sha256::digest(&bytes)),
            size: bytes.len() as u64,
        };
        // A carrier is consumed once. It cannot attach to another worker's
        // active root or overwrite a previous provider's evidence.
        fs::create_dir(root).context("private carrier requires a new worker root")?;
        let root = WorkerRootLease::acquire(root)?;
        let report = root.path().join("private-carrier.json");
        ensure!(
            !report.exists(),
            "provider report already exists; use a new worker root"
        );
        let (sender, receiver) = mpsc::sync_channel(1);
        let binding = PrivateChannelBinding::new(identity.clone(), sender)?;
        let (binding, lifecycle) = if let Some(lease) = &config.lease {
            let (binding, lifecycle) = binding.with_lease(PrivateChannelLeaseSpec {
                locked_run: lease.locked_run.object()?,
                process: identity.clone(),
                policy: lease.policy.clone(),
                support: config.support_identity.clone(),
                configuration: config_identity.clone(),
            })?;
            (binding, Some(lifecycle))
        } else {
            (binding, None)
        };
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let join = thread::spawn(move || {
            // This guard belongs to the actual relay thread. Any early error
            // or panic leaves the monitor failed; job Status is never used.
            let mut lifecycle = lifecycle;
            let result = (|| -> Result<Value> {
                let began = Instant::now();
                let handoff_budget = Duration::from_secs(config.handoff_seconds);
                let endpoint = loop {
                    if thread_stop.load(Ordering::Acquire) {
                        break None;
                    }
                    ensure!(
                        began.elapsed() < handoff_budget,
                        "provider deadline before job handoff"
                    );
                    match receiver.recv_timeout(Duration::from_millis(50)) {
                        Ok(file) => break Some(file),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break None,
                    }
                };
                let handoff_ms = began.elapsed().as_millis();
                let receipt = if let Some(endpoint) = endpoint {
                    if let Some(lifecycle) = &lifecycle {
                        lifecycle.active()?;
                    }
                    Some(private_channel::relay_provider(
                        endpoint,
                        peer,
                        policy,
                        thread_stop,
                        config.wall_seconds,
                    )?)
                } else {
                    None
                };
                Ok(json!({"schema":"md-sim-worker-private-carrier-result/1",
                "configuration":config_identity,"process":identity,"support_identity":config.support_identity,
                "receipt":receipt,"handoff_ms":handoff_ms,"handoff_seconds":config.handoff_seconds,
                "relay_wall_seconds":config.wall_seconds,"elapsed_ms":began.elapsed().as_millis(),
                "scope":"Trusted parent supplied a private support endpoint. Runtime created the separate isolated-job endpoint. Provenance and support-VM routing require the parent run evidence."}))
            })();
            if let Some(lifecycle) = &mut lifecycle {
                lifecycle.finish(match &result {
                    Err(_) => true,
                    Ok(evidence) => {
                        !evidence["receipt"]["error"].is_null()
                            || evidence["receipt"]["end"] == "timed-out"
                    }
                });
            }
            result
        });
        Ok((
            Self {
                stop,
                join: Some(join),
                report,
                root,
            },
            binding,
        ))
    }

    pub fn root(&self) -> &WorkerRootLease {
        &self.root
    }

    pub fn finish(mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        let result = self
            .join
            .take()
            .expect("owned provider thread")
            .join()
            .map_err(|_| anyhow::anyhow!("provider thread panicked"))
            .and_then(|result| result);
        match result {
            Ok(evidence) => {
                publish(&self.report, &evidence)?;
                ensure!(
                    evidence["receipt"]["error"].is_null()
                        && evidence["receipt"]["end"] != "timed-out",
                    "support carrier failed; see provider report"
                );
                Ok(())
            }
            Err(error) => {
                publish(
                    &self.report,
                    &json!({"failed":true,"error":format!("{error:#}")}),
                )?;
                Err(error)
            }
        }
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_runtime::isolation::{Limits, Mount};

    fn config() -> Config {
        let support = ObjectId {
            sha256: "a".repeat(64),
            size: 123,
        };
        Config {
            schema: "md-sim-worker-private-carrier/1".into(),
            support_identity: support.clone(),
            handoff_seconds: 30,
            wall_seconds: 20,
            lease: None,
            process: ProcessSpec {
                schema_version: "2.0".into(),
                namespace_uid: 0,
                argv: vec!["/fixture".into()],
                inputs: vec![Mount {
                    object: support,
                    guest_path: "/fixture".into(),
                }],
                environment: Default::default(),
                stdin_object: None,
                stdin_stream: None,
                dashboard: None,
                loopback: false,
                private_channel: Some(private_channel::Spec {
                    protocol: private_channel::Protocol::QemuEthernetV1,
                    guest_fd: 3,
                    max_frame_bytes: 1514,
                    max_wire_bytes: 65536,
                    max_packets: 100,
                }),
                limits: Limits {
                    wall_seconds: 10,
                    cpu_seconds: 5,
                    memory_bytes: 64 << 20,
                    processes: 16,
                    file_bytes: 1 << 20,
                    trace_bytes: 65536,
                    tmp_bytes: 16 << 20,
                },
            },
        }
    }

    #[test]
    fn provider_requires_its_captured_peer_and_a_finite_lifetime() {
        config().validate().unwrap();
        let mut missing = config();
        missing.support_identity.sha256 = "b".repeat(64);
        assert!(missing.validate().is_err());
        for seconds in [0, 9, 86401] {
            let mut short = config();
            short.wall_seconds = seconds;
            assert!(short.validate().is_err());
        }
        let mut undeclared = config();
        undeclared.process.private_channel = None;
        assert!(undeclared.validate().is_err());
        for seconds in [0, 301] {
            let mut unbounded = config();
            unbounded.handoff_seconds = seconds;
            assert!(unbounded.validate().is_err());
        }
    }

    fn leased_config() -> Config {
        use md_sim_coordinator::{ExecutionScope, SuiteIdentity};
        let mut config = config();
        let object = config.support_identity.clone();
        let run = LockedRun {
            name: "provider-configuration-contract".into(),
            scope: ExecutionScope::ProcessPrimitive {
                description: "configuration validation only".into(),
            },
            process: Some(config.process.clone()),
            suite: SuiteIdentity {
                id: "provider".into(),
                version: "1".into(),
                suite: object.clone(),
                verifier: object.clone(),
                required_gates: ["configuration".into()].into(),
            },
            inputs: [("captured".into(), object.clone())].into(),
            backend_tools: [
                ("bubblewrap".into(), object.clone()),
                ("unshare".into(), object.clone()),
            ]
            .into(),
            policies: json!({"carrier_dashboard":object}),
            capability_limits: vec!["No execution evidence".into()],
        };
        config.lease = Some(LeaseConfig {
            locked_run: run,
            policy: object,
        });
        config
    }

    #[test]
    fn provider_lease_requires_the_full_locked_process_and_policy_membership() {
        leased_config().validate().unwrap();
        let mut changed = leased_config();
        changed
            .lease
            .as_mut()
            .unwrap()
            .locked_run
            .process
            .as_mut()
            .unwrap()
            .environment
            .insert("changed".into(), "1".into());
        assert!(changed.validate().is_err());
        let mut missing = leased_config();
        missing.lease.as_mut().unwrap().policy.sha256 = "b".repeat(64);
        assert!(missing.validate().is_err());
        let mut unselected = leased_config();
        unselected.lease.as_mut().unwrap().locked_run.policies = json!({});
        assert!(unselected.validate().is_err());
    }

    #[test]
    fn completed_provider_report_never_overwrites_previous_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let report = directory.path().join("provider.json");
        publish(&report, &json!({"first":true})).unwrap();
        let before = fs::read(&report).unwrap();
        assert!(publish(&report, &json!({"second":true})).is_err());
        assert_eq!(fs::read(&report).unwrap(), before);
        assert!(directory.path().join("provider.pending").exists());
    }

    #[test]
    fn actual_provider_thread_panic_is_retained_before_root_release() {
        let directory = tempfile::tempdir().unwrap();
        let root = WorkerRootLease::acquire(directory.path()).unwrap();
        let report = root.path().join("private-carrier.json");
        let provider = Provider {
            stop: Arc::new(AtomicBool::new(false)),
            join: Some(thread::spawn(|| panic!("injected provider failure"))),
            report: report.clone(),
            root,
        };
        assert!(WorkerRootLease::acquire(directory.path()).is_err());
        assert!(provider.finish().is_err());
        let retained: Value = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
        assert_eq!(retained["failed"], true);
        assert!(retained["error"].as_str().unwrap().contains("panicked"));
        let lease = WorkerRootLease::acquire(directory.path()).unwrap();
        assert!(lease.path().join("private-carrier.json").is_file());
    }

    #[test]
    fn an_actual_relay_timeout_is_retained_and_fails_the_provider() {
        use std::os::{fd::OwnedFd, unix::net::UnixStream};
        let directory = tempfile::tempdir().unwrap();
        let root = WorkerRootLease::acquire(directory.path()).unwrap();
        let report = root.path().join("private-carrier.json");
        let (guest, _guest_hold) = UnixStream::pair().unwrap();
        let (peer, _peer_hold) = UnixStream::pair().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let policy = config().process.private_channel.unwrap();
        let join = thread::spawn(move || {
            let receipt = private_channel::relay_provider(
                File::from(OwnedFd::from(guest)),
                File::from(OwnedFd::from(peer)),
                policy,
                thread_stop,
                1,
            )?;
            Ok(json!({"receipt":receipt}))
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while !join.is_finished() {
            assert!(
                Instant::now() < deadline,
                "provider relay ignored its deadline"
            );
            thread::sleep(Duration::from_millis(20));
        }
        let provider = Provider {
            stop,
            join: Some(join),
            report: report.clone(),
            root,
        };
        assert!(provider.finish().is_err());
        let retained: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
        assert_eq!(retained["receipt"]["end"], "timed-out");
    }
}
