# Checkpoint500: tested CLI and user-requested pause

Status: **paused at the user's request after the complete CLI test passed**.
The full simulator task and milestone2 remain incomplete. Do not resume
implementation until the user asks to continue.

The user requested: "finish the test than save checpoint and pause".
Run500, its independent reviews and the checkpoint checks are complete.
On27 October, the user requested a Git commit so this checkpoint can be pushed
to another repository. This commit includes the simulator source, compact
reports, resume records and selected build manifests. Its parent is`b276dd7a`.
Implementation remains paused.

Git excludes original downloads, executable tools, disk images, browser
profiles and raw run data. Keep those files and the linked external archive
when transferring the runnable workspace. A Git clone alone does not contain
the prepared execution environment. The acquisition and build records retain
the required identities.

## Passed working path

The [complete test report](reports/shared-cli-pass-500.md) contains the actual
commands, exact inputs, selected run, mining evidence and remaining limits.
Original archived Braiins S9 firmware runs on the declared adapted reference
model, with three chains of63 BM1387 chips and a10H/s cap. It is not an exact
physical target or an original-board boot.

Run500 checks1,346 actual computations,15 firmware submissions, two fresh
jobs, a clean-job transition and a difficulty change. Nine shares pass; six
fail for low difficulty. Two firmware-produced blocks pass Core acceptance
and keep their exact tips after restart. The native browser passes through
SSH and authenticated TLS for that same run. Artifact transfer, fresh-input
provenance, isolated cookies, live telemetry, configuration restoration,
clock bounds, retained results and cleanup are independently reviewed.

The CLI generates a new seed and time for every invocation. Only those pool
fields and the corresponding mount change. The parent input remains intact.
Full source checks pass:1,069 tests, zero failures,49 declared ignored entries,
118 suites, strict Clippy, formatting and release build. Ignored tests are not
execution passes. Final guide checks pass571 links with zero failures. All21
links in this checkpoint and the full test report also resolve. Only README
documentation changed in the frozen source closure after the code checks.

```sh
work/simulator/cli-tools-500/md-sim demo s9-shared --check
work/simulator/cli-tools-500/md-sim demo s9-shared --out work/simulator/demos/s9-shared-500
```

These exact commands passed. The output path exists; use a new path to rerun.
Keep the prepared workspace and external archive links available. Clean
installation is not yet packaged. The private test URLs stop with the suite.

## Resume from this state

1. Read this checkpoint, the README, `AGENTS.md` and
   [the26 October continuation](continuation-2026-10-26.md). Preserve the
   passed500 cohort and earlier failures. No new implementation is required
   to repeat500 with its pinned inputs.
2. Continue exact FPGA validation from
   [HDL502](reviews/hdl-validation-resume-502.md). The actual497 conversion
   passed and is independently reviewed. HDL execution has not started.
   The gperf transfer is a failed partial file. Obtain a complete source in a
   new path and validate it before an isolated build. Preserve the partial
   transfer evidence. The vendor PS7 source has no behavioral CPU model;
   retain the explicit boundary with the existing CPU emulator.
3. Continue BM1397 state work from the
   [reviewed501 checkpoint](reviews/bm1397-programmed-checkpoint-501.json).
   Its [source patch](reviews/bm1397-programmed-501.patch) is saved separately.
   It was applied in a temporary copy and all resulting hashes matched.
   It is not integrated or executed. Check the production base hashes before
   applying it, then run its listed tests and original-firmware probe.
4. Continue the full specification. Exact controller/hashboard/PSU identities,
   validated FPGA behavior, physical timing and original-board gates remain
   open. The frozen matrix has79,599 rows and zero complete exact-target
   passes. Full per-target suites, clean installation and the remaining
   dashboard/remote workflows are still required.

Do not repeat completed FPGA frame/FASM research, PLL496 execution, or500
debugging without new evidence. Their reports retain exact inputs and results.
Do not treat the successful adapted path as an exact-target exclusion or pass.

Git exclusions are checked in the
[checkpoint regression](reviews/git-ignore-checkpoint-regression-501.json):
14 intended source/checkpoint paths are eligible and22 archive, run-store,
binary, profile and credential paths are excluded. The501 patch has one narrow
exception. Original archive files, unique run evidence and user changes stay
preserved. No runtime or compiler process is left running by this task.
