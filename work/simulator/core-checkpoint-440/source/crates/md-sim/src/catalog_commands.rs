use anyhow::{bail, ensure, Context, Result};
use clap::Subcommand;
use md_sim_catalog::{
    import_catalog, load_catalog, BackendRegistry, Catalog, MatrixPlan, Pairing, PlanState,
    TargetDefinitionV2,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// Freeze catalogue data, evidence, and available profile registries.
    Import {
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
    /// Read and verify the bytes of one archived input against the frozen record.
    VerifyArtifact {
        artifact_id: String,
        #[arg(long, default_value = ".")]
        repository: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum MatrixCommand {
    /// Account for every record and relevant candidate in the frozen catalogue.
    Plan {
        #[arg(long, default_value = "all-catalogued")]
        scope: String,
        #[arg(long, default_value = "relevant", value_parser = ["relevant", "cartesian"])]
        pairing: String,
        #[arg(long)]
        include_blocked: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Assess unresolved plan rows. Unimplemented execution remains not-run.
    Run {
        plan: PathBuf,
        #[arg(long, default_value = "local")]
        workers: String,
        #[arg(long, default_value = "conformance")]
        suite: String,
        #[arg(long, default_value_t = 1)]
        jobs: usize,
        #[arg(long)]
        resume: bool,
        #[arg(long)]
        require_complete: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Render a retained matrix result as an offline HTML report.
    Report {
        directory: PathBuf,
        #[arg(long, default_value = "html", value_parser = ["html", "json"])]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum TargetsCommand {
    List {
        #[arg(long)]
        device: Option<String>,
        #[arg(long)]
        firmware: Option<String>,
        #[arg(long)]
        runnable: bool,
    },
    Explain {
        target_id: String,
    },
}

fn save(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    serde_json::to_writer_pretty(file, value)?;
    Ok(())
}

fn current_catalog(state: &Path) -> Result<Catalog> {
    let current: Value = serde_json::from_reader(
        fs::File::open(state.join("current-snapshot.json"))
            .context("import a catalogue snapshot first")?,
    )?;
    let hash = current["snapshot_hash"]
        .as_str()
        .filter(|h| md_sim_catalog::valid_sha256(h))
        .context("invalid current snapshot reference")?;
    let catalog = load_catalog(&state.join("snapshots").join(hash))?;
    ensure!(
        catalog.snapshot.manifest.snapshot_hash == hash,
        "stored snapshot differs from its selected identity"
    );
    Ok(catalog)
}

pub fn catalog(command: CatalogCommand, state: &Path) -> Result<(Value, i32)> {
    match command {
        CatalogCommand::Import { path } => {
            let snapshot = import_catalog(&path, &state.join("snapshots"))?;
            save(
                &state.join("current-snapshot.json"),
                &json!({"snapshot_hash":snapshot.manifest.snapshot_hash}),
            )?;
            let frozen = load_catalog(&snapshot.directory)?;
            Ok((
                json!({"snapshot":snapshot,"entities":frozen.entities.len(),"artifacts":frozen.artifacts.len(),"payload_verification":"Selected artifact bytes must be verified before preparation."}),
                0,
            ))
        }
        CatalogCommand::VerifyArtifact {
            artifact_id,
            repository,
        } => {
            let verified = current_catalog(state)?.verify_artifact(&repository, &artifact_id)?;
            Ok((serde_json::to_value(verified)?, 0))
        }
    }
}

pub fn matrix(command: MatrixCommand, state: &Path) -> Result<(Value, i32)> {
    match command {
        MatrixCommand::Plan {
            scope,
            pairing,
            include_blocked: _,
            out,
        } => {
            ensure!(
                scope == "all-catalogued",
                "only the complete all-catalogued scope is implemented"
            );
            let pairing = if pairing == "cartesian" {
                Pairing::Cartesian
            } else {
                Pairing::Relevant
            };
            let plan = current_catalog(state)?.plan(pairing)?;
            plan.validate()?;
            save(&out, &plan)?;
            Ok((
                json!({"plan":out,"plan_hash":plan.plan_hash,"summary":plan.summary(),"notice":"Classification does not establish firmware execution or passing tests."}),
                0,
            ))
        }
        MatrixCommand::Run {
            plan,
            workers,
            suite,
            jobs,
            resume,
            require_complete,
            out,
        } => {
            ensure!(jobs > 0, "jobs must be positive");
            ensure!(
                [
                    "smoke",
                    "conformance",
                    "update-recovery",
                    "calibration",
                    "soak"
                ]
                .contains(&suite.as_str()),
                "unknown suite"
            );
            let plan: MatrixPlan = serde_json::from_reader(fs::File::open(plan)?)?;
            plan.validate()?;
            ensure!(
                md_sim_catalog::valid_sha256(&plan.snapshot_hash),
                "invalid plan snapshot identity"
            );
            let frozen = load_catalog(&state.join("snapshots").join(&plan.snapshot_hash))?;
            ensure!(
                frozen.snapshot.manifest.snapshot_hash == plan.snapshot_hash,
                "stored snapshot differs from the plan identity"
            );
            let expected = frozen.plan(plan.pairing)?;
            ensure!(expected.plan_hash == plan.plan_hash, "plan does not account for the frozen catalogue; regenerate it from the selected snapshot");
            ensure!(!out.exists(), "result directory already exists; choose a new output directory to preserve prior results; execution resume is not implemented");
            // No row may be promoted from an unresolved candidate to execution.
            // This command persists the exact input and every unsatisfied row.
            let summary = plan.summary();
            let dispositions:Vec<_>=plan.rows.iter().map(|row| json!({
                "row_id":row.id,"plan_state":row.state,
                "test_status":match row.state { PlanState::NotApplicable | PlanState::Incompatible => "not-applicable", PlanState::Blocked => "blocked", _ => "not-run" },
                "evidence":"See the complete retained plan row and frozen source constraints."
            })).collect();
            let complete = summary.complete;
            let result = json!({
                "schema_version":"2.0","plan_hash":plan.plan_hash,"snapshot_hash":plan.snapshot_hash,
                "requested_workers":workers,"suite":suite,"jobs":jobs,"resume_requested":resume,"reused_results":0,
                "summary":summary,"coverage_complete":complete,"executed_targets":0,"passing_targets":0,
                "notice":"Coverage is incomplete. No exact runnable firmware targets were executed by this command.",
                "rows":dispositions,"implicit_cartesian_pairs":plan.cartesian,
            });
            fs::create_dir_all(&out)?;
            save(&out.join("plan.json"), &plan)?;
            save(&out.join("result.json"), &result)?;
            Ok((
                json!({"result":out.join("result.json"),"plan_hash":plan.plan_hash,"summary":summary,"coverage_complete":complete,"executed_targets":0,"passing_targets":0}),
                if require_complete && !complete { 3 } else { 0 },
            ))
        }
        MatrixCommand::Report { directory, format } => {
            let result: Value =
                serde_json::from_reader(fs::File::open(directory.join("result.json"))?)?;
            if format == "json" {
                return Ok((result, 0));
            }
            let text = serde_json::to_string_pretty(&result)?;
            let escaped = text
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let output = directory.join("index.html");
            fs::write(&output,format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>md-sim coverage report</title><body><h1>Simulator coverage report</h1><p>Inventory classification does not prove firmware execution. Read all incomplete gates.</p><pre>{escaped}</pre></body></html>"))?;
            Ok((
                json!({"report":output,"coverage_complete":result["coverage_complete"]}),
                0,
            ))
        }
    }
}

pub fn targets(command: TargetsCommand, state: &Path) -> Result<(Value, i32)> {
    let catalog = current_catalog(state)?;
    let mut definitions = Vec::new();
    for file in &catalog.snapshot.manifest.files {
        if file.path.starts_with("profiles/targets/") && file.path.ends_with(".json") {
            let target: TargetDefinitionV2 = serde_json::from_reader(fs::File::open(
                catalog.snapshot.directory.join(&file.path),
            )?)
            .with_context(|| format!("parse exact target {}", file.path))?;
            definitions.push(target);
        }
    }
    let registry = BackendRegistry::default();
    let verified = BTreeMap::new();
    match command {
        TargetsCommand::List {
            device,
            firmware,
            runnable,
        } => {
            let mut targets = Vec::new();
            for definition in definitions {
                if device
                    .as_ref()
                    .is_some_and(|id| id != &definition.device_id)
                    || firmware
                        .as_ref()
                        .is_some_and(|id| id != &definition.firmware_entity_id)
                {
                    continue;
                }
                let resolved = catalog.resolve_target(&definition, &registry, &verified)?;
                if runnable && resolved.state != PlanState::Runnable {
                    continue;
                }
                targets.push(resolved);
            }
            Ok((
                json!({"targets":targets,"catalogue_devices":catalog.entities.values().filter(|v|v["kind"]=="device").count(),"notice":"Catalogue candidates are not exact executable targets. Missing profiles and models remain unresolved."}),
                0,
            ))
        }
        TargetsCommand::Explain { target_id } => {
            let Some(definition) = definitions.into_iter().find(|t| t.id == target_id) else {
                bail!("no exact target {target_id}; catalogue family names do not select executable profiles")
            };
            let resolution = catalog.resolve_target(&definition, &registry, &verified)?;
            Ok((json!({"definition":definition,"resolution":resolution}), 0))
        }
    }
}
