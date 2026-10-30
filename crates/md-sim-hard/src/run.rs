//! Run directory, circuit simulation and the unattended comparison loop.
//!
//! Layout of a run directory:
//! - `prepare.json`: input paths and SHA-256 hashes, pins, tool paths
//! - `tools/`, `tb.v`, `sim.vvp`: unpacked Icarus, testbench, compiled design
//! - `calibration.json` and `calibration/`: pin map, alignment, evidence
//! - `summary.json`: totals and first findings, rewritten after each sequence
//! - `run.log`: one line per sequence
//! - `cases/<seed>/`: kept only for sequences with findings or errors
use crate::{
    bench::{self, Pads, Sources, Tools},
    calibrate::Calibration,
    circuit::{self, CircuitTrace},
    compare::{self, Counts, Report},
    generate, model,
    sequence::Sequence,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const PREPARED_SCHEMA: &str = "md-sim-hard-s9-fabric-prepared/1";
pub const SUMMARY_SCHEMA: &str = "md-sim-hard-s9-fabric-summary/1";

/// Default input locations, relative to the repository root.
pub mod defaults {
    pub const NETLIST: &str = "work/simulator/qemu-s9/netlist-selected-502k/selected.v";
    pub const UNISIMS: &str = "artifacts/files/source/source-unisim-1c8e05f/verilog/src/unisims";
    pub const GLBL: &str = "artifacts/files/source/source-unisim-1c8e05f/verilog/src/glbl.v";
    pub const ICARUS: &str = "work/simulator/qemu-s9/iverilog-build-502f/iverilog-tools.tar.gz";
    pub const PS7_PORTS: &str =
        "artifacts/files/source/source-fasm-graph-8372b58/zynq7/ps7_ports.json";
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inputs {
    pub netlist: PathBuf,
    pub unisims: PathBuf,
    pub glbl: PathBuf,
    pub icarus: PathBuf,
    pub ps7_ports: PathBuf,
}

impl Inputs {
    pub fn under(workspace: &Path) -> Self {
        Self {
            netlist: workspace.join(defaults::NETLIST),
            unisims: workspace.join(defaults::UNISIMS),
            glbl: workspace.join(defaults::GLBL),
            icarus: workspace.join(defaults::ICARUS),
            ps7_ports: workspace.join(defaults::PS7_PORTS),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prepared {
    pub schema: String,
    pub inputs: Inputs,
    /// SHA-256 of every input file, including each vendor model file.
    pub hashes: BTreeMap<String, String>,
    pub pads: Pads,
    pub tools: Tools,
    pub testbench: PathBuf,
    pub compiled: PathBuf,
    pub compile_seconds: f64,
}

fn sha256(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

pub fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Hash inputs, unpack Icarus, write the testbench and compile it.
pub fn prepare(inputs: &Inputs, run_dir: &Path) -> Result<Prepared> {
    fs::create_dir_all(run_dir)?;
    let run_dir = run_dir.canonicalize()?;
    let mut hashes = BTreeMap::new();
    for path in [
        &inputs.netlist,
        &inputs.glbl,
        &inputs.icarus,
        &inputs.ps7_ports,
    ] {
        hashes.insert(path.display().to_string(), sha256(path)?);
    }
    let mut models: Vec<PathBuf> = fs::read_dir(&inputs.unisims)
        .with_context(|| format!("read {}", inputs.unisims.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "v"))
        .collect();
    models.sort();
    ensure!(
        !models.is_empty(),
        "no vendor models in {}",
        inputs.unisims.display()
    );
    for path in &models {
        hashes.insert(path.display().to_string(), sha256(path)?);
    }
    let netlist = fs::read_to_string(&inputs.netlist)?;
    let pads = bench::top_pads(&netlist)?;
    let ps7 = fs::read_to_string(&inputs.ps7_ports)?;
    let tools = bench::extract_tools(&inputs.icarus, &run_dir.join("tools"))?;
    let testbench = run_dir.join("tb.v");
    fs::write(&testbench, bench::testbench(&pads, &ps7)?)?;
    let compiled = run_dir.join("sim.vvp");
    let started = Instant::now();
    let log = tools.compile(
        &testbench,
        &Sources {
            netlist: inputs.netlist.clone(),
            unisims: inputs.unisims.clone(),
            glbl: inputs.glbl.clone(),
        },
        &compiled,
    )?;
    fs::write(run_dir.join("compile.log"), log)?;
    let prepared = Prepared {
        schema: PREPARED_SCHEMA.into(),
        inputs: inputs.clone(),
        hashes,
        pads,
        tools,
        testbench,
        compiled,
        compile_seconds: started.elapsed().as_secs_f64(),
    };
    write_json(&run_dir.join("prepare.json"), &prepared)?;
    Ok(prepared)
}

/// Run one sequence on the circuit. Files go to `dir`.
pub fn simulate(
    prepared: &Prepared,
    dir: &Path,
    seq: &Sequence,
    timeout: Duration,
) -> Result<(CircuitTrace, Duration)> {
    fs::create_dir_all(dir)?;
    fs::write(dir.join("sequence.json"), serde_json::to_vec(seq)?)?;
    let text = dir.join("sequence.txt");
    fs::write(&text, seq.bench_text())?;
    let out = dir.join("circuit.txt");
    let log = fs::File::create(dir.join("vvp.log"))?;
    let mut child = prepared
        .tools
        .simulation(&prepared.compiled, &text, &out)
        .stdout(log.try_clone()?)
        .stderr(log)
        .spawn()
        .context("start vvp")?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            ensure!(status.success(), "vvp exited with {status}");
            break;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("vvp exceeded {} s", timeout.as_secs());
        }
        thread::sleep(Duration::from_millis(250));
    }
    let trace = circuit::parse(&fs::read_to_string(&out).context("read circuit trace")?)?;
    Ok((trace, started.elapsed()))
}

/// Compare one simulated sequence with the model.
pub fn check(seq: &Sequence, trace: &CircuitTrace, calibration: &Calibration) -> Report {
    let model = model::run(
        seq,
        trace,
        &calibration.mapping,
        calibration.alignment,
        calibration.build_id,
    );
    compare::compare(seq, trace, &model, &calibration.mapping)
}

/// Parallel jobs that fit the free memory and leave cores for other work.
pub fn default_jobs() -> usize {
    let cores = thread::available_parallelism().map_or(4, |n| n.get());
    let available_mb = fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|l| l.starts_with("MemAvailable:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|kb| kb.parse::<usize>().ok())
        })
        .map_or(8192, |kb| kb / 1024);
    // Each simulator process needs about 0.5 GB.
    cores.saturating_sub(4).min(available_mb / 700).clamp(1, 24)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunOptions {
    pub hours: f64,
    pub jobs: usize,
    pub seed: u64,
    /// Planned cycles per random sequence.
    pub budget: u64,
    pub max_sequences: Option<u64>,
    pub keep_cases: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Example {
    pub seed: u64,
    pub case: PathBuf,
    pub class: String,
    pub chain: Option<u8>,
    pub step: Option<usize>,
    pub cycle: u64,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Summary {
    pub schema: String,
    pub status: String,
    pub started_unix: u64,
    pub updated_unix: u64,
    pub deadline_unix: u64,
    pub options: RunOptions,
    pub calibration: Calibration,
    pub sequences: u64,
    pub sequences_with_findings: u64,
    pub simulation_errors: u64,
    pub cycles: u64,
    pub circuit_seconds: f64,
    pub counts: Counts,
    /// Findings per class and register, all occurrences.
    pub findings_by_class: BTreeMap<String, u64>,
    /// Sequences that show each class and register at least once. One root
    /// cause often repeats inside a sequence; this count does not.
    pub sequences_by_class: BTreeMap<String, u64>,
    pub sequences_with_unsupported_model_cases: u64,
    pub tx_offset_range: Option<(i64, i64)>,
    pub kept_cases: u64,
    pub examples: Vec<Example>,
    pub errors: Vec<String>,
}

const EXAMPLES: usize = 200;
/// Examples kept per class and register, so rare classes stay visible.
const EXAMPLES_PER_KEY: usize = 5;

fn add_counts(total: &mut Counts, part: &Counts) {
    total.exact_reads += part.exact_reads;
    total.window_reads += part.window_reads;
    total.timing_only += part.timing_only;
    total.writes += part.writes;
    total.tx_frames += part.tx_frames;
    total.unknown_resolved += part.unknown_resolved;
    total.skipped_unsupported += part.skipped_unsupported;
}

impl Summary {
    fn record(
        &mut self,
        seed: u64,
        case: &Path,
        trace: &CircuitTrace,
        seconds: f64,
        report: &Report,
    ) {
        self.sequences += 1;
        self.cycles += trace.last_cycle();
        self.circuit_seconds += seconds;
        add_counts(&mut self.counts, &report.counts);
        if !report.unsupported.is_empty() {
            self.sequences_with_unsupported_model_cases += 1;
        }
        if let (Some(&lo), Some(&hi)) = (
            report.tx_offsets.iter().min(),
            report.tx_offsets.iter().max(),
        ) {
            let (a, b) = self.tx_offset_range.unwrap_or((lo, hi));
            self.tx_offset_range = Some((a.min(lo), b.max(hi)));
        }
        if report.findings.is_empty() {
            return;
        }
        self.sequences_with_findings += 1;
        let mut seen = Vec::new();
        for f in &report.findings {
            let key = f.key();
            *self.findings_by_class.entry(key.clone()).or_default() += 1;
            if seen.contains(&key) {
                continue;
            }
            seen.push(key.clone());
            let sequences = self.sequences_by_class.entry(key.clone()).or_default();
            *sequences += 1;
            // The first examples of each key, one per sequence.
            if *sequences as usize <= EXAMPLES_PER_KEY && self.examples.len() < EXAMPLES {
                self.examples.push(Example {
                    seed,
                    case: case.to_path_buf(),
                    class: f.class.clone(),
                    chain: f.chain,
                    step: f.step,
                    cycle: f.cycle,
                    detail: f.detail.clone(),
                });
            }
        }
    }
}

/// Compare random sequences until the deadline. Calibration must exist.
pub fn run(run_dir: &Path, options: RunOptions) -> Result<Summary> {
    let prepared: Prepared = read_json(&run_dir.join("prepare.json"))?;
    let calibration: Calibration = read_json(&run_dir.join("calibration.json"))?;
    let started = unix_now();
    let deadline = Instant::now() + Duration::from_secs_f64(options.hours * 3600.0);
    let summary = Mutex::new(Summary {
        schema: SUMMARY_SCHEMA.into(),
        status: "running".into(),
        started_unix: started,
        updated_unix: started,
        deadline_unix: started + (options.hours * 3600.0) as u64,
        options: options.clone(),
        calibration: calibration.clone(),
        sequences: 0,
        sequences_with_findings: 0,
        simulation_errors: 0,
        cycles: 0,
        circuit_seconds: 0.0,
        counts: Counts::default(),
        findings_by_class: BTreeMap::new(),
        sequences_by_class: BTreeMap::new(),
        sequences_with_unsupported_model_cases: 0,
        tx_offset_range: None,
        kept_cases: 0,
        examples: Vec::new(),
        errors: Vec::new(),
    });
    let next = AtomicU64::new(0);
    let cases = run_dir.join("cases");
    fs::create_dir_all(&cases)?;
    let log = Mutex::new(
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(run_dir.join("run.log"))?,
    );
    let summary_path = run_dir.join("summary.json");
    write_json(&summary_path, &*summary.lock().unwrap())?;
    thread::scope(|scope| {
        for _ in 0..options.jobs.max(1) {
            scope.spawn(|| loop {
                if Instant::now() >= deadline {
                    break;
                }
                let index = next.fetch_add(1, Ordering::SeqCst);
                if options.max_sequences.is_some_and(|m| index >= m) {
                    break;
                }
                let seed = options.seed.wrapping_add(index);
                let case = cases.join(format!("{seed:016x}"));
                let seq = generate::random(
                    seed,
                    &calibration.mapping,
                    calibration.build_id,
                    options.budget,
                    calibration.alignment.ratio,
                );
                let outcome = simulate(
                    &prepared,
                    &case,
                    &seq,
                    Duration::from_secs(options.timeout_seconds),
                )
                .map(|(trace, elapsed)| {
                    let report = check(&seq, &trace, &calibration);
                    (trace, elapsed, report)
                });
                let mut s = summary.lock().unwrap();
                let keep = match &outcome {
                    Ok((trace, elapsed, report)) => {
                        s.record(seed, &case, trace, elapsed.as_secs_f64(), report);
                        let _ = writeln!(
                            log.lock().unwrap(),
                            "{seed:016x} cycles={} seconds={:.1} findings={} unsupported={}",
                            trace.last_cycle(),
                            elapsed.as_secs_f64(),
                            report.findings.len(),
                            report.unsupported.len()
                        );
                        if !report.findings.is_empty() && s.kept_cases < options.keep_cases {
                            let _ = write_json(&case.join("report.json"), report);
                            true
                        } else {
                            false
                        }
                    }
                    Err(error) => {
                        s.simulation_errors += 1;
                        if s.errors.len() < 20 {
                            s.errors.push(format!("{seed:016x}: {error:#}"));
                        }
                        let _ = writeln!(log.lock().unwrap(), "{seed:016x} error={error:#}");
                        s.kept_cases < options.keep_cases
                    }
                };
                if keep {
                    s.kept_cases += 1;
                } else {
                    let _ = fs::remove_dir_all(&case);
                }
                s.updated_unix = unix_now();
                let _ = write_json(&summary_path, &*s);
            });
        }
    });
    let mut s = summary.into_inner().unwrap();
    s.status = "finished".into();
    s.updated_unix = unix_now();
    write_json(&summary_path, &s)?;
    Ok(s)
}

/// Compare a kept case again, optionally after a fresh circuit run.
pub fn replay(run_dir: &Path, case: &Path, simulate_again: bool) -> Result<Report> {
    let calibration: Calibration = read_json(&run_dir.join("calibration.json"))?;
    let seq: Sequence = read_json(&case.join("sequence.json"))?;
    let trace = if simulate_again {
        let prepared: Prepared = read_json(&run_dir.join("prepare.json"))?;
        simulate(
            &prepared,
            &case.join("replay"),
            &seq,
            Duration::from_secs(7200),
        )?
        .0
    } else {
        circuit::parse(&fs::read_to_string(case.join("circuit.txt"))?)?
    };
    Ok(check(&seq, &trace, &calibration))
}
