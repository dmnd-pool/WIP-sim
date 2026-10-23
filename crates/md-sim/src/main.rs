mod catalog_commands;
mod demo;
mod source_manifest;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use md_sim_runtime::{
    doctor,
    isolation::{self, ProcessSpec},
    store::{hash_file, ObjectId, Store},
    worker::{self, Request},
};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

#[derive(Parser)]
#[command(
    version,
    about = "Bitcoin ASIC simulator development tools. See capability and coverage results before selecting firmware."
)]
struct Cli {
    #[arg(long, global = true, default_value = "work/simulator/state")]
    state_dir: PathBuf,
    /// Output is always JSON. This option also supports the specified interface.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run a tested development milestone. This does not select an exact miner.
    Demo {
        #[command(subcommand)]
        command: Demo,
    },
    Doctor {
        /// Identify this Core binary without executing it on the host.
        #[arg(long)]
        bitcoin_core: Option<PathBuf>,
    },
    Catalog {
        #[command(subcommand)]
        command: catalog_commands::CatalogCommand,
    },
    Matrix {
        #[command(subcommand)]
        command: catalog_commands::MatrixCommand,
    },
    Targets {
        #[command(subcommand)]
        command: catalog_commands::TargetsCommand,
    },
    /// Content-addressed input storage. No installer is executed.
    Objects {
        #[command(subcommand)]
        command: Objects,
    },
    /// Isolated process primitives. These commands do not select a miner target.
    Process {
        #[command(subcommand)]
        command: Process,
    },
    /// Private worker control. No public control listener is provided.
    Worker {
        #[command(subcommand)]
        command: Worker,
    },
    /// Freeze or check the current simulator source files.
    Inputs {
        #[command(subcommand)]
        command: Inputs,
    },
}

