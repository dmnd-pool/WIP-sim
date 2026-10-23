use anyhow::{ensure, Result};
use std::path::Path;
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() >= 2,
        "usage: cgminer-lab prepare REPOSITORY OUTPUT [--ubsan]"
    );
    match args[1].as_str() {
        "__prepare" => {
            md_sim_firmware::prepare::inside(args.get(2).is_some_and(|x| x == "--ubsan"))
        }
        "prepare" => {
            ensure!(
                args.len() == 4 || (args.len() == 5 && args[4] == "--ubsan"),
                "usage: cgminer-lab prepare REPOSITORY OUTPUT [--ubsan]"
            );
            md_sim_firmware::prepare::prepare(
                Path::new(&args[2]),
                Path::new(&args[3]),
                args.len() == 5,
            )
        }
        "run" | "run-regtest" => {
            ensure!(
                (4..=8).contains(&args.len()),
                "usage: cgminer-lab run PREPARED OUTPUT [CAP_HPS] [SEED_HEX] [EPOCH] [DIFFICULTY_EXPONENT]"
            );
            let cap = args
                .get(4)
                .map(|x| x.parse())
                .transpose()?
                .unwrap_or(10_000.0);
            md_sim_firmware::lab::run(
                Path::new(&args[2]),
                Path::new(&args[3]),
                cap,
                args.get(5).map(String::as_str),
                args.get(6).map(|x| x.parse()).transpose()?,
                args.get(7)
                    .map(|x| x.parse())
                    .transpose()?
                    .unwrap_or_else(|| md_sim_firmware::schedule::automatic_exponent(cap)),
                args[1] == "run-regtest",
            )
        }
        _ => anyhow::bail!("unknown command"),
    }
}
