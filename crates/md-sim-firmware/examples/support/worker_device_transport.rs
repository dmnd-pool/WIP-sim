//! An owned character-device channel with no reuse after an uncertain exchange.
//! A delayed or partial response requires a fresh channel and the saved run
//! token. A complete validated worker error permits a later independent request.
//! This type does not establish SSH authentication.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::WorkerTransport;
use md_sim_runtime::worker::{request_device, CompletedWorkerError, Request};
use serde_json::Value;
use std::{fs::File, os::unix::fs::FileTypeExt};

pub struct DeviceTransport {
    device: Option<File>,
}

impl DeviceTransport {
    /// The caller must verify the private device identity before transferring
    /// ownership. It must not use another copy of this descriptor for requests.
    pub fn new(device: File) -> Result<Self> {
        ensure!(
            device.metadata()?.file_type().is_char_device(),
            "worker device channel is not a character device"
        );
        Ok(Self {
            device: Some(device),
        })
    }
}

impl WorkerTransport for DeviceTransport {
    fn request(&mut self, request: &Request) -> Result<Value> {
        let mut device = self
            .device
            .take()
            .context("worker device channel is closed after a failed exchange")?;
        let result = request_device(&mut device, request);
        let complete_error = result
            .as_ref()
            .err()
            .is_some_and(|error| error.is::<CompletedWorkerError>());
        if result.is_ok() || complete_error {
            self.device = Some(device);
            result
        } else {
            result.context("worker device exchange failed; channel closed")
        }
    }

    fn description(&self) -> String {
        "private virtual character channel; no SSH authentication claim".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::{self, Read, Write},
        os::fd::{AsRawFd, FromRawFd},
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };

