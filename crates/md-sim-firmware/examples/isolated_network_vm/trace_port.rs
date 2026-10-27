//! Private support-guest output. Kernel console messages stay on the UART.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::fd::AsRawFd,
    sync::mpsc::sync_channel,
};

pub const NAME: &str = "md-sim.trace";
pub const PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const NOISE_COUNT: usize = 8;
const PREFIX: &str = "MD_SIM_VM_TRACE_STRESS ";
const NOISE: &str = "MD_SIM_TRACE_KERNEL_NOISE";
const STDERR_MARKER: &str = "MD_SIM_VM_TRACE_STDERR private output descriptor";

pub fn selected(cmdline: &str, name: &str) -> Result<bool> {
    let prefix = format!("{name}=");
    let values = cmdline
        .split_ascii_whitespace()
        .filter_map(|word| word.strip_prefix(&prefix))
        .collect::<Vec<_>>();
    ensure!(
        values.is_empty() || values == ["1"],
        "invalid or duplicate guest option {name}"
    );
    Ok(!values.is_empty())
}

pub fn redirect() -> Result<()> {
    let port = super::virtual_port::open(NAME)?;
    let fd = port.as_raw_fd();
    ensure!(
        fd > libc::STDERR_FILENO,
        "trace port replaced a standard fd"
    );
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    ensure!(
        flags >= 0,
        "trace port flags: {}",
        std::io::Error::last_os_error()
    );
    ensure!(
        unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) } == 0,
        "blocking trace port: {}",
        std::io::Error::last_os_error()
    );
    for destination in [libc::STDOUT_FILENO, libc::STDERR_FILENO] {
        ensure!(
            unsafe { libc::dup2(fd, destination) } == destination,
            "trace port duplication: {}",
            std::io::Error::last_os_error()
        );
        let inherited_flags = unsafe { libc::fcntl(destination, libc::F_GETFL) };
        ensure!(
            inherited_flags >= 0 && inherited_flags & libc::O_NONBLOCK == 0,
            "inherited trace descriptor is not blocking"
        );
    }
    // dup2 clears CLOEXEC. Scenario children inherit both output descriptors.
    Ok(())
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stress {
    schema: String,
    challenge_hex: String,
    payload: String,
    payload_sha256: String,
    kernel_noise_records: usize,
}

fn kernel_record(output: &mut impl Write, challenge: &str, index: usize) -> Result<()> {
    let record = format!("<3>{NOISE} {challenge} {index}\n");
    // /dev/kmsg makes one kernel record per write syscall. write_fmt can
    // issue several writes, each with a separate priority and message body.
    ensure!(
        output.write(record.as_bytes())? == record.len(),
        "kernel noise write was partial"
    );
    Ok(())
}

pub fn guest_stress() -> Result<()> {
    eprintln!("{STDERR_MARKER}");
    let mut challenge = [0u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut challenge)?;
    let challenge_hex = hex::encode(challenge);
    let payload = challenge_hex.repeat(PAYLOAD_BYTES / challenge_hex.len());
    let record = Stress {
        schema: "md-sim-virtio-trace-stress/1".into(),
        challenge_hex: challenge_hex.clone(),
        payload_sha256: hex::encode(Sha256::digest(payload.as_bytes())),
        payload,
        kernel_noise_records: NOISE_COUNT,
    };
    let bytes = serde_json::to_vec(&record)?;
    let mut kmsg = OpenOptions::new().write(true).open("/dev/kmsg")?;
    let (send_chunk, receive_chunk) = sync_channel::<usize>(0);
    let (send_noise, receive_noise) = sync_channel::<()>(0);
    let noise = std::thread::spawn(move || -> Result<()> {
        for index in 0..NOISE_COUNT {
            ensure!(receive_chunk.recv()? == index, "trace stress chunk order");
            kernel_record(&mut kmsg, &challenge_hex, index)?;
            send_noise.send(())?;
        }
        Ok(())
    });
    let write_result = (|| -> Result<()> {
        let mut output = std::io::stdout().lock();
        output.write_all(PREFIX.as_bytes())?;
        for index in 0..NOISE_COUNT {
            let start = bytes.len() * index / NOISE_COUNT;
            let end = bytes.len() * (index + 1) / NOISE_COUNT;
            output.write_all(&bytes[start..end])?;
            output.flush()?;
            send_chunk.send(index)?;
            receive_noise.recv()?;
        }
        output.write_all(b"\n")?;
        output.flush()?;
        Ok(())
    })();
    drop(send_chunk);
    drop(receive_noise);
    let noise_result = noise
        .join()
        .map_err(|_| anyhow::anyhow!("noise writer panicked"))?;
    write_result?;
    noise_result
}

