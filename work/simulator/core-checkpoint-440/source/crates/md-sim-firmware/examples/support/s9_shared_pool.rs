//! Selected external pool contract for the actual archived S9 runner.
//! This module observes the miner. Independent pool evidence proves mining.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    private_channel::{Protocol, Spec, GUEST_FD},
    store::{hash_file, ObjectId},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs::File,
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{fs::FileTypeExt, process::CommandExt},
    },
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

pub const ENVIRONMENT: &str = "MD_SIM_S9_SHARED_POOL";
pub const GUEST_MANIFEST: &str = "/shared-pool.json";
pub const MANIFEST_BYTES: u64 = 8192;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: String,
    pub pool_verifier: ObjectId,
    pub support_runner: ObjectId,
    pub support_backend: ObjectId,
    pub support_kernel: ObjectId,
    pub network_contract: String,
    pub pool_endpoint: String,
    pub miner_address: String,
    pub readiness_seconds: u64,
    pub observation_margin_seconds: u64,
    pub carrier: Spec,
}

impl Manifest {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == "md-sim-s9-shared-pool/1"
                && self.network_contract == "md-sim-s9-server-namespace/1"
                && self.pool_endpoint == "10.0.2.100:3333"
                && self.miner_address == "10.0.2.15/24",
            "shared pool contract differs"
        );
        for object in [
            &self.pool_verifier,
            &self.support_runner,
            &self.support_backend,
            &self.support_kernel,
        ] {
            object.validate()?;
            ensure!(object.size > 0, "empty shared support component");
        }
        self.carrier.validate()?;
        ensure!(
            self.carrier.protocol == Protocol::QemuEthernetV1
                && self.carrier.max_wire_bytes <= 128 << 20
                && self.carrier.max_packets <= 1 << 20,
            "shared carrier exceeds its execution bounds"
        );
        ensure!(
            (1..=30).contains(&self.readiness_seconds) && self.observation_margin_seconds == 5,
            "shared pool timing contract differs"
        );
        Ok(())
    }

    pub fn observation_seconds(&self, pool_seconds: u64, wall_seconds: u64) -> Result<u64> {
        self.validate()?;
        ensure!(
            (20..=300).contains(&pool_seconds),
            "shared pool window bound"
        );
        let seconds = pool_seconds + self.observation_margin_seconds;
        ensure!(
            seconds < wall_seconds && wall_seconds <= 360,
            "shared observation must fit inside the unchanged miner wall budget"
        );
        Ok(seconds)
    }
}

pub fn read(path: &Path) -> Result<(Manifest, Vec<u8>)> {
    let file = File::open(path)?;
    ensure!(file.metadata()?.is_file(), "shared manifest is not a file");
    let mut bytes = Vec::new();
    file.take(MANIFEST_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MANIFEST_BYTES,
        "shared manifest size bound"
    );
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    manifest.validate()?;
    Ok((manifest, bytes))
}

pub fn enabled() -> Result<bool> {
    match std::env::var(ENVIRONMENT) {
        Err(std::env::VarError::NotPresent) => Ok(false),
        Ok(value) if value == "1" => Ok(true),
        _ => anyhow::bail!("invalid shared pool execution flag"),
    }
}

/// Ownership is acquired before other runner code can reuse descriptor3.
/// The endpoint cannot be supplied by a path, environment FD, or CLI number.
pub fn take_endpoint() -> Result<File> {
    ensure!(
        unsafe { libc::fcntl(GUEST_FD, libc::F_GETFD) } >= 0,
        "runtime shared carrier is absent"
    );
    // The caller uses this once, at entry, only for the declared runtime mode.
    let endpoint = unsafe { File::from_raw_fd(GUEST_FD) };
    protect(&endpoint)?;
    Ok(endpoint)
}

