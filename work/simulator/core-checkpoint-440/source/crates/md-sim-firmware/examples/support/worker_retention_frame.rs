//! One bounded receipt frame on a separately owned virtual character port.
use anyhow::{ensure, Result};
use std::{
    fs::File,
    io::Read,
    os::fd::AsRawFd,
    time::{Duration, Instant},
};

pub fn read(device: &mut File, maximum: usize, seconds: u64) -> Result<Vec<u8>> {
    ensure!(
        (1..=8192).contains(&maximum) && (1..=180).contains(&seconds),
        "receipt reader bounds differ"
    );
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut bytes = Vec::new();
    loop {
        ensure!(
            Instant::now() < deadline,
            "outer retention acknowledgment deadline"
        );
        let mut event = libc::pollfd {
            fd: device.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut event, 1, 100) };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        ensure!(
            event.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) == 0,
            "receipt channel closed"
        );
        if event.revents & libc::POLLIN == 0 {
            continue;
        }
        let mut chunk = vec![0; maximum + 1];
        let count = device.read(&mut chunk)?;
        ensure!(count != 0, "receipt channel EOF");
        bytes.extend_from_slice(&chunk[..count]);
        ensure!(bytes.len() <= maximum, "receipt frame too large");
        if bytes.contains(&b'\n') {
            ensure!(
                bytes.last() == Some(&b'\n')
                    && bytes[..bytes.len() - 1].iter().all(|b| *b != b'\n'),
                "extra receipt bytes"
            );
            return Ok(bytes);
        }
    }
}
