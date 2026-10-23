//! User-facing runner for the frozen, tested adapted S9 development milestone.
//! It invokes trusted simulator helpers. Acquired firmware runs only through
//! their isolated worker boundary. This is not an exact-target run command.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    driver: PathBuf,
    bundle: PathBuf,
    backend: PathBuf,
    pool_backend: PathBuf,
    browser_payload: PathBuf,
    monitor: PathBuf,
    inputs: BTreeMap<PathBuf, ObjectId>,
    limits: Vec<String>,
}
const MANIFEST: &str = include_str!("../../../simulator/demos/s9-adapted.json");
const DRIVER_SHA: &str = "d062d042080b8273bad91aee006101d66b2d31156f016d947ed1c9d05baa095b";

fn check(m: &Manifest) -> Result<()> {
    ensure!(
        m.schema == "md-sim-adapted-s9-demo/1",
        "unknown demo contract"
    );
    ensure!(
        m.inputs
            .get(&m.driver)
            .is_some_and(|v| v.sha256 == DRIVER_SHA),
        "unrecognized simulator runner"
    );
    for (path, expected) in &m.inputs {
        ensure!(
            !path.as_os_str().is_empty()
                && path.components().all(|c| matches!(c, Component::Normal(_))),
            "demo input path is not repository-relative"
        );
        ensure!(
            hash_file(path)
                .with_context(|| format!("missing demo prerequisite {}", path.display()))?
                == *expected,
            "demo prerequisite changed: {}",
            path.display()
        );
    }
    Ok(())
}

pub fn s9(check_only: bool, out: Option<PathBuf>) -> Result<(Value, i32)> {
    let manifest: Manifest = serde_json::from_str(MANIFEST)?;
    ensure!(
        check_only || out.is_some(),
        "use --check or provide --out NEW_DIRECTORY"
    );
    check(&manifest)?;
    if check_only {
        return Ok((
            json!({"passed":true,"checked_inputs":manifest.inputs.len(),"classification":"adapted-s9-development-milestone","full_specification_complete":false,"limits":manifest.limits}),
            0,
        ));
    }
    let out = out.context("demo output absent")?;
    ensure!(!out.exists(), "demo output must be new");
    fs::create_dir_all(
        out.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    fs::create_dir(&out)?;
    fs::write(out.join("input-manifest.json"), MANIFEST)?;
    let run = out.join("run");
    let argv = vec![
        manifest.bundle.display().to_string(),
        manifest.backend.display().to_string(),
        manifest.pool_backend.display().to_string(),
        run.display().to_string(),
        format!("signed-update={}", manifest.monitor.display()),
        format!("browser-payload={}", manifest.browser_payload.display()),
    ];
    save_json(
        &out.join("invocation.json"),
        &json!({"driver":manifest.driver,"argv":argv,"working_directory":std::env::current_dir()?,"input_manifest":hash_file(&out.join("input-manifest.json"))?,"cli":hash_file(&std::env::current_exe()?)?,"full_specification_complete":false}),
    )?;
    eprintln!(
        "Running the adapted S9 milestone. Evidence: {}",
        out.display()
    );
    eprintln!(
        "Four boots and eight Core checks. Progress: {}/run/progress.json",
        out.display()
    );
    let status = Command::new(&manifest.driver)
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(fs::File::create(out.join("driver.stdout.log"))?)
        .stderr(fs::File::create(out.join("driver.stderr.log"))?)
        .status()?;
    let result = (|| -> Result<Value> {
        ensure!(
            status.success(),
            "adapted S9 run failed; see retained driver log and run assessment"
        );
        check(&manifest)?;
        let path = run.join("assessment.json");
        ensure!(
            fs::metadata(&path)?.len() <= 4 << 20,
            "cycle assessment bound"
        );
        let assessment: Value = serde_json::from_slice(&fs::read(&path)?)?;
        ensure!(
            assessment["passed"] == true
                && assessment["result"]["phases"]
                    .as_array()
                    .is_some_and(|v| v.len() == 3),
            "cycle did not pass all required phases"
        );
        Ok(json!({"assessment":hash_file(&path)?,"path":path}))
    })();
    let summary = json!({"passed":result.is_ok(),"classification":"original-firmware-adapted-reference-environment","milestone":"usable-adapted-s9-development","full_specification_complete":false,"assessment":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),"limits":manifest.limits,"input_manifest":hash_file(&out.join("input-manifest.json"))?,"invocation":hash_file(&out.join("invocation.json"))?});
    save_json(&out.join("assessment.json"), &summary)?;
    Ok((summary, if result.is_ok() { 0 } else { 1 }))
}
