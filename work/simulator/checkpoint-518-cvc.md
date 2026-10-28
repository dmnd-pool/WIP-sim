# CVC component checkpoint516f

The user requested a checkpoint on28October because the weekly token balance is
low. The active serial cohort finished. No selected CVC HDL run started after
that request. No process, compiler or guest remains active from browser_resume.
The complete S9 milestone2 is **incomplete**. Resume S9 first; stop when that
complete milestone passes. Other miners remain outside this continuation.

## Tested state

The existing Icarus path still passes the selected72-operation FPGA test. The
shared C/VPI adapter and Verilog2005 clock wrapper passed actual execution in
`work/simulator/qemu-s9/live-icarus-516d`. The compiled CVC attempt
`live-cvc-516d` failed before simulation on an existing scanner defect. Both
results and exact original vendor inputs remain unchanged. Their joined record
is `work/simulator/reviews/live-engines-executed-516d.json`.

The small original-tool run `cvc-lexer-original-516e` reproduced the same3403
invalid-attribute error at ordinary `always @(*)` after an inactive conditional
inside an unused library module. The same literal fixture passes in Icarus.
The failure retains both source files and its real Icarus program. The independent
review is `reviews/cvc-lexer-original-independent-execution-516e.json`.

The separately derived existing CVC tool now has two recorded repairs:

- `rw-after-nba.patch`: process complete read/write callback batches after
  prior nonblocking assignments and before read-only observation callbacks.
- `lexer-context.patch`: restore the caller's scanner context after an inactive
  conditional. Vendor HDL and signal/timing semantics are unchanged.

The source archive is unchanged. The changed C files retain the original license
and dated modification notices. The build exports both patches, changed files
and separate adaptation manifests. All C compiler commands use undefined-behavior
trap checks. Original compiler warnings remain in the retained build report.

The combined build `qemu-s9/cvc-build-516f` passed in52,647ms, exit0 and
untruncated. Both patches, smoke tests, archive restoration and negative syntax
checks passed. Independent review:
`reviews/cvc-combined-build-independent-execution-516f.json`.

Exact tool archive:
`work/simulator/qemu-s9/cvc-build-516f/cvc-tools.tar.gz`
SHA256`9a5ed2d7e4c5d5ca0f26f5009265bda9188dfd314a64e01ad9d8922d0239d41f`,
5,382,343bytes. Its compiler binary SHA256 is
`af75045efe3a1809d2fb1d40fd496cae76ce1643e8688fd548ff7469809acaf2`.

The final helper cohort is `qemu-s9/cvc-combined-driver-516f/manifest.json`.
All622 captured source inputs match before and after the release build.
Nine probe tests and30 live-driver tests pass;3 explicit tests are ignored.
Strict scoped Clippy and formatting pass. Root owns final full-workspace checks.

- `cvc_rw_probe`: SHA256
  `a8a902170c431e88dfdb31df9479332506b52162708995e822aea4b3824b504e`.
- `s9_hdl_validate`: SHA256
  `2f6aea617e88db0addb5ea3d7ad8c59b4cb699da62ab2219a15441ac19ec790e`.

The unchanged literal scanner fixture passes on both engines at3ps. All33
scheduler comparisons also pass on the combined tool:3 original oracle runs,
12 ordinary-assignment cases,12 delayed-assignment cases and6 no-assignment
cases. All five invocations exit0, with untruncated results and retained actual
compiled programs. The joined report contains exact commands and input hashes:
`work/simulator/reviews/cvc-combined-components-executed-516f.json`.
The earlier original-tool and report-label failures remain failed.

## Next action after resume

First read the root checkpoint, current repository instructions and this record.
Check the independent combined-component audit when it is available. Do not
repeat the completed source research without a reason. Verify the frozen helper,
profile and referenced input hashes before execution.

The following command is **prepared but has not run**:

```sh
work/simulator/qemu-s9/cvc-combined-driver-516f/s9_hdl_validate \
  /home/user/src/md-sim \
  simulator/firmware/s9-hdl-inputs-516f-cvc.json \
  work/simulator/qemu-s9/live-cvc-516f
```

The output path did not exist at this checkpoint. Use a new path if it later
exists. The new profile and its exact driver hash are source-reviewed; the
actual selected-model result is still untested. Run only after other heavy
builds and guests stop. Keep the existing900s wall,600s CPU,4GiB memory,1GiB
scratch and strict diagnostic gates. Do not suppress new diagnostics or modify
vendor model logic to make compilation pass.

Required next results are582 primitive vectors, both510-case RAM forms, the
unchanged VPI event/X/Z and duplicate-failure cases, split/unsplit/unknown-start
clock cases, and all72 real selected AXI operations. Retain the first failure
and investigate it. A compiler process starting is not HDL execution.

After actual selected CVC behavior passes, measure its real fabric throughput
with explicit clock/next-edge checks. Icarus514 could not complete100us within
the unchanged10-second command bound; ordinary clocks must not be slowed or
silently stopped. The CVC performance gate is still open.

Then resume the staged CPU-to-FPGA bridge under`work/simulator/s9-cosim-514`.
Use the current S9 ROM/backend source base from the exact-target agent. Bind
actual FCLK1, reset0 and IRQ signals. The first explicit atomic-MMIO reference
contract is separate from a timed CPU stall. The reviewed TCG virtual-clock API
is not a CPU-stall implementation. No CPU/board/mining acceptance follows from
the component tests above.

The unchanged original S9 reset path and laterSLCR0x500 work are owned by root
and exact_target_resume. Read their current checkpoint before rebasing any QEMU
patch. Preserve all historical originals, failed runs and independent reviews.
