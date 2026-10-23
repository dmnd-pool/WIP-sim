//! Independent pool service in a captured Linux VM on a private Ethernet link.
//! The optional client mode tests transport only; it is not miner firmware.
use anyhow::{ensure, Context, Result};
use md_sim_mining::stratum::{serve_tcp_window, DifficultyScale, StratumConfig};
use md_sim_runtime::store::hash_file;
use serde_json::json;
use std::{
    ffi::CString,
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[path = "support/s9_browser_pool.rs"]
mod browser_pool;
#[path = "support/s9_dashboard_probe.rs"]
mod dashboard_probe;
#[allow(dead_code)] // This executable reads the shared contract; the host freezes it.
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[allow(dead_code)] // This service reads only the captured shared-pool contract.
#[path = "support/s9_shared_pool.rs"]
mod shared_pool;
#[path = "support/s9_shared_pool_service.rs"]
mod shared_service;

const MODULE: &str = "/e1000.ko.zst";
const MODULE_HASH: &str = "6542f4d97c2e1424dd65b8ce6aa667bdc1a25d542c6dce521f5593ba69f2f210";
const MARKER: &str = "MD_SIM_PRIVATE_POOL_RESULT ";
// Transport fixture only. Cross the legacy sixteen-connection lifetime bound.
const TRANSPORT_CONNECTIONS: usize = 32;

fn configure_network(client: bool) -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0,
        "pool VM network setup requires guest root"
    );
    ensure!(
        fs::read_to_string("/proc/sys/kernel/osrelease")?.trim() == "7.2.9-arch1-1",
        "pool guest kernel differs from captured module"
    );
    ensure!(
        hash_file(Path::new(MODULE))?.sha256 == MODULE_HASH,
        "pool network module differs"
    );
    let decoded = Command::new("/usr/bin/zstd")
        .args(["--decompress", "--stdout", MODULE])
        .output()?;
    ensure!(
        decoded.status.success() && decoded.stdout.len() <= 2 * 1024 * 1024,
        "module decompression failed or exceeded bound"
    );
    fs::write("/work/e1000.ko", &decoded.stdout)?;
    let module = File::open("/work/e1000.ko")?;
    let parameters = CString::new("")?;
    ensure!(
        unsafe {
            libc::syscall(
                libc::SYS_finit_module,
                module.as_raw_fd(),
                parameters.as_ptr(),
                0,
            )
        } == 0,
        "pool network module load: {}",
        std::io::Error::last_os_error()
    );
    let mut interfaces = fs::read_dir("/sys/class/net")?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>>>()?;
    interfaces.sort();
    ensure!(
        interfaces == ["eth0", "lo"],
        "unexpected pool VM interfaces: {interfaces:?}"
    );
    ensure!(
        fs::read_to_string("/sys/class/net/eth0/device/vendor")?.trim() == "0x8086"
            && fs::read_to_string("/sys/class/net/eth0/device/device")?.trim() == "0x100e",
        "unexpected private Ethernet device"
    );
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    ensure!(fd >= 0, "pool network control socket failed");
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
    for (target, byte) in request.ifr_name.iter_mut().zip(b"eth0") {
        *target = *byte as i8;
    }
    let address = if client {
        Ipv4Addr::new(10, 0, 2, 15)
    } else {
        Ipv4Addr::new(10, 0, 2, 100)
    };
    for (operation, ip) in [
        (libc::SIOCSIFADDR, address),
        (libc::SIOCSIFNETMASK, Ipv4Addr::new(255, 255, 255, 0)),
    ] {
        let address = libc::sockaddr_in {
            sin_family: libc::AF_INET as libc::sa_family_t,
            sin_port: 0,
            sin_addr: libc::in_addr {
                s_addr: u32::from_ne_bytes(ip.octets()),
            },
            sin_zero: [0; 8],
        };
        request.ifr_ifru.ifru_addr =
            unsafe { std::mem::transmute::<libc::sockaddr_in, libc::sockaddr>(address) };
        ensure!(
            unsafe { libc::ioctl(fd.as_raw_fd(), operation, &request) } == 0,
            "pool address setup failed"
        );
    }
    ensure!(
        unsafe { libc::ioctl(fd.as_raw_fd(), libc::SIOCGIFFLAGS, &mut request) } == 0,
        "interface flag read failed"
    );
    unsafe {
        request.ifr_ifru.ifru_flags |= libc::IFF_UP as i16;
    }
    ensure!(
        unsafe { libc::ioctl(fd.as_raw_fd(), libc::SIOCSIFFLAGS, &request) } == 0,
        "interface enable failed"
    );
    for line in fs::read_to_string("/proc/net/route")?.lines().skip(1) {
        ensure!(
            line.split_whitespace().nth(1) != Some("00000000"),
            "pool VM must have no default route"
        );
    }
    println!(
        "MD_SIM_POOL_NETWORK {}",
        json!({
            "address":address,"interfaces":interfaces,"module":hash_file(Path::new(MODULE))?,
            "decoded_module":hash_file(Path::new("/work/e1000.ko"))?,
            "routes":fs::read_to_string("/proc/net/route")?,
            "external_physical_host":false,"host_network":false
        })
    );
    Ok(())
}

