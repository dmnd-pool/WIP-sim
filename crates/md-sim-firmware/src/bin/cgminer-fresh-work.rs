use anyhow::{ensure, Result};
use md_sim_mining::stratum::{serve_inherited, StratumConfig};
use std::{
    fs,
    io::Read,
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        (8..=9).contains(&args.len()),
        "usage: cgminer-fresh-work FIRMWARE OUT CAP_HPS SEED_HEX EPOCH DIFFICULTY_EXPONENT random-parent|regtest-genesis [--export]"
    );
    let cap_hps = args
        .get(3)
        .map(|x| x.parse::<f64>())
        .transpose()?
        .unwrap_or(10_000.0);
    ensure!(
        cap_hps.is_finite() && cap_hps > 0.0 && cap_hps <= 10_000.0,
        "this experiment permits caps above zero through 10000 H/s"
    );
    let out = Path::new(&args[2]);
    ensure!(!out.exists(), "output directory must be fresh");
    fs::create_dir_all(out)?;
    let (pool_stream, firmware_stream) = UnixStream::pair()?;
    let pool_stream = fs::File::from(OwnedFd::from(pool_stream));
    let firmware_stream = fs::File::from(OwnedFd::from(firmware_stream));
    let mut seed = [0u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut seed)?;
    if let Some(s) = args.get(4) {
        let bytes = hex::decode(s)?;
        ensure!(bytes.len() == 32, "seed must contain 32 bytes");
        seed.copy_from_slice(&bytes);
    }
    ensure!(
        args.len() < 9 || args[8] == "--export",
        "unknown final option"
    );
    let time = args[5].parse::<u32>()?;
    let exponent = args[6].parse::<i32>()?;
    ensure!(
        (-31..=11).contains(&exponent),
        "difficulty exponent must be -31 through 11"
    );
    let schedule = md_sim_firmware::schedule::mining_schedule(cap_hps, exponent)?;
    let regtest = match args[7].as_str() {
        "random-parent" => false,
        "regtest-genesis" => true,
        _ => anyhow::bail!("unknown pool anchor mode"),
    };
    let duration = Duration::from_secs(schedule.timeout_seconds);
    let stop = Arc::new(AtomicBool::new(false));
    let pool_stop = stop.clone();
    let pool_done = stop.clone();
    let config = StratumConfig {
        response_policy: md_sim_mining::stratum::ResponsePolicy::JsonRpc1StratumV1,
        regtest_anchor: regtest.then(md_sim_mining::regtest::RegtestAnchor::genesis),
        difficulty_scale: md_sim_mining::stratum::DifficultyScale::ThreeHalves,
        limits: Default::default(),
        seed,
        time,
        difficulty_exponents: vec![exponent, exponent + 1],
        timeout: duration,
        trace_path: out.join("pool-transactions.jsonl"),
    };
    let pool = std::thread::spawn(move || {
        let result = serve_inherited(pool_stream, config, pool_stop);
        pool_done.store(true, Ordering::Relaxed);
        result
    });
    let session = md_sim_firmware::session::run(
        Path::new(&args[1]),
        &firmware_stream,
        out,
        duration,
        cap_hps,
        stop.clone(),
    );
    stop.store(true, Ordering::Relaxed);
    let report = pool
        .join()
        .map_err(|_| anyhow::anyhow!("pool thread panicked"))?
        .map_err(anyhow::Error::msg)?;
    fs::write(
        out.join("pool-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    let block_export = regtest.then(|| md_sim_mining::regtest::regtest_blocks(&report));
    if let Some(Ok(blocks)) = &block_export {
        fs::write(
            out.join("regtest-blocks.json"),
            serde_json::to_vec_pretty(blocks)?,
        )?;
    }
    let block_error = block_export.as_ref().and_then(|r| r.as_ref().err());
    let identity = serde_json::json!({"firmware":md_sim_runtime::store::hash_file(Path::new(&args[1]))?,
        "harness":md_sim_runtime::store::hash_file(&std::env::current_exe()?)?,"seed":hex::encode(seed),
        "clock":"wall-monotonic","pool_epoch":time,"hash_cap_hps":cap_hps,"mining_schedule":schedule,
        "difficulty_exponents":[exponent,exponent+1],"difficulty_scale":"three-halves",
        "pool_anchor":args[7],
        "patch_sha256":md_sim_firmware::prepare::hash_bytes(md_sim_firmware::prepare::PATCH)});
    fs::write(
        out.join("run-inputs.json"),
        serde_json::to_vec_pretty(&identity)?,
    )?;
    let mut evidence = std::collections::BTreeMap::new();
    if args.len() == 9 {
        for name in [
            "session.json",
            "guest.log",
            "device-transactions.jsonl",
            "pool-transactions.jsonl",
            "pool-report.json",
            "run-inputs.json",
            "regtest-blocks.json",
        ] {
            if out.join(name).is_file() {
                evidence.insert(name, hex::encode(fs::read(out.join(name))?));
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &serde_json::json!({"source_runtime":session.as_ref().ok(),"fresh_work_passed":report.completed && session.is_ok() && block_error.is_none(),
        "error":session.as_ref().err().map(ToString::to_string),"block_export_error":block_error,"evidence_hex":evidence,
        "pool_report":"pool-report.json","release_complete":false,"original_board_boot":"not-run","regtest":if regtest {"firmware-candidates-await-Core-import"} else {"not-run"}})
        )?
    );
    session?;
    if let Some(result) = block_export {
        result.map_err(anyhow::Error::msg)?;
    }
    ensure!(
        report.completed,
        "fresh-work firmware test incomplete; inspect pool-report.json and guest.log"
    );
    Ok(())
}
