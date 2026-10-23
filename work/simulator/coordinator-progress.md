# Locked source execution coordinator

The full simulator release remains incomplete. This work adds real dispatch for a source interface fixture. It does not convert catalogue candidates into supported physical boards.

## Implemented path

- Version 2 locked plans include the full ProcessSpec, all input identities, suite, verifier, backend tools, seed, cap, clocks, policies and resource limits. Resume also binds the executor build and deployment identity.
- Durable attempts preserve failed and interrupted evidence. A missing initial lock is recorded as interrupted. Changed evidence cannot satisfy resume. Required gate evidence must name saved files.
- Bounded verifier exports and aggregate attempt admission prevent unlimited retained results. The private queue and content stores have exclusive writers.
- Local execution uses the real isolated ProcessSpec executor. Worker execution transfers each unique input with destination hash checks, starts the same ProcessSpec, retrieves exact result chunks and acknowledges cleanup after syncing result bytes and their directory entry.
- The worker request token is saved before ExecuteToken. LookupToken can associate a lost reply with the original worker run. Cleanup tombstones prevent accidental duplicate execution. Repeated partial retrievals retain distinct files.
- Cancellation is checked between transfer chunks and during guest execution. Each attempt uses fresh guest scratch.
- The cgminer adapter freezes the prepared C binary, archived source, exact preparation runner, build evidence, full captured toolchain bundle, ELF runtime inputs and Rust suite/verifier. It runs actual source firmware over private IPC and a pseudo-terminal.
- The verifier reconstructs submitted headers independently, replays serial CRC/work/result data, checks programmed ASIC filters, and tests duplicate, invalid, stale, out-of-range and above-target rejection. Regtest exports remain candidates until Bitcoin Core accepts them.

## Checks and current status

- Eight coordinator behavioral tests pass. They use actual isolated processes. They cover resume and tampering, missing evidence, export and retained byte/count limits, interrupted initialization, private worker transfer and cleanup, identity changes, cancellation after real child output, and reply loss at ExecuteToken, ResultData and Cleanup.
- `cargo test -p md-sim-coordinator -p md-sim-mining -p md-sim-firmware --lib --tests --offline --locked` passed. Log: `work/simulator/reports/coordinator-related-tests-006.log`.
- `cargo clippy -p md-sim-coordinator -p md-sim-mining -p md-sim-firmware --all-targets --offline --locked -- -D warnings` passed. Log: `work/simulator/reports/coordinator-clippy-006.log`.
- Root review findings are fixed: malformed recipes cannot panic; serial CRC, work and result packets are independently replayed; closure output cannot modify the captured input tree; trace sequence and time bounds are checked.
- Catalogue review findings are fixed: required evidence references are validated; exports are bounded; lost initialization is retained; worker tokens are committed before dispatch; repeated partial retrievals are retained; result directory entries are synced before remote cleanup.
- Plans 001 and 002 remain development records. Plan 003 executed successfully before the last review fixes. Plan 004 correctly failed verification when the old replay code ignored a real stale submission after a clean job. The corrected verifier follows the ordered Stratum trace and retains the stale rejection. A regression covers this case.
- Plan 005 was blocked by an exact build identity change before execution. Shared `target/release` files had been rebuilt. The copied binaries in `work/simulator/coordinator/binaries-005/` now isolate plan and run identities from later builds.
- Plan 006 passed the declared five-gate source suite locally and through a separate private IPC worker. The complete preparation and runtime inputs were transferred with destination hash checks. Both repeat commands returned `reused: true` for the original attempt. No extra firmware execution was used for resume.

## Verified plan 006 commands and identities

Run these commands from the repository root. The plan command requires a new queue path. The copied binaries retain the exact tested builds.

```sh
work/simulator/coordinator/binaries-005/cgminer-coordinator plan-regtest work/simulator/firmware/prepared-closed-002 work/simulator/firmware/toolchain-snapshot-003 artifacts/files/source/open-ckolivas-cgminer-source/cgminer-b8491c66e7e2.tar.gz work/simulator/firmware/preparation-runners/3a44a1066bc68239767e87d39fd3ddd6ba7e96b39965505a0f15d1cff20697d1.elf work/simulator/coordinator/queue-006
work/simulator/coordinator/binaries-005/cgminer-coordinator run work/simulator/coordinator/queue-006 local
work/simulator/coordinator/binaries-005/cgminer-coordinator run work/simulator/coordinator/queue-006 worker work/simulator/coordinator/worker-006
```

