# Work tracks: easy mode and hard mode

Started on 30 October 2026. This file sets the scope of each track and the
rules for parallel work.

Simulator work has two separate tracks. They share the Rust device models.
Each track has its own goal, milestones, work folder and new code.

| Item | Easy mode | Hard mode |
| --- | --- | --- |
| Goal | Run each miner's real firmware, to test a mining pool later | Faithful hardware simulation, for study and for writing new miner firmware |
| Execution | Adapted environments are allowed: process-level runs, direct boot, low-rate filters. Label each adaptation. | Original boot path, original circuits and original programs, where the files contain them |
| Evidence | Independent share and block checks | Ground truth from files: circuits rebuilt from bitstreams, the original BootROM and PIC programs, firmware drivers, vendor test code, datasheets |
| Guide | [Easy-mode guide](easy-mode.md) | [Hard-mode guide](hard-mode.md) |
| Milestones | [Easy-mode milestones](../work/easy/milestones.md) | [Hard-mode milestones](../work/hard/milestones.md) |
| Handoff | [Easy-mode handoff](../work/easy/handoff.md) | [Hard-mode handoff](../work/hard/handoff.md) |
| New code | `crates/md-sim-easy*` | `crates/md-sim-hard*` |
| Run data | `work/easy/runs/` | `work/hard/runs/` |
| Build directory | `CARGO_TARGET_DIR=work/easy/target` | `CARGO_TARGET_DIR=work/hard/target` |

## Rules for both tracks

- Do not use hardware access. Do not capture, measure, scan or flash a
  physical miner, and do not buy one. Files are the only evidence.
- Do not invent shares, nonces or chip replies. Label every adaptation.
- The `work/simulator/` records are history. Read one only when a task needs
  it. Do not load the full README or the progress history into each session.
- Run long or repeated jobs as scripts outside the agent session. Each job
  writes one small JSON summary. Read the summary, not the raw logs.
- Use automated tests and comparisons for each step. Do one review at the
  end of each milestone.
- Keep each handoff file under two pages.

## Parallel work in one checkout

Two sessions can work at the same time in this checkout:

1. Edit only the paths of your track (table above). These paths are shared:
   existing crates, `Cargo.toml`, `Cargo.lock`, `README.md`, `AGENTS.md`,
   `docs/simulator-specification.md` and `catalog/`. Change a shared path in
   one short step, run the full Rust checks, and commit at once.
2. Commit only your own paths: `git commit -m "..." -- <paths>`. Do not use
   `git add -A`, `git commit -a`, `git stash`, `git reset` or `git checkout`
   on files that you did not change.
3. Use the build directory of your track. Then the two tracks do not block
   each other's builds.
4. The hard track owns the S9 QEMU machine sources and the behavior of the S9
   device models. The easy track uses frozen builds of them. It writes a
   request in the hard-mode handoff when it needs a change.
5. Write each change that affects the other track in that track's handoff.

## Gates by track

The gate table in the
[specification](simulator-specification.md#19-acceptance-tests-and-gates)
still applies, with these rules:

- Easy mode: a labelled direct boot is allowed (G6). G7 and G8 are
  `not-applicable` for pool testing. Record the reason.
- Hard mode: G7 and G8 compare the models with file-based ground truth.
  Record the source and confidence of each value. Measured hardware data is
  not required and not available.
- The full-matrix completion rule in the
  [implementation prompt](implementation-prompt.md) applies to the final full
  specification only. It does not apply to track milestones.
