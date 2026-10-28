//! Execute the exact archived ESP-Miner factory image in the archived ESP32-S3
//! backend. The probe has no claim of complete board support or mining.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::fs::OpenOptionsExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/bm1397_probe_model.rs"]
mod probe_model;
#[path = "support/qemu_stderr_tail.rs"]
mod stderr_tail;
#[path = "support/esp32_uart_trace.rs"]
mod uart_trace;
#[path = "support/bm1397_write_intent.rs"]
mod write_intent;
const FIRMWARE: &str = "3fd36298681d7797429dd72610041d731558c5253430902a3c7feeced232557c";
const SOURCE: &str = "5456034eb8a9f348b1f4c8820dc79f55677bddfe0d1c66566b6c42ceb9df66f0";

struct UartCapture {
    stop: Arc<AtomicBool>,
    input: fs::File,
    thread: Option<std::thread::JoinHandle<std::io::Result<()>>>,
}
impl UartCapture {
    fn start() -> Result<Self> {
        for name in [c"/work/asic.in", c"/work/asic.out"] {
            ensure!(
                unsafe { libc::mkfifo(name.as_ptr(), 0o600) } == 0,
                "create private ASIC UART FIFO"
            );
        }
        let open = |name| {
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(name)
        };
        let input = open("/work/asic.in")?;
        let mut reply_pipe = input.try_clone()?;
        let mut output = open("/work/asic.out")?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let identify = std::env::var_os("MD_SIM_BM1397_IDENTIFY").is_some();
            let control_storage = std::env::var_os("MD_SIM_BM1397_CONTROL_STORAGE").is_some();
            let support = if control_storage {
                md_sim_firmware::bm1397::ControlWriteSupport::RegisterStorage
            } else {
                md_sim_firmware::bm1397::ControlWriteSupport::Unsupported
            };
            let observe = std::env::var_os("MD_SIM_BM1397_CORE0_WRITE_INTENT").is_some();
            let reference: Option<probe_model::Config> =
                std::env::var("MD_SIM_BM1397_REFERENCE_POLICY")
                    .ok()
                    .map(|text| serde_json::from_str(&text))
                    .transpose()
                    .map_err(std::io::Error::other)?;
            if control_storage && (!identify || observe) {
                return Err(std::io::Error::other("control storage requires identification and excludes the write-intent observer"));
            }
            if reference.is_some() && (!identify || !control_storage) {
                return Err(std::io::Error::other(
                    "reference work requires identification and control storage",
                ));
            }
            let mut chain = probe_model::Model::new(support, observe, reference.as_ref())
                .map_err(|error| std::io::Error::other(format!("ASIC policy: {error:?}")))?;
            if let Some(config) = &reference {
                eprintln!(
                    "md-sim ASIC reference policy: {}",
                    serde_json::to_string(config).map_err(std::io::Error::other)?
                );
            }
            let mut batch = probe_model::Batch::default();
            if identify {
                // Explicit development-stage inputs. GPIO, regulator, UART
                // timing and mining support are not inferred from this probe.
                chain.power_and_release_reset();
                eprintln!(
                    "md-sim ASIC identification stage: {}",
                    json!({
                        "chips":1,"powered":true,"reset_asserted":false,"control_write_support":support,
                        "pin_binding":"explicit stage input; not connected to GPIO or regulator",
                        "timing":"host pipe response; no calibrated guest UART timing",
                        "supported":if reference.is_some(){"Explicit one-chip computed-work reference policy; physical core, clock and line timing unresolved."}else{"register0 identification, INACTIVE/SETADDRESS, per-chip ticket filter and clock-order/enable register storage; clock routing and status unsupported"},
                        "reference_view_sha256":"501c855b43ecf49a37575766454d1e4e439d0911f1b0c2f5bd2bef287642fe67"
                    })
                );
            }
            let mut total = 0usize;
            let mut sequence = 0u64;
            let mut decoder = md_sim_firmware::bm1397::Decoder::default();
            let mut bytes = [0; 4096];
            let capture_result = (|| {
                loop {
                    if reference.is_some() && stopped.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    if let Some(step) = chain.poll() {
                        batch.record(&step);
                        if step.job_computation_ordinal == 1
                            || step.computation.accepted
                            || step.datapath_dropped
                        {
                            eprintln!(
                                "md-sim ASIC actual computation: {}",
                                json!({
                            "work_id":step.computation.work_id,"wire_job_id":step.job_id,
                            "chip_address":step.chip_address,"nonce":step.computation.nonce,
                            "variant":format!("{:?}",step.computation.variant),"digest":hex::encode(step.computation.digest),
                            "filter":step.computation.filter,"filter_accepted":step.computation.accepted,
                            "datapath_dropped":step.datapath_dropped,"exhausted":step.computation.exhausted,
                            "job_computation_ordinal":step.job_computation_ordinal,"first_for_job":step.job_computation_ordinal==1,
                            "admission_ns":step.computation.admitted_at.duration_since(start).as_nanos(),
                            "completion_ns":step.computation.completed_at.duration_since(start).as_nanos()})
                            );
                        }
                        if let Some(reply) = step.reply {
                            let frame = reply.encode().map_err(|error| {
                                std::io::Error::other(format!("ASIC computed reply: {error:?}"))
                            })?;
                            if reply_pipe.write(&frame)? != frame.len() {
                                return Err(std::io::Error::other("short ASIC computed reply"));
                            }
                            eprintln!(
                                "md-sim ASIC UART RX: {}",
                                json!({"after_capture_sequence":sequence.checked_sub(1),
                            "capture_wall_microseconds":start.elapsed().as_micros(),"hex":hex::encode(frame),"state":reply})
                            );
                        }
                        if batch.due() {
                            if let Some(record) = batch.take(start) {
                                eprintln!("md-sim ASIC computation batch: {record}");
                            }
                        }
                    }
                    match output.read(&mut bytes) {
                        Ok(0) => return Ok(()),
                        Ok(count) => {
                            total += count;
                            if total > 1024 * 1024 {
                                return Err(std::io::Error::other("ASIC UART capture exceeds1MiB"));
                            }
                            eprintln!(
                                "md-sim ASIC UART TX: {}",
                                json!({"sequence":sequence,"capture_wall_microseconds":start.elapsed().as_micros(),"hex":hex::encode(&bytes[..count])})
                            );
                            sequence += 1;
                            for byte in &bytes[..count] {
                                if let Some(packet) = decoder.push(*byte) {
                                    eprintln!(
                                        "md-sim ASIC decoded TX: {}",
                                        json!({"after_capture_sequence":sequence-1,
                                        "capture_wall_microseconds":start.elapsed().as_micros(),
                                        "packet":packet})
                                    );
                                    if identify {
                                        let packet = packet.map_err(|error| {
                                            std::io::Error::other(format!(
                                                "ASIC TX decode failed: {error:?}"
                                            ))
                                        })?;
                                        let received = chain.receive(&packet).map_err(|error| {
                                            eprintln!(
                                                "md-sim ASIC identification boundary: {}",
                                                json!({"error":error,"packet":packet})
                                            );
                                            std::io::Error::other(format!(
                                                "ASIC identification stage boundary: {error:?}"
                                            ))
                                        })?;
                                        if let Some(intent) = received.intent {
                                            eprintln!(
                                                "md-sim ASIC write-intent adaptation: {}",
                                                json!({"after_capture_sequence":sequence-1,"intent":intent,"scope":"One posted firmware write observed; no ASIC acknowledgement, state, timing or success inferred."})
                                            );
                                        }
                                        if control_storage
                                            && matches!(
                                                packet,
                                                md_sim_firmware::bm1397::Packet::Command {
                                                    command:
                                                        md_sim_firmware::bm1397::Command::Write {
                                                            register: 0x3c
                                                                | 0x08
                                                                | 0x68
                                                                | 0x28
                                                                | 0x18
                                                                | 0x70,
                                                            ..
                                                        },
                                                    ..
                                                }
                                            )
                                        {
                                            eprintln!(
                                                "md-sim ASIC programmed controls: {}",
                                                json!({"after_capture_sequence":sequence-1,"chip_position":0,
                                                "state":chain.programmed_controls(0),"effective_clock_known":false,
                                                "guest_readback_available":false,"effective_uart_baud_known":false,"response_bytes":0})
                                            );
                                        }
                                        for reply in received.replies {
                                            let frame = reply.encode().map_err(|error| {
                                                std::io::Error::other(format!(
                                                    "ASIC reply failed: {error:?}"
                                                ))
                                            })?;
                                            // Nine bytes fit atomically in this private FIFO.
                                            let written = reply_pipe.write(&frame)?;
                                            if written != frame.len() {
                                                return Err(std::io::Error::other(
                                                    "short ASIC identification reply",
                                                ));
                                            }
                                            eprintln!(
                                                "md-sim ASIC UART RX: {}",
                                                json!({
                                                    "after_capture_sequence":sequence-1,
                                                    "capture_wall_microseconds":start.elapsed().as_micros(),
                                                    "hex":hex::encode(frame),"state":reply
                                                })
                                            );
                                        }
                                    }
                                }
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            if stopped.load(Ordering::Relaxed) {
                                return Ok(());
                            }
                            let mut poll = libc::pollfd {
                                fd: output.as_raw_fd(),
                                events: libc::POLLIN,
                                revents: 0,
                            };
                            let delay = chain.wait_duration();
                            let timeout = libc::timespec {
                                tv_sec: delay.as_secs() as libc::time_t,
                                tv_nsec: delay.subsec_nanos() as libc::c_long,
                            };
                            let result =
                                unsafe { libc::ppoll(&mut poll, 1, &timeout, std::ptr::null()) };
                            if result < 0 {
                                let error = std::io::Error::last_os_error();
                                if error.kind() != std::io::ErrorKind::Interrupted {
                                    return Err(error);
                                }
                            } else if poll.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
                                return Err(std::io::Error::other(
                                    "ASIC UART capture descriptor error",
                                ));
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(error) => return Err(error),
                    }
                }
            })();
            if let Some(record) = batch.take(start) {
                eprintln!("md-sim ASIC computation batch: {record}");
            }
            if reference.is_some() {
                eprintln!(
                    "md-sim ASIC measurement: {}",
                    json!({"measurement":chain.measurement()})
                );
            }
            capture_result
        });
        Ok(Self {
            stop,
            input,
            thread: Some(thread),
        })
    }
    fn finish(mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        self.thread
            .take()
            .unwrap()
            .join()
            .map_err(|_| anyhow::anyhow!("ASIC UART collector panic"))??;
        Ok(())
    }
}
impl Drop for UartCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // The unread input descriptor remains open until capture teardown.
        // The explicit identification stage can write computed register replies.
        let _ = self.input.as_raw_fd();
    }
}
fn attach_asic_uart(command: &mut Command) -> Result<Option<UartCapture>> {
    if std::env::var_os("MD_SIM_ASIC_UART_TRACE").is_some() {
        let capture = UartCapture::start()?;
        command.args(["-serial", "pipe:/work/asic"]);
        Ok(Some(capture))
    } else {
        Ok(None)
    }
}
fn inner(registers: bool) -> Result<()> {
    confinement::require_board()?;
    ensure!(
        hash_file(Path::new("/original-factory.bin"))?.sha256 == FIRMWARE,
        "factory firmware differs"
    );
    let mut image = fs::read("/original-factory.bin")?;
    ensure!(
        image.len() == 15_802_368 && image[0] == 0xe9,
        "unexpected factory image layout"
    );
    image.resize(16 * 1024 * 1024, 0xff);
    fs::write("/work/flash.bin", &image)?;
    eprintln!(
        "md-sim preparation: {}",
        json!({"original_firmware":FIRMWARE,"derived_flash":hash_file(Path::new("/work/flash.bin"))?,"operation":"Append erased flash bytes 0xff to the declared 16 MiB module capacity. Original bytes unchanged."})
    );
    // QEMU closes the stdio character backend at input EOF. Keep a private
    // input pipe open so its UART can transmit while no user input is queued.
    // This is transport state, not a generated guest byte or boot message.
    let mut pipe = [-1; 2];
    ensure!(
        unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) } == 0,
        "cannot create private UART input pipe"
    );
    let input = unsafe { OwnedFd::from_raw_fd(pipe[0]) };
    let held_input = unsafe { OwnedFd::from_raw_fd(pipe[1]) };
    let mut command = Command::new("/qemu");
    command.stdin(fs::File::from(input)).args([
        "-machine",
        "esp32s3",
        "-accel",
        "tcg,tb-size=64",
        "-m",
        "8M",
        "-global",
        "ssi_psram.is_octal=on",
        "-L",
        "/rom",
        "-drive",
        "file=/work/flash.bin,if=mtd,format=raw",
        "-display",
        "none",
        "-nic",
        "none",
        "-d",
        "cpu_reset",
        "-D",
        "/dev/stderr",
    ]);
    if std::env::var_os("MD_SIM_S3_UART_CONFIG_TRACE").is_some() {
        command.args(["-trace", "enable=esp32s3_uart_config_*"]);
    }
    if std::env::var_os("MD_SIM_SAR_TRACE").is_some() {
        command.args(["-trace", "md_sim_sar_complete"]);
    }
    if std::env::var_os("MD_SIM_I2C_TRACE").is_some() {
        command.args([
            "-trace",
            "md_sim_i2c_bus",
            "-trace",
            "md_sim_i2c_state",
            "-trace",
            "md_sim_i2c_mmio",
        ]);
    }
    if std::env::var_os("MD_SIM_DS4432_FIXTURE").is_some() {
        command.args([
            "-device", "md-sim-ds4432,bus=md-sim-i2c-0,address=0x48,rfs0-ohms=80000,rfs1-ohms=0,supply-uv=3300000",
            "-trace", "md_sim_ds4432_bus",
        ]);
    }
    if std::env::var_os("MD_SIM_EMC2101_FIXTURE").is_some() {
        command.args([
            "-device", "md-sim-emc2101,bus=md-sim-i2c-0,address=0x4c,internal-eighths=216,external-kind=0,host-boot-full=off",
            "-trace", "enable=md_sim_emc2101_*",
        ]);
    }
    if std::env::var_os("MD_SIM_INA260_FIXTURE").is_some() {
        command.args([
            "-device",
            "md-sim-ina260,bus=md-sim-i2c-0,address=0x40,vin-uv=5000000,current-ua=0",
            "-trace",
            "enable=md_sim_ina260_*",
        ]);
    }
    if std::env::var_os("MD_SIM_SAR_INPUT_FIXTURE").is_some() {
        command.args(["-global", "md-sim-esp32s3-sar.adc1-channel1-uv=550000"]);
    }
    let status = if registers {
        // This diagnostic monitor is a private pipe inside the namespace. It
        // stops execution before reading registers and never changes guest
        // memory or supplies a device response. Keep its output separate from
        // real UART bytes.
        command.args(["-monitor", "stdio", "-serial", "file:/work/uart.log"]);
        command.args(["-trace", "memory_region_ops_write"]);
        let capture = attach_asic_uart(&mut command)?;
        command.stderr(Stdio::piped());
        command.stdout(fs::File::create("/work/monitor.log")?);
        let mut child = command.spawn()?;
        let trace_pipe = child.stderr.take().context("missing diagnostic pipe")?;
        let collector = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut bytes = Vec::new();
            trace_pipe
                .take(128 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            Ok(bytes)
        });
        std::thread::sleep(std::time::Duration::from_secs(5));
        let mut monitor = fs::File::from(held_input);
        let commands: &[u8] = if std::env::var_os("MD_SIM_BM1397_IDENTIFY").is_some() {
            // These code addresses were observed in probe027 of this exact
            // firmware. Xtensa's monitor does not implement a $pc register.
            b"stop\ncpu 0\ninfo registers\nx/48i 0x420e0800\nx/24i 0x4208ed00\nx/8wx 0x60008800\ncpu 1\ninfo registers\nx/24i 0x4037a520\nquit\n"
        } else {
            b"stop\ninfo registers\nx/32i 0x420dffb0\nx/32wx 0x60008800\ncpu 1\ninfo registers\nx/16i 0x40041a6c\nquit\n"
        };
        monitor.write_all(commands)?;
        let status = child.wait()?;
        let captured = capture.map_or(Ok(()), UartCapture::finish);
        let mut uart = Vec::new();
        fs::File::open("/work/uart.log")?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut uart)?;
        ensure!(
            uart.len() <= 1024 * 1024,
            "UART diagnostic output exceeded budget"
        );
        std::io::stdout().write_all(&uart)?;
        let trace = collector
            .join()
            .map_err(|_| anyhow::anyhow!("diagnostic collector failed"))??;
        ensure!(
            trace.len() < 128 * 1024 * 1024,
            "raw trace exceeds diagnostic file budget"
        );
        let mut compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        compressed.write_all(&trace)?;
        let compressed = compressed.finish()?;
        ensure!(
            compressed.len() <= 12 * 1024 * 1024,
            "compressed trace exceeds export budget"
        );
        eprintln!(
            "md-sim diagnostic trace: {}",
            json!({"sha256":md_sim_firmware::prepare::hash_bytes(&trace),"size":trace.len(),"gzip_hex":hex::encode(compressed)})
        );
        let mut bytes = Vec::new();
        fs::File::open("/work/monitor.log")?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 1024 * 1024,
            "monitor diagnostic output exceeded budget"
        );
        eprintln!(
            "md-sim monitor commands: {}",
            String::from_utf8_lossy(commands)
        );
        std::io::stderr().write_all(&bytes)?;
        captured?;
        status
    } else {
        command.args(["-monitor", "none", "-serial", "stdio"]);
        // Keep the child C stdio stream separate while the UART collector
        // writes JSON records. Cross-process writes otherwise split records.
        command.stderr(fs::File::create("/work/qemu-stderr.log")?);
        let live_export = std::env::var_os("MD_SIM_BM1397_REFERENCE_POLICY").is_some();
        let mut live_tail = if live_export {
            // Open before child spawn, with an independent file offset.
            Some(stderr_tail::Tail::new(fs::File::open(
                "/work/qemu-stderr.log",
            )?))
        } else {
            None
        };
        let capture = attach_asic_uart(&mut command)?;
        let (status, captured) = if let Some(capture) = capture {
            let mut child = command.spawn()?;
            let status = loop {
                if let Some(tail) = live_tail.as_mut() {
                    if let Err(error) = tail.drain(&mut std::io::stderr().lock()) {
                        eprintln!("md-sim QEMU stderr live error: {error}");
                        child.kill()?;
                        break child.wait()?;
                    }
                }
                if let Some(status) = child.try_wait()? {
                    break status;
                }
                if capture.thread.as_ref().unwrap().is_finished() {
                    child.kill()?;
                    break child.wait()?;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            (status, capture.finish())
        } else {
            (command.status()?, Ok(()))
        };
        let mut backend = Vec::new();
        fs::File::open("/work/qemu-stderr.log")?
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut backend)?;
        ensure!(
            backend.len() <= 16 * 1024 * 1024,
            "QEMU stderr exceeds diagnostic budget"
        );
        if let Some(tail) = live_tail.as_mut() {
            tail.drain(&mut std::io::stderr().lock())?;
            match tail.verified_summary(&backend) {
                Ok(receipt) => eprintln!("md-sim QEMU stderr live final: {receipt}"),
                Err(error) => {
                    eprintln!(
                        "md-sim QEMU stderr live incomplete: {}",
                        json!({"observed":tail.summary(),"error":error.to_string()})
                    );
                    return Err(error.into());
                }
            }
        }
        eprintln!(
            "md-sim separate QEMU stderr: {}",
            json!({"sha256":md_sim_firmware::prepare::hash_bytes(&backend),"size":backend.len(),"live_export":live_export})
        );
        if !live_export {
            std::io::stderr().write_all(&backend)?;
        }
        drop(held_input);
        captured?;
        status
    };
    ensure!(status.success(), "QEMU returned {status}");
    Ok(())
}
struct Options {
    registers: bool,
    ds4432: bool,
    emc2101: bool,
    ina260: bool,
    asic_uart: bool,
    identify: bool,
    sar_input_fixture: bool,
    core0_write_intent: bool,
    control_storage: bool,
    uart_configuration: bool,
    reference_work: Option<probe_model::Config>,
}
fn run(backend: &Path, firmware: &Path, out: &Path, options: Options) -> Result<()> {
    let Options {
        registers,
        ds4432,
        emc2101,
        ina260,
        asic_uart,
        identify,
        sar_input_fixture,
        core0_write_intent,
        control_storage,
        uart_configuration,
        reference_work,
    } = options;
    ensure!(!out.exists(), "new output required");
    fs::create_dir_all(out)?;
    let build: Value = serde_json::from_slice(&fs::read(backend.join("build-report.json"))?)?;
    let assessment: Value = serde_json::from_slice(&fs::read(backend.join("assessment.json"))?)?;
    ensure!(
        assessment["backend_build_passed"] == true && build["source"] == SOURCE,
        "backend build did not pass or source differs"
    );
    let binary = backend.join("qemu-system-xtensa");
    let rom = backend.join("esp32s3_rev0_rom.bin");
    for (path, key) in [(&binary, "binary"), (&rom, "rom")] {
        ensure!(
            hash_file(path)? == serde_json::from_value::<ObjectId>(build[key].clone())?,
            "backend output identity differs"
        );
    }
    let original = hash_file(firmware)?;
    ensure!(
        original.sha256 == FIRMWARE && original.size == 15_802_368,
        "archived factory image differs"
    );
    let store = Store::open(&out.join("inputs"), 64 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/probe".into(), std::env::current_exe()?),
            ("/qemu".into(), binary),
        ],
    )?;
    for (host, guest) in [
        (firmware, "/original-factory.bin"),
        (rom.as_path(), "/rom/esp32s3_rev0_rom.bin"),
    ] {
        let object = hash_file(host)?;
        store.import(host, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    let mut environment = BTreeMap::from([("TMPDIR".into(), "/tmp".into())]);
    if build.get("rtc_sar_model").is_some() {
        environment.insert("MD_SIM_SAR_TRACE".into(), "1".into());
    }
    if build.get("i2c_master_model").is_some() {
        environment.insert("MD_SIM_I2C_TRACE".into(), "1".into());
    }
    if ds4432 {
        ensure!(
            build.get("ds4432_model").is_some(),
            "backend has no DS4432 model"
        );
        environment.insert("MD_SIM_DS4432_FIXTURE".into(), "1".into());
    }
    if emc2101 {
        ensure!(
            build.get("emc2101_model").is_some(),
            "EMC2101 backend model absent"
        );
        environment.insert("MD_SIM_EMC2101_FIXTURE".into(), "1".into());
    }
    if ina260 {
        ensure!(
            build.get("ina260_model").is_some(),
            "INA260 backend model absent"
        );
        environment.insert("MD_SIM_INA260_FIXTURE".into(), "1".into());
    }
    if asic_uart {
        environment.insert("MD_SIM_ASIC_UART_TRACE".into(), "1".into());
    }
    if identify {
        ensure!(
            asic_uart && ina260,
            "identification probe needs UART and declared power inputs"
        );
        environment.insert("MD_SIM_BM1397_IDENTIFY".into(), "1".into());
    }
    if sar_input_fixture {
        ensure!(
            identify,
            "ADC fixture requires the explicit identification stage"
        );
        environment.insert("MD_SIM_SAR_INPUT_FIXTURE".into(), "1".into());
    }
    if core0_write_intent {
        ensure!(
            identify && sar_input_fixture,
            "write-intent observer requires the explicit identification and ADC fixture"
        );
        environment.insert("MD_SIM_BM1397_CORE0_WRITE_INTENT".into(), "1".into());
    }
    if control_storage {
        ensure!(identify && sar_input_fixture && !core0_write_intent,
            "control storage requires the reference identification/ADC profile and excludes the observer");
        environment.insert("MD_SIM_BM1397_CONTROL_STORAGE".into(), "1".into());
    }
    if uart_configuration {
        ensure!(
            control_storage && build["uart_configuration_model"].is_object(),
            "UART observations require the posted-control profile and S3 UART configuration backend"
        );
        environment.insert("MD_SIM_S3_UART_CONFIG_TRACE".into(), "1".into());
    }
    let adaptation = if let Some(config) = &reference_work {
        ensure!(
            control_storage && uart_configuration && !core0_write_intent,
            "computed reference work needs the explicit control/ADC/UART profile"
        );
        let model = config
            .build()
            .map_err(|error| anyhow::anyhow!("reference policy: {error:?}"))?;
        environment.insert(
            "MD_SIM_BM1397_REFERENCE_POLICY".into(),
            serde_json::to_string(config)?,
        );
        json!({"id":md_sim_firmware::bm1397::reference::POLICY,"kind":"reference-device-adaptation",
            "configuration":config,"policy":model.policy(),
            "trace_policy":"Live complete QEMU stderr lines through one locked writer; final receipt required for complete export. First actual computation of each submitted job, plus all solutions/faults, retained with real nonce/digest; no additional hash computation.",
            "source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../src/bm1397/reference.rs")),
            "engine_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../../md-sim-mining/src/engine.rs")),
            "scope":"Actual software hashing at the programmed filter; reference queue, chip routing, rate windows and optional computation-drop faults. Factory assertions and firmware bytes unchanged; physical core/clock and multi-chip behavior unresolved.",
            "original_board_gate":false,"functional_mining_gate":false})
    } else if control_storage {
        json!({
            "id":"bm1397-programmed-control-storage-v3","kind":"reference-device-capability",
            "capability":md_sim_firmware::bm1397::ControlWriteSupport::RegisterStorage,
            "scope":"Per-chip posted core0, PLL0/PLL3, PLL0 output divider, FastUART and MiscControl configuration. Reset/power invalidate unknown defaults. No effective core fan-out, clock, lock, UART baud, completion or guest readback is inferred. No write acknowledgement is sent.",
            "source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../src/bm1397/programmed.rs")),
            "original_arithmetic_fixture_sha256":"9fd7c05d9f83cdab32094688708aac380fd9a5997a2493be0a2515e6af17b13c",
            "uart_view_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../src/bm1397/uart_control.rs")),
            "original_board_gate":false,"functional_mining_gate":false
        })
    } else if core0_write_intent {
        json!({
            "id":write_intent::ADAPTATION,"kind":"reference-environment-observer",
            "command_hex":"55aa5109003c8000807410","maximum_observations":1,
            "response_bytes":0,"silicon_effects_modeled":false,
            "scope":"Continue once after the exact posted core0 write to discover the next original firmware command. Core state, readback, clock behavior, timing and mining remain unsupported.",
            "source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/bm1397_write_intent.rs")),
            "original_board_gate":false,"functional_mining_gate":false
        })
    } else {
        Value::Null
    };
    save_json(&out.join("adaptation.json"), &adaptation)?;
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec![
            "/probe".into(),
            if registers {
                "__inside-registers"
            } else {
                "__inside"
            }
            .into(),
        ],
        inputs,
        stdin_object: None,
        environment,
        limits: Limits {
            wall_seconds: 60,
            cpu_seconds: 45,
            memory_bytes: 1024 * 1024 * 1024,
            processes: 64,
            file_bytes: 32 * 1024 * 1024,
            trace_bytes: if registers {
                64 * 1024 * 1024
            } else {
                8 * 1024 * 1024
            },
            tmp_bytes: if registers {
                256 * 1024 * 1024
            } else {
                64 * 1024 * 1024
            },
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("input-identities.json"),
        &json!({"firmware":original,"source":SOURCE,"recipe":hash_file(&std::env::current_exe()?)?,"backend_build_report":hash_file(&backend.join("build-report.json"))?,"backend_spec":hash_file(&backend.join("spec.json"))?,"scope":"Original archived factory firmware and ROM on an upstream ESP32-S3 machine; no claim of complete Bitaxe device models.","storage_adaptation":"Erase-fill trailing16MiB flash capacity in private scratch; preserve all original factory bytes.","register_diagnostic":registers,"sar_external_fixture":if sar_input_fixture {json!({"unit":1,"channel":1,"microvolts":550000,"scope":"Independent fixed test input; no regulator coupling or physical core-voltage claim."})}else{Value::Null},"ds4432_register_fixture":ds4432,"ds4432_scope":"When selected: explicit 3.3V supply, OUT0 80kOhm FS resistor, OUT1 disconnected. No modeled regulator or ASIC-voltage telemetry; exact Max R14 remains unresolved.","emc2101_register_fixture":emc2101,"emc2101_scope":"When selected: host-load0% strap, explicit internal27C test input, external diode open, no connected fan tachometer. These are declared fixture inputs, not measured physical board telemetry.","ina260_register_fixture":ina260,"ina260_scope":"When selected: explicit5V VIN and0A current diagnostic input. No physical PSU or ASIC load is inferred; no core-voltage measurement.","uart_configuration_observation":uart_configuration,"uart_configuration_scope":"When selected: bounded programmed-register observations with device identity and QEMU virtual time. Overflow is incomplete capture. No effective clock, baud, line timing or physical calibration is inferred.","uart_configuration_parser_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/esp32_uart_trace.rs")),"asic_uart_tx_capture":asic_uart,"asic_uart_scope":if reference_work.is_some() {"Explicit one-chip computed-work reference capability. Actual engine hashes and optional named fault events; no GPIO/power coupling, calibrated line timing, physical clock/core map, or factory-gate bypass."} else if identify {"Identification, addresses and clock-control register storage: one explicitly powered, reset-released reference chip; no GPIO/regulator binding or calibrated guest UART timing. Per-chip state follows writes and reset. Clock routing, status and all other unsupported commands remain visible failures. No mining initialization or nonce response."} else {"When selected: private second UART output capture only; no response bytes or ASIC model. Capture timestamps use host wall time, not calibrated guest UART timing."},"core0_write_intent_adaptation":if core0_write_intent {adaptation.clone()}else{Value::Null},"programmed_control_profile":if control_storage {adaptation.clone()}else{Value::Null},"reference_work_adaptation":if reference_work.is_some(){adaptation.clone()}else{Value::Null},"bm1397_identification_stage":identify,"bm1397_identification_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../src/bm1397/identify.rs")),"bm1397_reply_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../src/bm1397/reply.rs"))}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    let stderr = hex::decode(&result.stderr)?;
    fs::write(out.join("guest.log"), &stdout)?;
    fs::write(out.join("backend.log"), &stderr)?;
    let live_final = if reference_work.is_some() {
        let final_lines: Vec<_> = std::str::from_utf8(&stderr)?
            .lines()
            .filter_map(|line| line.strip_prefix("md-sim QEMU stderr live final: "))
            .collect();
        ensure!(
            final_lines.len() <= 1,
            "duplicate live QEMU stderr final receipt"
        );
        let final_record: Option<Value> = final_lines
            .first()
            .map(|text| serde_json::from_str(text))
            .transpose()?;
        let complete =
            final_record.as_ref().is_some_and(|r| r["complete"] == true) && !result.trace_truncated;
        save_json(
            &out.join("qemu-stderr-retention.json"),
            &json!({
            "process_status":result.status,"exit_code":result.exit_code,"trace_truncated":result.trace_truncated,
            "final_receipt":final_record,"final_export_complete":complete,
            "partial_line_status":if final_record.is_none(){"Unknown: process ended before final receipt; complete emitted lines are retained."}else{"See exact final receipt."},
            "scope":"Run-owned file; complete byte lines emitted under the Rust stderr lock. Missing final receipt cannot establish complete QEMU output."}),
        )?;
        complete
    } else {
        true
    };
    if uart_configuration {
        let mut observations =
            uart_trace::parse(std::str::from_utf8(&stderr)?).with_context(|| {
                format!(
                    "UART observations unavailable; actual process status {}, exit {:?}",
                    result.status, result.exit_code
                )
            })?;
        observations.complete &= !result.trace_truncated && live_final;
        save_json(
            &out.join("uart-configuration-observations.json"),
            &observations,
        )?;
        ensure!(
            observations.complete,
            "UART configuration capture incomplete (overflow, truncation or missing final stderr receipt); actual process status {}, exit {:?}", result.status, result.exit_code
        );
        ensure!(
            observations.devices.contains_key("/machine/soc/uart1"),
            "ASIC controller UART observations absent"
        );
    }
    if registers {
        let text = String::from_utf8_lossy(&stderr);
        if let Some(line) = text
            .lines()
            .find_map(|l| l.strip_prefix("md-sim diagnostic trace: "))
        {
            let record: Value = serde_json::from_str(line)?;
            let gzip = hex::decode(
                record["gzip_hex"]
                    .as_str()
                    .context("missing compressed trace")?,
            )?;
            let mut trace = Vec::new();
            flate2::read::GzDecoder::new(gzip.as_slice())
                .take(128 * 1024 * 1024 + 1)
                .read_to_end(&mut trace)?;
            ensure!(
                trace.len() < 128 * 1024 * 1024
                    && record["size"].as_u64() == Some(trace.len() as u64)
                    && record["sha256"] == md_sim_firmware::prepare::hash_bytes(&trace),
                "diagnostic trace identity differs"
            );
            fs::write(out.join("qemu-trace.log.gz"), gzip)?;
            let selected = String::from_utf8_lossy(&trace)
                .lines()
                .filter(|l| l.contains("name 'esp32s3.iomem'"))
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(out.join("unsupported-mmio-writes.log"), selected)?;
            save_json(
                &out.join("diagnostic-trace-identity.json"),
                &json!({"raw_sha256":record["sha256"],"raw_size":trace.len(),"compressed":hash_file(&out.join("qemu-trace.log.gz"))?,"selection":"Exact lines with name 'esp32s3.iomem'; raw complete trace retained as gzip."}),
            )?;
        }
    }
    if reference_work.is_some() {
        ensure!(result.status == "exited" && result.exit_code == Some(0) && live_final,
            "reference probe incomplete: actual process status {}, exit {:?}, complete QEMU stderr export {}",
            result.status, result.exit_code, live_final);
    }
    let log = String::from_utf8_lossy(&stdout);
    save_json(
        &out.join("assessment.json"),
        &json!({"probe_finished":true,"process_status":result.status,"exit_code":result.exit_code,"trace_truncated":result.trace_truncated,"rom_banner_observed":log.lines().any(|s|s.trim()=="ESP-ROM:esp32s3-20210327"),"complete_original_board_boot":false,"native_dashboard":false,"functional_mining":false,"limitations":["ASIC UART, regulator and complete support-device models remain incomplete.","Physical ESP32 Wi-Fi behavior is not provided by the virtual OpenETH interface.","Logs must be reviewed to establish the actual guest execution stage."]}),
    )?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args
        .get(1)
        .is_some_and(|a| a == "__inside" || a == "__inside-registers")
    {
        return inner(args[1] == "__inside-registers");
    }
    ensure!(
        (5..=7).contains(&args.len()) && args[4] == "bm1397-reference-work-adc-uart-config" || args.len() == 4 || (args.len() == 5 && (args[4] == "registers" || args[4] == "ds4432" || args[4] == "emc2101" || args[4] == "asic-trace" || args[4] == "power-asic-trace" || args[4] == "bm1397-identify" || args[4] == "bm1397-identify-registers" || args[4] == "bm1397-identify-adc" || args[4] == "bm1397-observe-core0-adc" || args[4] == "bm1397-control-storage-adc" || args[4] == "bm1397-control-storage-adc-uart-config")),
        "usage: esp32_original_probe BACKEND_BUILD FACTORY_FIRMWARE NEW_OUTPUT [registers|ds4432|emc2101|asic-trace|power-asic-trace|bm1397-identify|bm1397-identify-registers|bm1397-identify-adc|bm1397-observe-core0-adc|bm1397-control-storage-adc|bm1397-control-storage-adc-uart-config|bm1397-reference-work-adc-uart-config [HASH_CAP_HPS [FAULT_EVERY_N]]]"
    );
    let mode = args.get(4).and_then(|a| a.to_str());
    let reference_mode = mode == Some("bm1397-reference-work-adc-uart-config");
    let reference_work = if reference_mode {
        let cap = args
            .get(5)
            .map(|s| {
                s.to_str()
                    .context("UTF-8 hash cap")?
                    .parse::<f64>()
                    .context("hash cap")
            })
            .transpose()?
            .unwrap_or(10_000.0);
        let fault = if let Some(n) = args.get(6) {
            md_sim_firmware::bm1397::reference::FaultPolicy::DropEveryNthComputation {
                interval: n
                    .to_str()
                    .context("UTF-8 fault interval")?
                    .parse()
                    .context("fault interval")?,
            }
        } else {
            md_sim_firmware::bm1397::reference::FaultPolicy::NoFaults
        };
        let config = probe_model::Config {
            chips: 1,
            hash_cap_hps: cap,
            queue_slots: 32,
            fault_policy: fault,
        };
        config
            .build()
            .map_err(|error| anyhow::anyhow!("reference policy: {error:?}"))?;
        Some(config)
    } else {
        None
    };
    let identify = reference_mode
        || matches!(
            mode,
            Some(
                "bm1397-identify"
                    | "bm1397-identify-registers"
                    | "bm1397-identify-adc"
                    | "bm1397-observe-core0-adc"
                    | "bm1397-control-storage-adc"
                    | "bm1397-control-storage-adc-uart-config"
            )
        );
    let asic_uart = identify || matches!(mode, Some("asic-trace" | "power-asic-trace"));
    let out = PathBuf::from(&args[3]);
    let new = !out.exists();
    let result = run(
        Path::new(&args[1]),
        Path::new(&args[2]),
        &out,
        Options {
            registers: matches!(mode, Some("registers" | "bm1397-identify-registers")),
            ds4432: mode.is_some_and(|m| m != "registers"),
            emc2101: asic_uart || mode == Some("emc2101"),
            ina260: identify || mode == Some("power-asic-trace"),
            asic_uart,
            identify,
            sar_input_fixture: reference_mode
                || matches!(
                    mode,
                    Some(
                        "bm1397-identify-adc"
                            | "bm1397-observe-core0-adc"
                            | "bm1397-control-storage-adc"
                            | "bm1397-control-storage-adc-uart-config"
                    )
                ),
            core0_write_intent: mode == Some("bm1397-observe-core0-adc"),
            control_storage: reference_mode
                || matches!(
                    mode,
                    Some("bm1397-control-storage-adc" | "bm1397-control-storage-adc-uart-config")
                ),
            uart_configuration: reference_mode
                || mode == Some("bm1397-control-storage-adc-uart-config"),
            reference_work,
        },
    );
    if let Err(error) = &result {
        if new && out.is_dir() {
            save_json(&out.join("assessment.json"),&json!({"probe_finished":false,"complete_original_board_boot":false,"functional_mining":false,"error":format!("{error:#}")})).context("save failed probe evidence")?;
        }
    }
    result
}