The plan hash is `43595ba4387c7e840e7cf01cf8f4f4c65da7fdf972e743786f2fed9fc1b51612`. The complete run hash is `c197c45eaf1a9c24f0eb98d9509715bd30363b84f1aff3bff60dc72748decebf`. The coordinator/verifier binary hash is `5803beadce75d3a1fef33bbad843a442933b294077e48b0b543e7add60f153b6`. The harness hash is `ae45a9fbad942f75d427c0641f7ab698b8f41877eb9b52c8be200410b168735e`. The worker hash is `44a6c802a4f8f495723d616b12c20c0222ed7a30de5ead504edc911cc497d3ba`.

Reports are `work/simulator/reports/coordinator-{local,worker}-006.json` and `coordinator-{local,worker}-resume-006.json`. The two sealed attempts are under `queue-006/jobs/<run hash>/attempt-000001` and `attempt-000002`. Their evidence includes the actual guest log, serial work/results, Stratum messages, independent header and serial replay, preparation identities and regtest block candidates. The suite reports zero exact physical targets and `complete_hardware_matrix: false`.

## Next action

Root executed all four new block candidates from both attempt000001 and
attempt000002 through the pinned Bitcoin Core build. All four passed import
and restart persistence at height one. Reports are under
`work/simulator/bitcoin-core/coordinator-006-attempt-{1,2}-candidate-{0,1}/`.
Each has the exact source submission and input links. These remain source
fixture results, not physical target coverage.

Incremental review found that ordered pool replay must also bind every
`mining.notify` field and the current announced difficulty to the saved job.
Job ID and clean flag alone are insufficient. This finding was fixed and
rerun as plan 007 below. Plan 006 remains historical evidence.

Test the SSH executor through the isolated guest network when its transport is ready. Resolve an exact Bitaxe hardware/firmware identity and build its archived ESP QEMU backend. Do not claim TCP, SSH, TLS, external physical-host, original-board or full-matrix acceptance from the private IPC source suite.

## Incremental wire review and plan 007

Root review found that plan 006's verifier bound job IDs and submissions but did not bind every notification field or the announced difficulty. The pool itself rejected stale work correctly. The verifier now correlates the subscription response, checks both extra nonce fields, checks every job notification field, and derives the target from the preceding exact supported wire difficulty. It also checks the complete reported schedule and job limits against the locked seed, epoch, cap, exponents and anchor. Pool trace time must fit the ProcessSpec wall budget.

The regression changes each notification field, subscription ID, extra nonce and size, wire difficulty, time bound and clean-job order. Each change is rejected. Firmware library tests and strict Clippy pass in `work/simulator/reports/coordinator-wire-tests-007.log` and `coordinator-wire-clippy-007.log`. Root reviewed the fix. The catalogue agent separately reviewed the ordered clean-job and serial sequence/time checks.

Plan 007 passed locally and through the private worker with the stronger verifier. Both repeat commands reused the exact sealed results. Its plan hash is `57c0e27d5f63fba3d080efedf9c309c496e27a8c869d969b5a865962f2c364c6`. Its run hash is `b9760042cf355469958689f430206d9617e3c2a215c9065904a2f65afaf02d05`. Reports use the same names as above with suffix `007`. The frozen command directory is `work/simulator/coordinator/binaries-007/`; the queue and worker roots are `queue-007` and `worker-007`.

The tested coordinator hash is `c1e8aa265c6fda1ea3ac8669cede4b39c16d71dc5e916f7a4ed779bf308e5ae2`. The harness hash is `db3dd4b660971bcb58612a6cc27c0480007023840634deb132e4ac475a890fe9`. The worker hash is `8030ba0461b12a11076241c87132625a5c103c61f9abda4a2e9daef026793da0`. Use the same plan and run command forms above with these `007` paths. The original 006 evidence remains intact. Both 006 and 007 now have separate four-candidate Bitcoin Core acceptance and restart evidence, identified below.

Duplicate intermediate toolchain tar files were hash-checked against each final locked queue object before removal. The exact cleanup manifest is `work/simulator/coordinator/staging-bundle-cleanup-001.json`. No unique source input, final queue object, firmware evidence or archive file was removed. New plans remove this temporary duplicate only after final content-store verification and record `bundle-cleanup.json` in their staging directory.

Root verification after the full notify/difficulty trace binding correction:
queue-007 has two actual source firmware attempts (local and private worker).
Each produced two fresh candidate blocks. All four passed the pinned Core
three-phase import and restart check in bitcoin-core/coordinator-007-attempt-
{1,2}-candidate-{0,1}/block-acceptance.json. Each check links the exact firmware
submission and report, independently reconstructs the header, and observes the
height-one tip after restart. This remains a source fixture, not a resolved
physical board or complete matrix result.