#[derive(Subcommand)]
enum Demo {
    /// Archived S9 firmware, private native browser, fresh mining and Core checks.
    S9 {
        /// Verify the frozen inputs without starting firmware.
        #[arg(long, conflicts_with = "out")]
        check: bool,
        /// New directory for complete run evidence.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum Objects {
    Hash {
        path: PathBuf,
    },
    Import {
        path: PathBuf,
        #[arg(long)]
        sha256: String,
        #[arg(long)]
        size: u64,
    },
    Verify {
        #[arg(long)]
        sha256: String,
        #[arg(long)]
        size: u64,
    },
}

#[derive(Subcommand)]
enum Process {
    /// Execute a closed set of hashed files with per-process limits.
    Execute {
        spec: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Require aggregate limits under this delegated cgroup-v2 parent.
        #[arg(long)]
        cgroup_parent: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum Worker {
    Serve {
        #[arg(long)]
        socket: PathBuf,
        /// Frozen JSON policy for input storage and retained run evidence.
        #[arg(long)]
        limits: Option<PathBuf>,
    },
    Request {
        #[arg(long)]
        socket: PathBuf,
        file: PathBuf,
    },
    Upload {
        #[arg(long)]
        socket: PathBuf,
        path: PathBuf,
        #[arg(long)]
        sha256: String,
        #[arg(long)]
        size: u64,
    },
}

#[derive(Subcommand)]
enum Inputs {
    Freeze {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Verify {
        manifest: PathBuf,
        #[arg(long, default_value = ".")]
        path: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok((value, code)) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("serialize command result")
            );
            std::process::exit(code);
        }
        Err(error) => {
            let code = if error.chain().any(|cause| cause.is::<std::io::Error>()) {
                4
            } else {
                2
            };
            eprintln!(
                "{}",
                json!({"status":"error","error":format!("{error:#}"),"exit_code":code})
            );
            std::process::exit(code);
        }
    }
}

fn dispatch(cli: Cli) -> Result<(Value, i32)> {
    let state = &cli.state_dir;
    match cli.command {
        Command::Demo {
            command: Demo::S9 { check, out },
        } => demo::s9(check, out),
        Command::Doctor { bitcoin_core } => {
            let report = doctor::inspect_with_core(bitcoin_core.as_deref());
            let code = if report["infrastructure_incomplete"] == true {
                4
            } else {
                0
            };
            Ok((report, code))
        }
        Command::Catalog { command } => catalog_commands::catalog(command, state),
        Command::Matrix { command } => catalog_commands::matrix(command, state),
        Command::Targets { command } => catalog_commands::targets(command, state),
        Command::Objects { command } => {
            let store = Store::open(&state.join("store"), 8 * 1024 * 1024 * 1024)?;
            let value = match command {
                Objects::Hash { path } => {
                    json!({"path":path,"observed_identity":hash_file(&path)?,"authenticity_claim":false})
                }
                Objects::Import { path, sha256, size } => {
                    let id = ObjectId { sha256, size };
                    let saved = store.import(&path, &id)?;
                    json!({"verified":id,"stored_path":saved})
                }
                Objects::Verify { sha256, size } => {
                    let id = ObjectId { sha256, size };
                    let saved = store.verified_path(&id)?;
                    json!({"verified":id,"stored_path":saved})
                }
            };
            Ok((value, 0))
        }
        Command::Process { command } => match command {
            Process::Execute {
                spec,
                out,
                cgroup_parent,
            } => {
                let spec: ProcessSpec = serde_json::from_slice(&fs::read(spec)?)
                    .context("read process specification")?;
                spec.validate()?;
                fs::create_dir(&out)
                    .context("result directory must be new and have an existing parent")?;
                let store = Store::open(&state.join("store"), 8 * 1024 * 1024 * 1024)?;
                isolation::save_json(&out.join("process-spec.json"), &spec)?;
                isolation::save_json(
                    &out.join("lock.json"),
                    &json!({"schema_version":"2.0","specification":hash_file(&out.join("process-spec.json"))?,"runtime":hash_file(&std::env::current_exe()?)?,"aggregate_cgroup_parent":cgroup_parent,"claim":"isolated-process primitive; no miner target or gate pass"}),
                )?;
                let result = match isolation::execute_dashboard_recorded(
                    &store,
                    &spec,
                    Arc::new(AtomicBool::new(false)),
                    None,
                    cgroup_parent.as_deref(),
                    cgroup_parent
                        .as_ref()
                        .map(|_| out.join("cgroup.json"))
                        .as_deref(),
                    Some(&out.join("dashboard.json")),
                ) {
                    Ok(result) => result,
                    Err(error) => {
                        let failure = json!({"status":"infrastructure-failed","error":format!("{error:#}"),"firmware_gate":"not-run"});
                        isolation::save_json(&out.join("result.json"), &failure)?;
                        return Ok((
                            json!({"result":out.join("result.json"),"failure":failure}),
                            4,
                        ));
                    }
                };
                let code = if result.status == "exited" && result.exit_code == Some(0) {
                    0
                } else {
                    1
                };
                isolation::save_json(&out.join("result.json"), &result)?;
                Ok((
                    json!({"result":out.join("result.json"),"status":result.status,"process_exit_code":result.exit_code,"trace_truncated":result.trace_truncated,"firmware_gate":"not-run"}),
                    code,
                ))
            }
        },
        Command::Worker { command } => match command {
            Worker::Serve { socket, limits } => {
                let limits = match limits {
                    Some(path) => {
                        anyhow::ensure!(
                            fs::metadata(&path)?.len() <= 16384,
                            "worker limits file is too large"
                        );
                        serde_json::from_slice(&fs::read(path)?)?
                    }
                    None => worker::WorkerLimits::default(),
                };
                worker::serve_with_limits(&state.join("worker"), &socket, limits)?;
                Ok((json!({"stopped":true}), 0))
            }
            Worker::Request { socket, file } => {
                let request: Request = serde_json::from_slice(&fs::read(file)?)?;
                Ok((worker::request(&socket, &request)?, 0))
            }
            Worker::Upload {
                socket,
                path,
                sha256,
                size,
            } => {
                let id = ObjectId { sha256, size };
                worker::upload(&socket, &path, &id)?;
                Ok((json!({"destination_verified":id}), 0))
            }
        },
        Command::Inputs { command } => match command {
            Inputs::Freeze { path, out } => {
                let manifest = source_manifest::collect(&path)?;
                isolation::save_json(&out, &manifest)?;
                Ok((
                    json!({"manifest":out,"identity":manifest.identity,"files":manifest.files.len()}),
                    0,
                ))
            }
            Inputs::Verify { manifest, path } => {
                let expected: source_manifest::SourceManifest =
                    serde_json::from_slice(&fs::read(manifest)?)?;
                let actual = source_manifest::collect(&path)?;
                if expected != actual {
                    Ok((
                        json!({"status":"fail","reason":"source state differs from manifest","expected":expected.identity,"actual":actual.identity}),
                        1,
                    ))
                } else {
                    Ok((
                        json!({"status":"pass","identity":actual.identity,"scope":"simulator source identity only"}),
                        0,
                    ))
                }
            }
        },
    }
}
