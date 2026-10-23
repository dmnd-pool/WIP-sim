//! Replay saved traces with the shared independent oracle.
use std::{fs::File, io::Read, path::Path};
fn read(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut b = Vec::new();
    File::open(path)?
        .take(20 * 1024 * 1024 + 1)
        .read_to_end(&mut b)?;
    if b.len() > 20 * 1024 * 1024 {
        return Err("replay byte limit".into());
    }
    Ok(b)
}
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let a: Vec<_> = std::env::args_os().collect();
    if a.len() != 2 {
        return Err("usage: replay_s2_trace RUN_DIRECTORY".into());
    }
    let p = Path::new(&a[1]);
    let report = serde_json::from_slice(&read(&p.join("pool-report.json"))?)?;
    let spec: serde_json::Value = serde_json::from_slice(&read(&p.join("run-spec.json"))?)?;
    let limit = spec["limits"]["wall_seconds"]
        .as_u64()
        .ok_or("missing declared wall budget")?
        .checked_mul(1_000_000_000)
        .ok_or("wall budget overflow")?;
    let result = md_sim_mining::replay_s2::replay(
        &report,
        &read(&p.join("device-transactions.jsonl"))?,
        limit,
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
