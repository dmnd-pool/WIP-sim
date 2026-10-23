//! One absolute deadline for a private worker request, including connect.
use super::{check_response, read_message, Request, MAX_REQUEST};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{
    fs::File,
    io::{self, Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    path::Path,
    time::{Duration, Instant},
};

struct BoundedMessage(Vec<u8>);
impl Write for BoundedMessage {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > (MAX_REQUEST as usize - 1).saturating_sub(self.0.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "worker request exceeds message bound",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct DeadlineIo {
    file: File,
    deadline: Instant,
}

impl DeadlineIo {
    fn remaining(&self) -> io::Result<Duration> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            Err(io::ErrorKind::TimedOut.into())
        } else {
            Ok(remaining)
        }
    }
    fn wait(&self, events: i16) -> io::Result<()> {
        loop {
            let remaining = self.remaining()?;
            let mut event = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events,
                revents: 0,
            };
            let ready =
                unsafe { libc::poll(&mut event, 1, remaining.as_millis().clamp(1, 50) as i32) };
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if event.revents & libc::POLLNVAL != 0 {
                return Err(io::ErrorKind::InvalidInput.into());
            }
            if ready > 0 {
                self.remaining()?;
                return Ok(());
            }
        }
    }
}
impl Read for DeadlineIo {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            self.wait(libc::POLLIN)?;
            match self.file.read(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
}
impl Write for DeadlineIo {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            self.wait(libc::POLLOUT)?;
            match self.file.write(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.remaining().map(|_| ())
    }
}

fn connect(path: &Path, deadline: Instant) -> Result<DeadlineIo> {
    let raw = path.as_os_str().as_encoded_bytes();
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    ensure!(
        !raw.is_empty() && raw.len() < address.sun_path.len() && !raw.contains(&0),
        "invalid worker socket path"
    );
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (destination, source) in address.sun_path.iter_mut().zip(raw) {
        *destination = *source as libc::c_char;
    }
    let descriptor = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    ensure!(
        descriptor >= 0,
        "create nonblocking worker socket: {}",
        io::Error::last_os_error()
    );
    let channel = DeadlineIo {
        file: unsafe { File::from_raw_fd(descriptor) },
        deadline,
    };
    channel.remaining()?;
    let length =
        (std::mem::offset_of!(libc::sockaddr_un, sun_path) + raw.len() + 1) as libc::socklen_t;
    let result = unsafe {
        libc::connect(
            channel.file.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            length,
        )
    };
    if result < 0 {
        let error = io::Error::last_os_error();
        // Linux AF_UNIX reports EAGAIN for a full backlog. It is not a
        // connection in progress; return it without retry or waiting.
        if error.raw_os_error() != Some(libc::EINPROGRESS) {
            return Err(error).context("connect to worker");
        }
        channel.wait(libc::POLLOUT)?;
        let mut socket_error: libc::c_int = 0;
        let mut size = std::mem::size_of_val(&socket_error) as libc::socklen_t;
        ensure!(
            unsafe {
                libc::getsockopt(
                    channel.file.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_ERROR,
                    (&mut socket_error as *mut libc::c_int).cast(),
                    &mut size,
                )
            } == 0
                && size as usize == std::mem::size_of_val(&socket_error),
            "cannot read worker connection result"
        );
        if socket_error != 0 {
            return Err(io::Error::from_raw_os_error(socket_error)).context("connect to worker");
        }
    }
    channel.remaining()?;
    Ok(channel)
}

/// The caller authenticates the socket path and chooses its policy deadline.
/// A fresh owned connection closes on every outcome; uncertain exchanges are
/// never reused. Slow partial replies cannot renew the absolute time budget.
pub fn request_with_timeout(socket: &Path, request: &Request, timeout: Duration) -> Result<Value> {
    ensure!(
        !timeout.is_zero() && timeout <= Duration::from_secs(60),
        "worker request timeout must be positive and at most 60 seconds"
    );
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("worker request deadline overflow")?;
    let mut message = BoundedMessage(Vec::new());
    serde_json::to_writer(&mut message, request)?;
    message.0.push(b'\n');
    exchange(connect(socket, deadline)?, &message.0)
}

fn exchange(mut channel: DeadlineIo, message: &[u8]) -> Result<Value> {
    channel.write_all(message)?;
    let value = read_message(&mut channel)?;
    let result = check_response(value);
    channel.remaining()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::{CompletedWorkerError, PROTOCOL_VERSION};
    use std::{
        io::{BufRead, BufReader},
        os::unix::net::UnixListener,
        sync::mpsc,
        thread,
    };

    #[test]
    #[ignore = "executed as a captured helper in the private network VM"]
    fn actual_socket_uses_the_existing_complete_response_and_error_contract() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("worker.sock");
        let listener = UnixListener::bind(&path)?;
        let server = thread::spawn(move || -> Result<()> {
            for ok in [true, false] {
                let (mut stream, _) = listener.accept()?;
                let mut request = String::new();
                BufReader::new(&mut stream).read_line(&mut request)?;
                assert!(matches!(
                    serde_json::from_str::<Request>(&request)?,
                    Request::Probe
                ));
                writeln!(
                    stream,
                    "{}",
                    if ok {
                        serde_json::json!({"protocol":PROTOCOL_VERSION,"ok":true,"result":{"actual_reply":true}})
                    } else {
                        serde_json::json!({"protocol":PROTOCOL_VERSION,"ok":false,"error":"actual complete worker error"})
                    }
                )?;
            }
            Ok(())
        });
        assert_eq!(
            request_with_timeout(&path, &Request::Probe, Duration::from_secs(1))?["actual_reply"],
            true
        );
        assert!(
            request_with_timeout(&path, &Request::Probe, Duration::from_secs(1))
                .unwrap_err()
                .is::<CompletedWorkerError>()
        );
        server.join().unwrap()?;
        Ok(())
    }

