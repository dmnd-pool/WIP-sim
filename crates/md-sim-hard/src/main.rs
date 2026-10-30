use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use md_sim_hard::{
    calibrate,
    run::{self, Inputs, RunOptions},
};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "md-sim-hard",
    about = "Hard-mode tools for faithful hardware simulation"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Rebuilt S9 FPGA circuit versus the Rust S9IO model.
    #[command(subcommand)]
    S9Fabric(Fabric),
}

#[derive(Subcommand)]
enum Fabric {
    /// Hash the inputs, unpack Icarus and compile the testbench.
    Prepare {
        #[command(flatten)]
        inputs: InputArgs,
        /// New run directory.
        #[arg(long)]
        out: PathBuf,
    },
    /// Find the build ID, serial pins and bus alignment from the circuit.
    Calibrate {
        #[arg(long)]
        run: PathBuf,
        #[arg(long)]
        jobs: Option<usize>,
    },
    /// Compare random sequences until the deadline.
    Run {
        #[arg(long)]
        run: PathBuf,
        #[command(flatten)]
        options: LoopArgs,
    },
    /// Prepare, calibrate and run in one command.
    Overnight {
        #[command(flatten)]
        inputs: InputArgs,
        #[arg(long)]
        out: PathBuf,
        #[command(flatten)]
        options: LoopArgs,
    },
    /// Compare one kept case again.
    Replay {
        #[arg(long)]
        run: PathBuf,
        #[arg(long)]
        case: PathBuf,
        /// Run the circuit simulation again instead of reusing its trace.
        #[arg(long)]
        simulate: bool,
    },
}

#[derive(Args)]
struct InputArgs {
    /// Repository root. Default input paths are relative to it.
    #[arg(long, default_value = ".")]
    workspace: PathBuf,
    #[arg(long)]
    netlist: Option<PathBuf>,
    #[arg(long)]
    unisims: Option<PathBuf>,
    #[arg(long)]
    glbl: Option<PathBuf>,
    #[arg(long)]
    icarus: Option<PathBuf>,
    #[arg(long)]
    ps7_ports: Option<PathBuf>,
}

impl InputArgs {
    fn resolve(self) -> Inputs {
        let d = Inputs::under(&self.workspace);
        Inputs {
            netlist: self.netlist.unwrap_or(d.netlist),
            unisims: self.unisims.unwrap_or(d.unisims),
            glbl: self.glbl.unwrap_or(d.glbl),
            icarus: self.icarus.unwrap_or(d.icarus),
            ps7_ports: self.ps7_ports.unwrap_or(d.ps7_ports),
        }
    }
}

#[derive(Args)]
struct LoopArgs {
    /// Stop starting new sequences after this many hours.
    #[arg(long, default_value_t = 8.0)]
    hours: f64,
    /// Parallel simulations. Default fits free memory and leaves 4 cores.
    #[arg(long)]
    jobs: Option<usize>,
    /// First seed. Default: the current Unix time.
    #[arg(long)]
    seed: Option<u64>,
    /// Planned cycles per random sequence.
    #[arg(long, default_value_t = 40_000)]
    budget: u64,
    /// Stop after this many sequences.
    #[arg(long)]
    sequences: Option<u64>,
    /// Keep at most this many failing cases on disk.
    #[arg(long, default_value_t = 300)]
    keep_cases: u64,
    #[arg(long, default_value_t = 3600)]
    timeout_seconds: u64,
}

impl LoopArgs {
    fn options(self) -> RunOptions {
        RunOptions {
            hours: self.hours,
            jobs: self.jobs.unwrap_or_else(run::default_jobs),
            seed: self.seed.unwrap_or_else(run::unix_now),
            budget: self.budget,
            max_sequences: self.sequences,
            keep_cases: self.keep_cases,
            timeout_seconds: self.timeout_seconds,
        }
    }
}

fn calibrate_and_report(run_dir: &Path, jobs: usize) -> Result<()> {
    let c = calibrate::calibrate(run_dir, jobs)?;
    println!(
        "calibrated: build_id={:#010x} version={:#010x} tx={:?} rx={:?} alignment={:?} findings={}",
        c.build_id,
        c.version,
        c.mapping.tx,
        c.mapping.rx,
        c.alignment,
        c.findings.len()
    );
    for note in &c.notes {
        println!("note: {note}");
    }
    Ok(())
}

fn run_and_report(run_dir: &Path, options: RunOptions) -> Result<()> {
    println!(
        "running {} jobs for {} h, first seed {}; summary: {}",
        options.jobs,
        options.hours,
        options.seed,
        run_dir.join("summary.json").display()
    );
    let s = run::run(run_dir, options)?;
    println!(
        "finished: {} sequences, {} with findings, {} errors, {} cycles; classes {:?}",
        s.sequences, s.sequences_with_findings, s.simulation_errors, s.cycles, s.findings_by_class
    );
    Ok(())
}

fn main() -> Result<()> {
    let Command::S9Fabric(command) = Cli::parse().command;
    match command {
        Fabric::Prepare { inputs, out } => {
            let p = run::prepare(&inputs.resolve(), &out)?;
            println!(
                "prepared {} in {:.1} s: {} inputs, {} outputs",
                p.compiled.display(),
                p.compile_seconds,
                p.pads.inputs.len(),
                p.pads.outputs.len()
            );
        }
        Fabric::Calibrate { run: dir, jobs } => {
            calibrate_and_report(&dir, jobs.unwrap_or_else(run::default_jobs))?
        }
        Fabric::Run { run: dir, options } => run_and_report(&dir, options.options())?,
        Fabric::Overnight {
            inputs,
            out,
            options,
        } => {
            let options = options.options();
            run::prepare(&inputs.resolve(), &out)?;
            calibrate_and_report(&out, options.jobs)?;
            run_and_report(&out, options)?;
        }
        Fabric::Replay {
            run: dir,
            case,
            simulate,
        } => {
            let report = run::replay(&dir, &case, simulate)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(())
}
