# Paused S9 checkpoint518

**30 October 2026:** the next actions in this file are replaced by
[the hard-mode handoff](../hard/handoff.md). Keep this file as history.

The user requested this checkpoint because the weekly allowance is low.
All active component runs finished. No new selected CVC run started after
that request. Work is paused after the final checks and Git commit.
S9 milestone2 is **incomplete**. Other miner work remains paused.

The original public ROM now passes the reset-reason, LVTTL and PS GPIO paths.
The GPIO suite passes144 actual transactions,14 interrupt transitions and two
warm resets. Its rejection suite passes nine faults. A separate per-pin model
checks the results. The fresh ROM diagnostic stops at SLCR0x500. It does not
boot SPL or the complete firmware. See the
[execution report](reviews/ps-gpio-executed-513j.md),
[independent audit](reviews/ps-gpio-independent-execution-513j.json) and
[exact-target handoff](reviews/exact-s9-checkpoint-513l.json).

The shared live driver passes the selected72-operation FPGA regression with
Icarus. The first CVC attempt failed before simulation. The separately recorded
CVC scheduler and parser repairs now pass the unchanged parser fixture and all33
scheduler comparisons. Original failed attempts remain failed. See the
[combined results](reviews/cvc-combined-components-executed-516f.json),
[independent audit](reviews/cvc-combined-components-independent-execution-516f.json)
and [exact FPGA resume commands](checkpoint-518-cvc.md).
Selected S9 execution and throughput on the repaired CVC tool remain untested.
CPU/FPGA coupling, full original-board boot, exact component binding,
physical calibration and mining through that exact path remain open.
Historical adapted S9 mining/dashboard results stay separate.

The narrow storage relocation utility passes its component and actual namespace
checks. It has not moved or removed real run data. See the
[storage checkpoint](reviews/runtime-relocation-checkpoint-517.md).
At checkpoint preparation, root has about28GiB free; `/tmp` has9.5GiB free.
Storage is not blocking work. All owned emulator/compiler processes stopped.

The SLCR0x500 increment is saved under
[debug-ref-dev-513k](debug-ref-dev-513k/DEBUG-REFERENCE.md).
Its C patch and Rust test plan passed independent source review. They are not
integrated, built or executed. The dispatcher wiring is incomplete and has not
been reviewed or compiled. Its [status](debug-ref-dev-513k/wiring-work-in-progress.json)
records the exact next edits. Keep its full-word reference contract and default
machine behavior distinct. Do not infer physical debug or security effects.

Final Rust formatting, strict Clippy and all-target test results, source hashes
and documentation link checks are in the
[compact manifest](source-checkpoint-518/manifest.json).
Ignored integration helpers are explicit; they do not count as execution passes.
No final full-matrix or clean-install acceptance is claimed.

After the user resumes, first verify this checkpoint and the frozen helper/input
hashes. Run the prepared selected-CVC command in the FPGA handoff. If it passes,
measure its real throughput before CPU coupling. Complete the reviewed0x500
builder/dispatcher/probe wiring and its actual isolated tests in a separate
runtime slot. Then advance the unchanged ROM from its next real failure.
Do not repeat completed research without a reason.

Original archives, linked payload storage, disks, executables, credentials and
raw runtime data remain excluded from Git. Keep them in the workspace archive.
Copy linked archive contents as well if moving execution to another host. The
Git checkpoint includes source, patches, compact reports and pending source work.
It alone does not contain the firmware/tool payload archive.