fn drop_privileges() -> Result<()> {
    let work = CString::new("/work")?;
    ensure!(
        unsafe { libc::chown(work.as_ptr(), 1000, 1000) } == 0,
        "pool work ownership failed"
    );
    for (resource, maximum) in [
        (libc::RLIMIT_AS, 256 * 1024 * 1024),
        (libc::RLIMIT_FSIZE, 2 * 1024 * 1024),
        (libc::RLIMIT_NPROC, 32),
        (libc::RLIMIT_CPU, 30),
        (libc::RLIMIT_NOFILE, 64),
    ] {
        let limit = libc::rlimit {
            rlim_cur: maximum,
            rlim_max: maximum,
        };
        ensure!(
            unsafe { libc::setrlimit(resource, &limit) } == 0,
            "pool resource bound failed"
        );
    }
    ensure!(
        unsafe { libc::setgroups(0, std::ptr::null()) } == 0
            && unsafe { libc::setgid(1000) } == 0
            && unsafe { libc::setuid(1000) } == 0
            && unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } == 0,
        "pool privilege drop failed"
    );
    let status = fs::read_to_string("/proc/self/status")?;
    ensure!(
        status
            .lines()
            .any(|line| line == "CapEff:\t0000000000000000"),
        "pool retained capabilities"
    );
    Ok(())
}

