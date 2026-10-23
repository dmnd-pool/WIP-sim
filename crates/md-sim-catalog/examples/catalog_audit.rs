use anyhow::{Context, Result};
use md_sim_catalog::{import_catalog, load_catalog, Pairing};
use std::{fs, path::PathBuf};

fn main() -> Result<()> {
    let repository = PathBuf::from(std::env::args_os().nth(1).unwrap_or_else(|| ".".into()));
    let output = PathBuf::from(
        std::env::args_os()
            .nth(2)
            .unwrap_or_else(|| "work/simulator/catalog-audit".into()),
    );
    fs::create_dir_all(&output)?;
    let snapshot = import_catalog(&repository, &output.join("snapshots"))?;
    eprintln!("Verified snapshot {}", snapshot.manifest.snapshot_hash);
    let catalog = load_catalog(&snapshot.directory)?;
    eprintln!(
        "Loaded {} entities and {} artifacts",
        catalog.entities.len(),
        catalog.artifacts.len()
    );
    for (name, pairing) in [
        ("relevant", Pairing::Relevant),
        ("cartesian", Pairing::Cartesian),
    ] {
        let plan = catalog.plan(pairing)?;
        eprintln!("Built {name} plan with {} explicit rows", plan.rows.len());
        plan.validate()?;
        let file = fs::File::create(output.join(format!("{name}-plan.json")))
            .context("create plan output")?;
        serde_json::to_writer_pretty(file, &plan)?;
        let summary = plan.summary();
        fs::write(
            output.join(format!("{name}-summary.json")),
            serde_json::to_vec_pretty(&summary)?,
        )?;
        println!("{name}: {}", serde_json::to_string(&summary)?);
        println!(
            "{name} require-complete: {}",
            plan.require_complete().unwrap_err()
        );
    }
    println!("snapshot: {}", snapshot.directory.display());
    Ok(())
}
