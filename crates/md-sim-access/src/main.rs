use anyhow::{ensure, Context, Result};
use md_sim_access::{
    carrier::{ProviderBinding, Selection},
    carrier_gateway::{self, CarrierGateway, Control},
    carrier_inputs, credentials,
    gateway::{Gateway, GatewaySpec},
    selected::SelectedGateway,
};
use md_sim_coordinator::{LockedRun, WorkerTransport};
use md_sim_runtime::{isolation::save_json, store::ObjectId};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Release);
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    Ok(serde_json::from_slice(&read_bytes(path, limit)?)?)
}
fn read_bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("read access input {}", path.display()))?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= limit,
        "unsafe or oversized access input"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "access input grew beyond limit"
    );
    Ok(bytes)
}

fn fetch_carrier_context(
    socket: &Path,
    token: &str,
    objects: [&ObjectId; 3],
    output: &Path,
) -> Result<ProviderBinding> {
    fs::DirBuilder::new().mode(0o700).create(output)?;
    let inputs = output.join("inputs");
    fs::DirBuilder::new().mode(0o700).create(&inputs)?;
    let mut fetched = Vec::new();
    let mut transport = ConnectedControl(Control::new(socket, Duration::from_secs(5))?);
    for ((name, limit), expected) in [
        ("locked-run.json", 4 << 20),
        ("policy.json", 65536),
        ("provider.json", 4 << 20),
    ]
    .into_iter()
    .zip(objects)
    {
        let path = inputs.join(name);
        carrier_inputs::fetch(&mut transport, expected, &path, limit)?;
        let bytes = read_bytes(&path, limit)?;
        let actual = ObjectId {
            sha256: hex::encode(Sha256::digest(&bytes)),
            size: bytes.len() as u64,
        };
        ensure!(
            actual == *expected,
            "fetched carrier metadata changed before use"
        );
        fetched.push(bytes);
    }
    let run: LockedRun = serde_json::from_slice(&fetched[0])?;
    let selected = Selection::new(&run, objects[1].clone(), &fetched[1], token)?;
    let binding = ProviderBinding::from_config(selected, objects[2].clone(), &fetched[2])?;
    save_json(
        &output.join("input-objects.json"),
        &json!({
        "locked_run":objects[0],"policy":objects[1],"provider":objects[2],
        "retrieval":"Actual authenticated worker Get, exact size and SHA-256 at destination. No native worker store is mounted."}),
    )?;
    Ok(binding)
}

struct ConnectedControl(Control);
impl WorkerTransport for ConnectedControl {
    fn request(&mut self, request: &md_sim_runtime::worker::Request) -> Result<serde_json::Value> {
        ensure!(
            !STOP.load(Ordering::Acquire),
            "connected access startup cancelled"
        );
        let value = self.0.request(request)?;
        ensure!(
            !STOP.load(Ordering::Acquire),
            "connected access startup cancelled"
        );
        Ok(value)
    }
    fn description(&self) -> String {
        self.0.description()
    }
}

