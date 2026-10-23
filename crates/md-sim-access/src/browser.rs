//! Bounded client for Firefox's existing Marionette protocol, version 3.
//! The browser must run in the caller's isolated client environment. Its
//! control endpoint is loopback-only and is never a dashboard endpoint.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::fd::AsRawFd,
    time::{Duration, Instant},
};

const MAX_FRAME: usize = 4 * 1024 * 1024;

/// WebDriver navigation barrier. Tests must still verify their actual page
/// state, assets and service behavior after navigation returns.
#[derive(Clone, Copy)]
pub enum PageLoad {
    Complete,
    Interactive,
}
impl PageLoad {
    pub fn capability(self) -> &'static str {
        match self {
            Self::Complete => "normal",
            Self::Interactive => "eager",
        }
    }
}

fn ready(fd: libc::c_int, events: libc::c_short, deadline: Instant) -> Result<()> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("browser command deadline")?;
        let mut p = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let n = unsafe {
            libc::poll(
                &mut p,
                1,
                remaining.as_millis().clamp(1, i32::MAX as u128) as i32,
            )
        };
        if n < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        ensure!(n > 0, "browser command deadline");
        ensure!(
            p.revents & libc::POLLNVAL == 0,
            "browser control descriptor closed"
        );
        return Ok(());
    }
}
fn read_exact<S: Read + AsRawFd>(
    stream: &mut S,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> Result<()> {
    while !bytes.is_empty() {
        ready(stream.as_raw_fd(), libc::POLLIN, deadline)?;
        match stream.read(bytes) {
            Ok(0) => anyhow::bail!("browser control stream ended inside a frame"),
            Ok(n) => bytes = &mut bytes[n..],
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn receive<S: Read + AsRawFd>(stream: &mut S, deadline: Instant) -> Result<Value> {
    let mut size = 0usize;
    let mut digits = 0;
    loop {
        let mut byte = [0];
        read_exact(stream, &mut byte, deadline)?;
        if byte[0] == b':' {
            ensure!(digits > 0 && size > 0, "empty browser frame");
            break;
        }
        ensure!(
            byte[0].is_ascii_digit() && digits < 7,
            "invalid browser frame length"
        );
        ensure!(
            digits != 1 || size != 0,
            "non-canonical browser frame length"
        );
        size = size * 10 + usize::from(byte[0] - b'0');
        digits += 1;
        ensure!(size <= MAX_FRAME, "browser response exceeds frame budget");
    }
    let mut bytes = vec![0; size];
    read_exact(stream, &mut bytes, deadline)?;
    serde_json::from_slice(&bytes).context("invalid browser response JSON")
}
fn send<S: Write + AsRawFd>(stream: &mut S, value: &Value, deadline: Instant) -> Result<()> {
    let payload = serde_json::to_vec(value)?;
    ensure!(
        payload.len() <= MAX_FRAME,
        "browser request exceeds frame budget"
    );
    let mut frame = format!("{}:", payload.len()).into_bytes();
    frame.extend(payload);
    let mut bytes = frame.as_slice();
    while !bytes.is_empty() {
        ready(stream.as_raw_fd(), libc::POLLOUT, deadline)?;
        match stream.write(bytes) {
            Ok(0) => anyhow::bail!("browser control write made no progress"),
            Ok(n) => bytes = &bytes[n..],
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn response(value: Value, id: u32) -> Result<Value> {
    let values = value
        .as_array()
        .context("browser response is not an array")?;
    ensure!(
        values.len() == 4 && values[0] == 1 && values[1] == id,
        "browser response identity differs"
    );
    ensure!(
        values[2].is_null(),
        "browser command failed: {}: {}",
        values[2]["error"]
            .as_str()
            .unwrap_or("invalid error object"),
        values[2]["message"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(512)
            .collect::<String>()
    );
    Ok(values[3].clone())
}

pub struct Session {
    stream: TcpStream,
    sequence: u32,
    timeout: Duration,
    pub handshake: Value,
}
impl Session {
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<()> {
        ensure!(
            !timeout.is_zero() && timeout <= Duration::from_secs(120),
            "browser command timeout must be within120 seconds"
        );
        self.timeout = timeout;
        Ok(())
    }
    pub fn connect(address: SocketAddr, timeout: Duration) -> Result<Self> {
        ensure!(
            address.ip().is_loopback() && address.port() > 0,
            "browser control must use loopback"
        );
        ensure!(
            !timeout.is_zero() && timeout <= Duration::from_secs(60),
            "browser timeout must be within 60 seconds"
        );
        let deadline = Instant::now() + timeout;
        let mut stream = TcpStream::connect_timeout(&address, timeout)?;
        stream.set_nonblocking(true)?;
        let handshake = receive(&mut stream, deadline)?;
        ensure!(
            handshake["applicationType"] == "gecko" && handshake["marionetteProtocol"] == 3,
            "unsupported browser protocol"
        );
        Ok(Self {
            stream,
            sequence: 0,
            timeout,
            handshake,
        })
    }
    fn command(&mut self, name: &str, parameters: Value) -> Result<Value> {
        self.command_with_timeout(name, parameters, self.timeout)
    }
    fn command_with_timeout(
        &mut self,
        name: &str,
        parameters: Value,
        timeout: Duration,
    ) -> Result<Value> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .context("browser sequence exhausted")?;
        let deadline = Instant::now() + timeout;
        send(
            &mut self.stream,
            &json!([0, self.sequence, name, parameters]),
            deadline,
        )?;
        response(receive(&mut self.stream, deadline)?, self.sequence)
            .with_context(|| format!("browser command {name}, sequence {}", self.sequence))
    }
    /// Certificate verification remains enabled. The caller must import the
    /// selected server CA and client credential into this run's new profile.
    pub fn start(&mut self) -> Result<Value> {
        self.start_with_timeout(self.timeout)
    }
    /// Startup can take longer under full-system CPU emulation. This separate
    /// budget does not change the deadline of subsequent browser commands.
    pub fn start_with_timeout(&mut self, timeout: Duration) -> Result<Value> {
        self.start_with_page_load(timeout, PageLoad::Complete)
    }

    pub fn start_with_page_load(
        &mut self,
        timeout: Duration,
        page_load: PageLoad,
    ) -> Result<Value> {
        ensure!(
            !timeout.is_zero() && timeout <= Duration::from_secs(270),
            "browser startup timeout must be within270 seconds"
        );
        self.command_with_timeout(
            "WebDriver:NewSession",
            json!({
            "acceptInsecureCerts":false,"pageLoadStrategy":page_load.capability(),
            "timeouts":{"pageLoad":30000,"script":5000,"implicit":0}}),
            timeout,
        )
    }
    pub fn navigate(&mut self, url: &str) -> Result<Value> {
        ensure!(
            url.len() <= 8192 && (url.starts_with("http://") || url.starts_with("https://")),
            "dashboard navigation requires an HTTP URL"
        );
        self.command("WebDriver:Navigate", json!({"url":url}))
    }
    pub fn execute(&mut self, script: &str, args: Vec<Value>) -> Result<Value> {
        ensure!(
            script.len() <= 65536 && args.len() <= 32,
            "browser script exceeds budget"
        );
        self.command(
            "WebDriver:ExecuteScript",
            json!({"script":script,"args":args,"newSandbox":true,"sandbox":null}),
        )
    }
    pub fn find(&mut self, selector: &str) -> Result<Value> {
        ensure!(selector.len() <= 4096, "selector exceeds budget");
        self.command(
            "WebDriver:FindElement",
            json!({"using":"css selector","value":selector}),
        )
    }
    pub fn click(&mut self, element_id: &str) -> Result<Value> {
        ensure!(element_id.len() <= 128, "element identity exceeds budget");
        self.command("WebDriver:ElementClick", json!({"id":element_id}))
    }
    pub fn send_keys(&mut self, element_id: &str, text: &str) -> Result<Value> {
        ensure!(
            element_id.len() <= 128 && text.len() <= 4096,
            "browser key input exceeds budget"
        );
        self.command(
            "WebDriver:ElementSendKeys",
            json!({"id":element_id,"text":text}),
        )
    }
    pub fn clear(&mut self, element_id: &str) -> Result<Value> {
        ensure!(
            element_id.len() <= 128,
            "browser element identity exceeds budget"
        );
        self.command("WebDriver:ElementClear", json!({"id":element_id}))
    }
    pub fn set_script_timeout(&mut self, milliseconds: u32) -> Result<Value> {
        ensure!(
            (1..=30000).contains(&milliseconds),
            "browser script timeout exceeds budget"
        );
        self.command("WebDriver:SetTimeouts", json!({"script":milliseconds}))
    }
    /// A support-tool budget for slow CPU emulation. This does not alter the
    /// guest's service, network or device deadlines.
    pub fn set_page_timeout(&mut self, milliseconds: u32) -> Result<Value> {
        ensure!(
            (1..=90000).contains(&milliseconds),
            "browser page timeout exceeds budget"
        );
        self.command("WebDriver:SetTimeouts", json!({"pageLoad":milliseconds}))
    }
    pub fn cookies(&mut self) -> Result<Value> {
        self.command("WebDriver:GetCookies", json!({}))
    }
    pub fn close(&mut self) -> Result<Value> {
        self.command("WebDriver:DeleteSession", json!({}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::File,
        os::{fd::OwnedFd, unix::net::UnixStream},
        thread,
    };
    fn pair() -> Result<(File, File)> {
        let (client, peer) = UnixStream::pair()?;
        client.set_nonblocking(true)?;
        peer.set_nonblocking(true)?;
        Ok((
            File::from(OwnedFd::from(client)),
            File::from(OwnedFd::from(peer)),
        ))
    }
    #[test]
    fn utf8_frames_and_response_identity() -> Result<()> {
        let (mut client, mut peer) = pair()?;
        let deadline = Instant::now() + Duration::from_secs(1);
        send(&mut peer, &json!([1, 7, null, {"value":"温度"}]), deadline)?;
        assert_eq!(
            response(receive(&mut client, deadline)?, 7)?,
            json!({"value":"温度"})
        );
        assert!(response(json!([1, 6, null, {}]), 7).is_err());
        assert!(response(json!([0, 7, null, {}]), 7).is_err());
        assert!(response(json!([1, 7, {"error":"invalid session id"}, null]), 7).is_err());
        Ok(())
    }
    #[test]
    fn malformed_and_oversized_frames_fail_before_allocation() -> Result<()> {
        for bytes in [
            b"4194305:".as_slice(),
            b":",
            b"0:",
            b"01:0",
            b"x:",
            b"2:{",
            b"4:null",
        ] {
            let (mut client, mut peer) = pair()?;
            peer.write_all(bytes)?;
            drop(peer);
            let result = receive(&mut client, Instant::now() + Duration::from_secs(1));
            if bytes == b"4:null" {
                assert_eq!(result?, Value::Null);
            } else {
                assert!(result.is_err(), "accepted {bytes:?}");
            }
        }
        Ok(())
    }
    #[test]
    fn slow_partial_frame_cannot_extend_absolute_deadline() -> Result<()> {
        let (mut client, mut peer) = pair()?;
        let writer = thread::spawn(move || {
            for byte in b"100:incomplete" {
                if peer.write_all(&[*byte]).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let start = Instant::now();
        let error = receive(&mut client, start + Duration::from_millis(45)).unwrap_err();
        assert!(error.to_string().contains("deadline"), "{error}");
        assert!(start.elapsed() >= Duration::from_millis(35));
        assert!(start.elapsed() < Duration::from_millis(250));
        drop(client);
        writer.join().unwrap();
        Ok(())
    }
}