fn server(input: pool_input::PoolInput, shared: bool) -> Result<()> {
    let session_limit = input.session_limit()?;
    let listener = TcpListener::bind("10.0.2.100:3333")?;
    listener.set_nonblocking(true)?;
    drop_privileges()?;
    println!(
        "MD_SIM_POOL_READY {}",
        json!({
            "endpoint":"10.0.2.100:3333","input":hash_file(Path::new("/private-pool.json"))?,
            "verifier":hash_file(&std::env::current_exe()?)?
        })
    );
    let deadline = Instant::now() + Duration::from_secs(input.seconds);
    let dashboard = if Path::new("/dashboard-probe").exists() {
        ensure!(
            fs::read("/dashboard-probe")? == b"md-sim-original-s9-dashboard-probe/1\n",
            "unknown dashboard contract"
        );
        Some(std::thread::spawn(move || dashboard_probe::run(deadline)))
    } else {
        None
    };
    let stop = Arc::new(AtomicBool::new(false));
    let mut sessions: Vec<(usize, std::thread::JoinHandle<Result<()>>)> = Vec::new();
    let mut allocated = 0;
    let mut cancelled = false;
    let mut incomplete = Vec::new();
    while Instant::now() < deadline {
        if shared && shared_service::cancelled()? {
            cancelled = true;
            incomplete.extend(
                sessions
                    .iter()
                    .filter(|(_, session)| !session.is_finished())
                    .map(|(id, _)| *id),
            );
            stop.store(true, Ordering::Release);
            break;
        }
        // Joining finished threads releases their stacks and retained runtime
        // state. A long-lived pool must bound active resources, not keep every
        // completed JoinHandle until its entire observation window ends.
        let mut index = 0;
        while index < sessions.len() {
            if sessions[index].1.is_finished() {
                sessions
                    .swap_remove(index)
                    .1
                    .join()
                    .map_err(|_| anyhow::anyhow!("pool VM session panicked"))??;
            } else {
                index += 1;
            }
        }
        let (stream, peer) = match listener.accept() {
            Ok(pair) => pair,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        ensure!(
            allocated < session_limit,
            "pool VM connection budget exhausted"
        );
        ensure!(
            sessions.len() < 16,
            "pool VM active connection budget exhausted"
        );
        ensure!(
            peer.ip() == Ipv4Addr::new(10, 0, 2, 15),
            "unexpected pool peer"
        );
        let id = allocated;
        let seed = input.session_seed(id)?;
        let input = input.clone();
        let signal = stop.clone();
        sessions.push((id, std::thread::Builder::new().name(format!("pool-{id}")).spawn(move || -> Result<()> {
            let path = format!("/work/session-{id}.jsonl");
            let config = StratumConfig {
                response_policy: input.response_policy()?,
                regtest_anchor: input.regtest_anchor.clone(),
                seed,
                time: input.time,
                difficulty_exponents: input.difficulty_exponents.to_vec(),
                difficulty_scale: DifficultyScale::One,
                timeout: deadline.saturating_duration_since(Instant::now()).max(Duration::from_nanos(1)),
                trace_path: path.clone().into(),
                limits: md_sim_mining::stratum::StratumLimits {
                    max_messages: 2048,max_submissions: 1024,trace_bytes: 512 * 1024,
                },
            };
            let report = serve_tcp_window(stream, config, signal).map_err(anyhow::Error::msg)?;
            let mut trace = String::new();
            File::open(path)?.take(512 * 1024 + 1).read_to_string(&mut trace)?;
            ensure!(trace.len() <= 512 * 1024, "pool trace bound");
            let evidence = json!({
                "schema":"md-sim-s9-private-pool-session/2","session_id":id,
                "network":"Independent Linux pool VM; private Ethernet stream to miner VM; no external interface or host listener.",
                "peer":peer,"seed_hex":hex::encode(seed),"job_time":input.time,
                "report":report,"trace":trace,"source":{"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_pool_guest.rs"))},
                "scope":"Independent pool protocol evidence. No firmware, exact-target or regtest gate is inferred."
            });
            println!("{MARKER}{evidence}");
            Ok(())
        }).context("pool VM session thread creation failed")?));
        allocated += 1;
    }
    // Each session has its own remaining window. Let it finish that window
    // and retain its real result. Closing a successful two-job fixture early
    // caused repeated native reconnects and exhausted the connection budget.
    for (_, session) in sessions {
        session
            .join()
            .map_err(|_| anyhow::anyhow!("pool VM session panicked"))??;
    }
    let mut collection = json!({"sessions_completed":allocated - incomplete.len(),
        "incomplete_sessions":incomplete,"max_sessions":session_limit});
    if shared {
        collection["cancelled"] = json!(cancelled);
    }
    println!("MD_SIM_PRIVATE_POOL_COLLECTION {collection}");
    if let Some(dashboard) = dashboard {
        dashboard
            .join()
            .map_err(|_| anyhow::anyhow!("native dashboard probe panicked"))??;
    }
    Ok(())
}