    fn pty() -> (File, File) {
        let mut master = -1;
        let mut slave = -1;
        // SAFETY: valid output pointers; optional arguments are null.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                )
            },
            0,
            "{}",
            io::Error::last_os_error()
        );
        // SAFETY: each fresh descriptor transfers ownership exactly once.
        let master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        let mut attributes = std::mem::MaybeUninit::<libc::termios>::uninit();
        assert_eq!(
            unsafe { libc::tcgetattr(slave.as_raw_fd(), attributes.as_mut_ptr()) },
            0
        );
        // SAFETY: tcgetattr initialized the value and the descriptor is live.
        let mut attributes = unsafe { attributes.assume_init() };
        unsafe { libc::cfmakeraw(&mut attributes) };
        assert_eq!(
            unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &attributes) },
            0
        );
        for file in [&master, &slave] {
            assert_eq!(
                unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) },
                0
            );
            let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
            assert!(flags >= 0);
            assert_eq!(
                unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
                0
            );
        }
        (master, slave)
    }

    fn readable(file: &File, timeout_ms: i32) -> bool {
        let mut event = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let count = unsafe { libc::poll(&mut event, 1, timeout_ms) };
        assert!(count >= 0, "{}", io::Error::last_os_error());
        count != 0 && event.revents & libc::POLLIN != 0
    }

    fn frame(file: &mut File) -> Value {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut bytes = Vec::new();
        while bytes.last() != Some(&b'\n') {
            assert!(Instant::now() < deadline, "test frame deadline");
            assert!(readable(file, 1000), "test frame did not arrive");
            let mut byte = [0];
            assert_eq!(file.read(&mut byte).unwrap(), 1);
            bytes.push(byte[0]);
            assert!(bytes.len() <= 4096, "test frame bound");
        }
        serde_json::from_slice(&bytes).unwrap()
    }

    fn reply(file: &mut File, result: Value) {
        let bytes =
            serde_json::to_vec(&json!({"protocol":"md-sim-worker/2","ok":true,"result":result}))
                .unwrap();
        file.write_all(&bytes).unwrap();
        file.write_all(b"\n").unwrap();
    }

    #[test]
    fn sequential_successes_use_the_same_real_character_channel() {
        let (mut peer, device) = pty();
        let (finished, hold) = mpsc::channel();
        let server = thread::spawn(move || {
            assert_eq!(frame(&mut peer), json!({"operation":"probe"}));
            reply(&mut peer, json!({"sequence":1}));
            assert_eq!(frame(&mut peer), json!({"operation":"shutdown"}));
            reply(&mut peer, json!({"stopping":true}));
            // Keep the PTY master alive until its final bytes are read.
            hold.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        let mut transport = DeviceTransport::new(device).unwrap();
        assert_eq!(transport.request(&Request::Probe).unwrap()["sequence"], 1);
        assert_eq!(
            transport.request(&Request::Shutdown).unwrap()["stopping"],
            true
        );
        finished.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn timeout_closes_channel_and_cannot_consume_a_late_reply() {
        let (mut peer, device) = pty();
        // This copy is an observer only. It lets the test queue a late response
        // after the owned transport descriptor closes, and inspect that response.
        let mut observer = device.try_clone().unwrap();
        let mut transport = DeviceTransport::new(device).unwrap();
        let error = transport.request(&Request::Probe).unwrap_err();
        assert!(error
            .chain()
            .filter_map(|cause| cause.downcast_ref::<io::Error>())
            .any(|cause| cause.kind() == io::ErrorKind::TimedOut));
        assert!(transport.device.is_none());
        assert_eq!(frame(&mut peer), json!({"operation":"probe"}));

        reply(&mut peer, json!({"late_reply":true}));
        assert!(readable(&observer, 1000), "late response did not arrive");
        let error = transport.request(&Request::Shutdown).unwrap_err();
        assert!(error.to_string().contains("closed after a failed exchange"));
        assert_eq!(frame(&mut observer)["result"]["late_reply"], true);
        assert!(!readable(&peer, 50), "closed channel sent another request");
    }

    #[test]
    fn complete_worker_error_preserves_a_later_independent_request() {
        let (mut peer, device) = pty();
        let (finished, hold) = mpsc::channel();
        let expected = json!({"protocol":"md-sim-worker/2","ok":false,
            "error":"selected run is absent","detail":{"run_id":"missing"}});
        let envelope = expected.clone();
        let server = thread::spawn(move || {
            assert_eq!(
                frame(&mut peer),
                json!({"operation":"status","run_id":"missing"})
            );
            peer.write_all(&serde_json::to_vec(&envelope).unwrap())
                .unwrap();
            peer.write_all(b"\n").unwrap();
            assert_eq!(frame(&mut peer), json!({"operation":"probe"}));
            reply(&mut peer, json!({"independent_probe":true}));
            hold.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        let mut transport = DeviceTransport::new(device).unwrap();
        let error = transport
            .request(&Request::Status {
                run_id: "missing".into(),
            })
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<CompletedWorkerError>()
                .unwrap()
                .response(),
            &expected
        );
        assert!(transport.device.is_some());
        assert_eq!(
            transport.request(&Request::Probe).unwrap()["independent_probe"],
            true
        );
        finished.send(()).unwrap();
        server.join().unwrap();
    }

    #[test]
    fn malformed_error_responses_close_the_owned_descriptor() {
        for response in [
            &b"invalid JSON\n"[..],
            &b"{\"protocol\":\"md-sim-worker/2\",\"ok\":false,\"error\":null}\n"[..],
            &b"{\"protocol\":\"md-sim-worker/2\",\"ok\":false,\"error\":\"\"}\n"[..],
            &b"{\"protocol\":\"md-sim-worker/1\",\"ok\":false,\"error\":\"rejected\"}\n"[..],
        ] {
            let (mut peer, device) = pty();
            let _observer = device.try_clone().unwrap();
            peer.write_all(response).unwrap();
            let mut transport = DeviceTransport::new(device).unwrap();
            let error = transport.request(&Request::Probe).unwrap_err();
            assert!(!error.is::<CompletedWorkerError>());
            assert!(transport.device.is_none());
            assert_eq!(frame(&mut peer), json!({"operation":"probe"}));
            assert!(transport.request(&Request::Shutdown).is_err());
            assert!(!readable(&peer, 50), "failed channel was reused");
        }
    }

    #[test]
    fn constructor_rejects_a_regular_file_without_changing_it() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"retained input").unwrap();
        assert!(DeviceTransport::new(file.reopen().unwrap()).is_err());
        assert_eq!(std::fs::read(file.path()).unwrap(), b"retained input");
    }
}