fn connected_stop() {
    std::thread::spawn(|| {
        let mut byte = [0u8];
        loop {
            match std::io::stdin().read(&mut byte) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                _ => {
                    STOP.store(true, Ordering::Release);
                    break;
                }
            }
        }
    });
}
fn signals() {
    unsafe {
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
    }
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len()>=2,"usage: md-sim-access credentials NEW_DIRECTORY IP RUN_SHA256 | serve SPEC_JSON NEW_DIRECTORY | serve-selected[-connected] SPEC_JSON LOCKED_RUN_JSON WORKER_SOCKET NEW_DIRECTORY | inspect-carrier LOCK_ID POLICY_ID PROVIDER_ID TOKEN SOCKET READY ENDPOINT NEW_OUTPUT | serve-carrier-connected SPEC_ID LOCK_ID POLICY_ID PROVIDER_ID TOKEN SOCKET READY ENDPOINT NEW_OUTPUT");
    match args[1].as_str() {
        "inspect-carrier" | "serve-carrier-connected" => {
            let publish = args[1] == "serve-carrier-connected";
            ensure!(args.len() == if publish { 11 } else { 10 },
                "inspect-carrier LOCK_ID POLICY_ID PROVIDER_ID TOKEN SOCKET READY ENDPOINT NEW_OUTPUT; serve-carrier-connected adds SPEC_ID before LOCK_ID");
            let offset = usize::from(publish);
            let object = |index: usize| carrier_inputs::parse_object(args[index + offset].as_str());
            let locked = object(2)?;
            let policy = object(3)?;
            let provider = object(4)?;
            let token = &args[5 + offset];
            let socket = Path::new(&args[6 + offset]);
            let ready = Path::new(&args[7 + offset]);
            let endpoint = Path::new(&args[8 + offset]);
            let output = Path::new(&args[9 + offset]);
            if publish {
                signals();
                connected_stop();
            }
            let binding =
                fetch_carrier_context(socket, token, [&locked, &policy, &provider], output)?;
            if !publish {
                let mut record = carrier_gateway::inspect(socket, &binding, ready, endpoint)?;
                record["input_objects"] =
                    json!({"locked_run":locked,"policy":policy,"provider":provider});
                save_json(&output.join("inspection.json"), &record)?;
                println!("{record}");
                return Ok(());
            }
            let spec_object = carrier_inputs::parse_object(&args[2])?;
            let path = output.join("inputs/gateway.json");
            carrier_inputs::fetch(
                &mut ConnectedControl(Control::new(socket, Duration::from_secs(5))?),
                &spec_object,
                &path,
                65536,
            )?;
            let bytes = read_bytes(&path, 65536)?;
            ensure!(
                ObjectId {
                    sha256: hex::encode(Sha256::digest(&bytes)),
                    size: bytes.len() as u64
                } == spec_object,
                "gateway metadata changed before use"
            );
            let spec: GatewaySpec = serde_json::from_slice(&bytes)?;
            let mut gateway = CarrierGateway::start_with_cancel(
                socket,
                binding,
                ready,
                endpoint,
                &spec,
                &output.join("tls"),
                &STOP,
            )?;
            let mut manifest = gateway.manifest()?;
            manifest["input_objects"] = json!({"locked_run":locked,"policy":policy,"provider":provider,"gateway":spec_object});
            save_json(&output.join("publication.json"), &manifest)?;
            println!("{manifest}");
            std::io::stdout().flush()?;
            while !STOP.load(Ordering::Acquire) && gateway.poll()? {
                std::thread::sleep(Duration::from_millis(100));
            }
            gateway.stop()?;
        }
        "credentials" => {
            ensure!(args.len() == 5, "credentials NEW_DIRECTORY IP RUN_SHA256");
            println!(
                "{}",
                credentials::create(
                    Path::new("/usr/bin/openssl"),
                    Path::new(&args[2]),
                    args[3].parse()?,
                    &args[4]
                )?
            );
        }
        "serve" => {
            ensure!(args.len() == 4, "serve SPEC_JSON NEW_DIRECTORY");
            let spec: GatewaySpec = read_json(Path::new(&args[2]), 65536)?;
            signals();
            let mut gateway = Gateway::start(&spec, Path::new(&args[3]))?;
            println!("{}", gateway.manifest()?);
            std::io::stdout().flush()?;
            while !STOP.load(Ordering::Acquire) {
                ensure!(
                    gateway.is_running()?,
                    "gateway process exited; inspect its private logs"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
            gateway.stop()?;
        }
        "serve-selected" | "serve-selected-connected" => {
            ensure!(
                args.len() == 6,
                "serve-selected SPEC_JSON LOCKED_RUN_JSON WORKER_SOCKET NEW_DIRECTORY"
            );
            let spec: GatewaySpec = read_json(Path::new(&args[2]), 65536)?;
            let run = read_json(Path::new(&args[3]), 16 << 20)?;
            signals();
            if args[1] == "serve-selected-connected" {
                connected_stop();
            }
            let mut gateway =
                SelectedGateway::start(Path::new(&args[4]), run, &spec, Path::new(&args[5]))?;
            println!("{}", gateway.manifest()?);
            std::io::stdout().flush()?;
            while !STOP.load(Ordering::Acquire) && gateway.poll()? {
                std::thread::sleep(Duration::from_millis(100));
            }
            gateway.stop()?;
        }
        _ => anyhow::bail!("unknown access operation"),
    }
    Ok(())
}