fn connect() -> Result<TcpStream> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let error = match TcpStream::connect_timeout(
            &"10.0.2.100:3333".parse()?,
            Duration::from_millis(100),
        ) {
            Ok(stream) => {
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                return Ok(stream);
            }
            Err(error) => error,
        };
        if Instant::now() >= deadline {
            println!(
                "MD_SIM_POOL_NETWORK_FAILURE {}",
                json!({
                    "error":error.to_string(),"devices":fs::read_to_string("/proc/net/dev")?,
                    "arp":fs::read_to_string("/proc/net/arp")?,"routes":fs::read_to_string("/proc/net/route")?,
                    "carrier":fs::read_to_string("/sys/class/net/eth0/carrier")?,
                    "state":fs::read_to_string("/sys/class/net/eth0/operstate")?
                })
            );
            anyhow::bail!("private pool connection timed out: {error}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn read_message(reader: &mut BufReader<TcpStream>) -> Result<serde_json::Value> {
    let mut bytes = Vec::new();
    reader.take(65537).read_until(b'\n', &mut bytes)?;
    ensure!(
        bytes.len() <= 65536 && bytes.last() == Some(&b'\n'),
        "pool response framing"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

fn client(input: &pool_input::PoolInput) -> Result<()> {
    ensure!(
        input.session_limit()? > TRANSPORT_CONNECTIONS,
        "transport fixture needs one binary probe and32 V1 connections"
    );
    drop_privileges()?;
    let mut probe = connect()?;
    probe.write_all(b"\x09\x00STR2\x01AESG")?;
    let mut evidence = Vec::new();
    for session in 0..TRANSPORT_CONNECTIONS {
        let mut reader = BufReader::new(connect()?);
        writeln!(
            reader.get_mut(),
            "{}",
            json!({"id":1,"method":"mining.subscribe","params":[]})
        )?;
        ensure!(
            read_message(&mut reader)?["id"] == 1,
            "subscription response differs"
        );
        writeln!(
            reader.get_mut(),
            "{}",
            json!({"id":2,"method":"mining.authorize","params":["simulator","x"]})
        )?;
        ensure!(
            read_message(&mut reader)?["result"] == true,
            "authorization failed"
        );
        ensure!(
            read_message(&mut reader)?["method"] == "mining.set_difficulty",
            "difficulty absent"
        );
        let job = read_message(&mut reader)?;
        ensure!(job["method"] == "mining.notify", "fresh job absent");
        writeln!(
            reader.get_mut(),
            "{}",
            json!({"id":3,"method":"mining.submit","params":[]})
        )?;
        let rejection = read_message(&mut reader)?;
        ensure!(
            rejection
                == json!({"id":3,"result":null,
                "error":[20,"unsupported submit parameter count",null]}),
            "invalid submission did not receive the exact JSON-RPC1 error"
        );
        evidence
            .push(json!({"connection":session,"job":job,"invalid_submission_response":rejection}));
    }
    drop(probe);
    println!(
        "MD_SIM_POOL_LINK_CLIENT {}",
        json!({
            "connections":evidence,"binary_probe_on_separate_connection":true,
            "reconnect":true,"mining_gate":false
        })
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    let server_child = args.len() == 2 && args[1] == "--server";
    let shared_server = args.len() == 2 && args[1] == "--shared-server";
    ensure!(
        args.len() == 1 || server_child || shared_server,
        "unknown pool service command"
    );
    let command = fs::read_to_string("/proc/cmdline")?;
    ensure!(
        command
            .split_whitespace()
            .any(|word| word == "md_sim_private_guest=1"),
        "captured pool VM required"
    );
    let client_role = command
        .split_whitespace()
        .any(|word| word == "md_sim_pool_role=client");
    fs::create_dir_all("/work")?;
    let input = pool_input::read(Path::new("/private-pool.json"))?;
    if shared_server {
        ensure!(
            !client_role
                && !Path::new("/boser-browser").exists()
                && !Path::new("/dashboard-probe").exists(),
            "shared pool cannot run a different client or internal dashboard suite"
        );
        shared_service::require()?;
        return server(input, true);
    }
    if server_child {
        ensure!(
            !client_role && Path::new("/boser-browser").is_file(),
            "browser supervisor contract absent"
        );
        return server(input, false);
    }
    configure_network(client_role)?;
    if client_role {
        client(&input)
    } else if Path::new("/boser-browser").is_file() {
        browser_pool::run()
    } else {
        server(input, false)
    }
}
