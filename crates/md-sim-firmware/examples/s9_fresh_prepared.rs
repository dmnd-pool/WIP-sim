//! Copy a verified prepared S9 reference process with new pool entropy and time.
use anyhow::{ensure, Result};
#[allow(dead_code)] // The common pool module also serves execution transports.
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[path = "support/prepared_board.rs"]
mod prepared_board;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 3,
        "usage: s9_fresh_prepared SOURCE NEW_OUTPUT"
    );
    let receipt = prepared_board::fresh::prepare(args[1].as_ref(), args[2].as_ref())?;
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}