fn protect(endpoint: &File) -> Result<()> {
    ensure!(
        endpoint.metadata()?.file_type().is_socket(),
        "shared carrier is not a socket"
    );
    ensure!(
        unsafe { libc::fcntl(endpoint.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
        "cannot protect shared carrier"
    );
    Ok(())
}

pub struct Observation {
    endpoint: Option<File>,
    started: Instant,
    seconds: u64,
}

impl Observation {
    pub fn attach(command: &mut Command, endpoint: File, pool_seconds: u64) -> Result<Self> {
        protect(&endpoint)?;
        let (manifest, _) = read(Path::new(GUEST_MANIFEST))?;
        let wall: u64 = std::env::var("MD_SIM_S9_MINER_WALL_SECONDS")?.parse()?;
        let seconds = manifest.observation_seconds(pool_seconds, wall)?;
        attach(command, &endpoint);
        let observation = Self {
            endpoint: Some(endpoint),
            started: Instant::now(),
            seconds,
        };
        println!(
            "MD_SIM_SHARED_POOL_ATTACHMENT {}",
            json!({
                "schema_version":"md-sim-s9-shared-pool-observation/1",
                "manifest":hash_file(Path::new(GUEST_MANIFEST))?,
                "pool_input":hash_file(Path::new("/private-pool.json"))?,
                "pool_verifier":manifest.pool_verifier,
                "guest_descriptor":GUEST_FD,"pool_seconds":pool_seconds,
                "observation_seconds":seconds,"observation_start":"immediately before owned QEMU spawn",
                "pool_readiness":"Required separately from the selected server before ExecuteToken",
                "scope":"Carrier attachment and bounded miner observation only. No pool readiness or mining success is inferred."
            })
        );
        Ok(observation)
    }

    pub fn spawned(&mut self) {
        drop(self.endpoint.take());
        println!(
            "MD_SIM_SHARED_POOL_PARENT_CLOSED {}",
            json!({
                "elapsed_milliseconds":self.started.elapsed().as_millis(),
                "parent_endpoint_closed":true,
                "scope":"QEMU owns the inherited endpoint. The runner retains no carrier descriptor."
            })
        );
    }

    pub fn wait_window(&self) -> Result<()> {
        ensure!(
            self.endpoint.is_none(),
            "parent still owns the shared carrier"
        );
        let end = self.started + Duration::from_secs(self.seconds);
        while let Some(left) = end.checked_duration_since(Instant::now()) {
            std::thread::sleep(left.min(Duration::from_millis(20)));
        }
        println!(
            "MD_SIM_SHARED_POOL_OBSERVATION_END {}",
            json!({
                "observation_seconds":self.seconds,
                "elapsed_milliseconds":self.started.elapsed().as_millis(),
                "scope":"The declared miner observation interval ended. External pool collection and independent mining verification remain required."
            })
        );
        Ok(())
    }
}

fn attach(command: &mut Command, endpoint: &File) {
    let source = endpoint.as_raw_fd();
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ECANCELED));
            }
            if libc::dup2(source, GUEST_FD) < 0 || libc::fcntl(GUEST_FD, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if source != GUEST_FD {
                libc::close(source);
            }
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::unix::net::UnixStream, process::Stdio};

    fn manifest() -> Manifest {
        let component = ObjectId {
            sha256: "ab".repeat(32),
            size: 1,
        };
        Manifest {
            schema_version: "md-sim-s9-shared-pool/1".into(),
            pool_verifier: component.clone(),
            support_runner: component.clone(),
            support_backend: component.clone(),
            support_kernel: component,
            network_contract: "md-sim-s9-server-namespace/1".into(),
            pool_endpoint: "10.0.2.100:3333".into(),
            miner_address: "10.0.2.15/24".into(),
            readiness_seconds: 10,
            observation_margin_seconds: 5,
            carrier: Spec {
                protocol: Protocol::QemuEthernetV1,
                guest_fd: 3,
                max_frame_bytes: 65535,
                max_wire_bytes: 128 << 20,
                max_packets: 1 << 20,
            },
        }
    }

    #[test]
    fn contract_rejects_changed_network_unbounded_carrier_and_extended_budget() {
        let original = manifest();
        original.validate().unwrap();
        assert_eq!(original.observation_seconds(300, 360).unwrap(), 305);
        assert!(original.observation_seconds(300, 305).is_err());
        assert!(original.observation_seconds(301, 360).is_err());
        let mut changed = original.clone();
        changed.pool_endpoint = "127.0.0.1:3333".into();
        assert!(changed.validate().is_err());
        changed = original.clone();
        changed.carrier.guest_fd = 9;
        assert!(changed.validate().is_err());
        changed = original.clone();
        changed.carrier.max_wire_bytes += 1;
        assert!(changed.validate().is_err());
        changed = original.clone();
        changed.support_kernel.size = 0;
        assert!(changed.validate().is_err());
        let mut value = serde_json::to_value(original).unwrap();
        value["passed"] = json!(true);
        assert!(serde_json::from_value::<Manifest>(value).is_err());
    }

    #[test]
    fn only_selected_child_inherits_carrier_and_parent_close_exposes_eof() {
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let (endpoint, peer) = UnixStream::pair().unwrap();
        let endpoint = File::from(std::os::fd::OwnedFd::from(endpoint));
        let mut peer = File::from(std::os::fd::OwnedFd::from(peer));
        protect(&endpoint).unwrap();
        let test_module = module_path!().split_once("::").unwrap().1;
        let executable = std::env::current_exe().unwrap();
        let status = Command::new(&executable)
            .args([
                "--exact",
                &format!("{test_module}::unselected_child"),
                "--ignored",
            ])
            .env("S9_SHARED_UNSELECTED_FD", endpoint.as_raw_fd().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        let mut command = Command::new(executable);
        let child_test = format!("{test_module}::inherited_child");
        command
            .args(["--exact", &child_test, "--ignored"])
            .stdin(Stdio::null())
            .stdout(Stdio::null());
        attach(&mut command, &endpoint);
        let mut child = ChildGuard(command.spawn().unwrap());
        drop(command);
        drop(endpoint);
        peer.write_all(b"actual carrier").unwrap();
        let mut wait = libc::pollfd {
            fd: peer.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(unsafe { libc::poll(&mut wait, 1, 3000) }, 1);
        let mut response = [0; 14];
        peer.read_exact(&mut response).unwrap();
        assert_eq!(&response, b"actual carrier");
        assert_eq!(unsafe { libc::poll(&mut wait, 1, 3000) }, 1);
        assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
        assert!(child.0.wait().unwrap().success());
        assert!(protect(&File::open("/dev/null").unwrap()).is_err());
    }

    #[test]
    #[ignore = "executed as the owned child of the descriptor regression"]
    fn inherited_child() {
        let mut endpoint = take_endpoint().unwrap();
        let mut message = [0; 14];
        endpoint.read_exact(&mut message).unwrap();
        endpoint.write_all(&message).unwrap();
    }

    #[test]
    #[ignore = "executed without attachment by the descriptor regression"]
    fn unselected_child() {
        let source: i32 = std::env::var("S9_SHARED_UNSELECTED_FD")
            .unwrap()
            .parse()
            .unwrap();
        for descriptor in [GUEST_FD, source] {
            assert_eq!(unsafe { libc::fcntl(descriptor, libc::F_GETFD) }, -1);
            assert_eq!(
                std::io::Error::last_os_error().raw_os_error(),
                Some(libc::EBADF)
            );
        }
    }
}