    #[test]
    #[ignore = "executed as a captured helper in the private network VM"]
    fn slow_partial_reply_cannot_extend_the_one_absolute_deadline() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("worker.sock");
        let listener = UnixListener::bind(&path)?;
        let (sent, received) = mpsc::channel();
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = String::new();
            BufReader::new(&mut stream).read_line(&mut request)?;
            let mut count = 0;
            for _ in 0..30 {
                if stream.write_all(b" ").is_err() {
                    break;
                }
                count += 1;
                thread::sleep(Duration::from_millis(20));
            }
            sent.send(count)?;
            Ok(())
        });
        let started = Instant::now();
        let error =
            request_with_timeout(&path, &Request::Probe, Duration::from_millis(120)).unwrap_err();
        assert!(error.chain().any(|error| error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::TimedOut)));
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(received.recv_timeout(Duration::from_secs(1))? > 1);
        server.join().unwrap()?;
        Ok(())
    }

    #[test]
    #[ignore = "executed as a captured helper in the private network VM"]
    fn a_full_actual_unix_backlog_fails_without_a_blocking_connect() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("worker.sock");
        let listener = UnixListener::bind(&path)?;
        ensure!(
            unsafe { libc::listen(listener.as_raw_fd(), 1) } == 0,
            "set test backlog"
        );
        let mut held = Vec::new();
        let mut full = false;
        for _ in 0..8 {
            match connect(&path, Instant::now() + Duration::from_secs(1)) {
                Ok(channel) => held.push(channel),
                Err(error) => {
                    assert!(error.chain().any(|error| error
                        .downcast_ref::<io::Error>()
                        .is_some_and(|error| error.raw_os_error() == Some(libc::EAGAIN))));
                    full = true;
                    break;
                }
            }
        }
        assert!(full && !held.is_empty());
        let started = Instant::now();
        assert!(request_with_timeout(&path, &Request::Probe, Duration::from_secs(1)).is_err());
        assert!(started.elapsed() < Duration::from_millis(250));
        Ok(())
    }

    #[test]
    fn actual_socket_pair_drip_obeys_the_same_absolute_exchange_deadline() -> Result<()> {
        use std::os::{fd::OwnedFd, unix::net::UnixStream};
        let (client, server) = UnixStream::pair()?;
        client.set_nonblocking(true)?;
        server.set_nonblocking(true)?;
        let (sent, received) = mpsc::channel();
        let server = thread::spawn(move || -> Result<()> {
            let mut channel = DeadlineIo {
                file: File::from(OwnedFd::from(server)),
                deadline: Instant::now() + Duration::from_secs(1),
            };
            assert_eq!(read_message(&mut channel)?["operation"], "probe");
            let mut count = 0;
            for _ in 0..30 {
                if channel.write_all(b" ").is_err() {
                    break;
                }
                count += 1;
                thread::sleep(Duration::from_millis(20));
            }
            sent.send(count)?;
            Ok(())
        });
        let started = Instant::now();
        let channel = DeadlineIo {
            file: File::from(OwnedFd::from(client)),
            deadline: started + Duration::from_millis(120),
        };
        let error = exchange(channel, b"{\"operation\":\"probe\"}\n").unwrap_err();
        assert!(error.chain().any(|error| error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::TimedOut)));
        assert!(started.elapsed() < Duration::from_millis(500));
        assert!(received.recv_timeout(Duration::from_secs(1))? > 1);
        server.join().unwrap()?;
        Ok(())
    }
}
