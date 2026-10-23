//! Commands sent to the unchanged native shell, never executed on the host.
use anyhow::{ensure, Result};

pub const CONSOLE: &str = "simulator/scenarios/s9-private-pool.sh";

/// Require the previously saved mining configuration without rewriting it.
/// Network setup is deliberately unchanged. This does not claim network
/// persistence across boots with an unprovisioned factory identity.
pub fn persisted_pool_console(parent: &str) -> Result<String> {
    let mut resumed = parent.to_owned();
    for (before, after) in [
        ("# This runs in the original guest before its START=99 miner service.\n",
         "# This restart uses the original guest's persisted native service configuration.\n"),
        ("[ -z \"$(pidof bosminer || true)\" ]\n[ ! -e /etc/bosminer.toml ]\n", ""),
        ("(\nset -C\ncat /tmp/md-sim-pool.toml > /etc/bosminer.toml_tmp\ncat /tmp/md-sim-pool.toml > /etc/bosminer.toml\n)\n", "cmp /tmp/md-sim-pool.toml /etc/bosminer.toml\n"),
    ] {
        ensure!(resumed.matches(before).count() == 1,
            "console template changed; review persisted mining configuration");
        resumed = resumed.replacen(before, after, 1);
    }
    Ok(resumed)
}