pub fn verify(stdout: &[u8], uart: &[u8]) -> Result<serde_json::Value> {
    ensure!(
        stdout.len() <= 16 * 1024 * 1024 && uart.len() <= 1024 * 1024,
        "trace stress output bound"
    );
    let output = std::str::from_utf8(stdout)?;
    let kernel = std::str::from_utf8(uart)?;
    ensure!(
        !output.contains(NOISE),
        "kernel noise entered the user trace"
    );
    ensure!(
        !kernel.contains("MD_SIM_VM_"),
        "user trace entered the kernel channel"
    );
    ensure!(
        output.lines().filter(|line| *line == STDERR_MARKER).count() == 1,
        "redirected guest stderr record absent or duplicated"
    );
    let mut records = output.lines().filter_map(|line| line.strip_prefix(PREFIX));
    let record: Stress =
        serde_json::from_str(records.next().context("trace stress record absent")?)?;
    ensure!(records.next().is_none(), "duplicate trace stress record");
    ensure!(
        record.schema == "md-sim-virtio-trace-stress/1"
            && record.challenge_hex.len() == 64
            && hex::decode(&record.challenge_hex)?.len() == 32
            && record.payload.len() == PAYLOAD_BYTES
            && record.kernel_noise_records == NOISE_COUNT,
        "trace stress identity or size differs"
    );
    ensure!(
        record
            .payload
            .bytes()
            .enumerate()
            .all(|(index, byte)| byte == record.challenge_hex.as_bytes()[index % 64]),
        "trace stress payload changed"
    );
    let digest = hex::encode(Sha256::digest(record.payload.as_bytes()));
    ensure!(
        digest == record.payload_sha256,
        "trace stress payload digest differs"
    );
    let actual = kernel
        .lines()
        .filter_map(|line| line.split_once(NOISE).map(|(_, value)| value.trim()))
        .collect::<Vec<_>>();
    let expected = (0..NOISE_COUNT)
        .map(|index| format!("{} {index}", record.challenge_hex))
        .collect::<Vec<_>>();
    ensure!(
        actual == expected,
        "kernel noise count, identity or order differs"
    );
    Ok(
        serde_json::json!({"schema":record.schema,"payload_bytes":record.payload.len(),"payload_sha256":digest,"challenge_hex":record.challenge_hex,"kernel_noise_records":actual.len(),"kernel_and_userspace_channels_separate":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_kernel_options_reject_duplicates_and_near_matches() {
        assert!(selected("other=1 md_sim_trace_port=1", "md_sim_trace_port").unwrap());
        assert!(!selected("xmd_sim_trace_port=1", "md_sim_trace_port").unwrap());
        assert!(selected("md_sim_trace_port=0", "md_sim_trace_port").is_err());
        assert!(selected(
            "md_sim_trace_port=1 md_sim_trace_port=1",
            "md_sim_trace_port"
        )
        .is_err());
    }
    #[test]
    fn kernel_message_is_one_complete_write_and_partial_records_fail() {
        struct Records {
            writes: Vec<Vec<u8>>,
            partial: bool,
        }
        impl Write for Records {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.writes.push(bytes.to_vec());
                Ok(bytes.len() - usize::from(self.partial))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut records = Records {
            writes: Vec::new(),
            partial: false,
        };
        kernel_record(&mut records, "abcd", 3).unwrap();
        assert_eq!(
            records.writes,
            [b"<3>MD_SIM_TRACE_KERNEL_NOISE abcd 3\n".to_vec()]
        );
        records.partial = true;
        assert!(kernel_record(&mut records, "abcd", 4).is_err());
        assert_eq!(records.writes.len(), 2);
    }
    #[test]
    fn verifier_rejects_corruption_missing_noise_and_cross_channel_records() {
        let challenge = "0123456789abcdef".repeat(4);
        let payload = challenge.repeat(PAYLOAD_BYTES / 64);
        let record = serde_json::json!({"schema":"md-sim-virtio-trace-stress/1","challenge_hex":challenge,"payload_sha256":hex::encode(Sha256::digest(payload.as_bytes())),"payload":payload,"kernel_noise_records":8});
        let output = format!("{STDERR_MARKER}\n{PREFIX}{record}\n");
        let uart = (0..8)
            .map(|index| format!("[ 1.000] {NOISE} {challenge} {index}\n"))
            .collect::<String>();
        assert!(verify(output.as_bytes(), uart.as_bytes()).is_ok());
        let mut damaged = output.clone().into_bytes();
        let payload_start = output.find("\"payload\":\"").unwrap() + "\"payload\":\"".len();
        damaged[payload_start + 5_100_000] ^= 1;
        assert!(verify(&damaged, uart.as_bytes()).is_err());
        assert!(verify(&output.as_bytes()[..output.len() - 100], uart.as_bytes()).is_err());
        assert!(verify(
            output.as_bytes(),
            uart.lines()
                .take(7)
                .collect::<Vec<_>>()
                .join("\n")
                .as_bytes()
        )
        .is_err());
        assert!(verify(
            output.as_bytes(),
            format!("{uart}MD_SIM_VM_INIT actual Rust PID 1 started\n").as_bytes()
        )
        .is_err());
        assert!(verify(format!("{output}{NOISE}\n").as_bytes(), uart.as_bytes()).is_err());
        assert!(verify(format!("{output}{output}").as_bytes(), uart.as_bytes()).is_err());
    }
}
