# Independent recovered-run mining and Core review 466

The recovered mining and Core checks pass for the retained native S9 run 453.
The original browser and client gateway shutdown acknowledgment remain failed.
Recovery 461 did not execute the firmware again or change its original result.
No original-board, exact-target or full remote-dashboard gate is granted.

The [copy review](shared-recovery-copy-independent-461.json) independently
rehashed all 43 original and copied files, totaling 1,038,030,428 bytes. Every
copy has a distinct inode and one link. Both failed original verdicts remain
unchanged. The source 453 directory still has no added pool-result.json. Only
the separate derived root contains the recovered observation. The unchanged
strict verifier accepted that observation before mining replay.

The [mining review](shared-recovery-mining-independent-461.json) independently
reconstructed all 12 submitted headers from the actual raw Stratum session 0.
It used SHA-256d from a separate standard hashing library and exact rational
pool difficulty. All 12 headers and digests match the Rust audit and actual
hardware-result correlations. Six shares were accepted and six were rejected
as LowDifficulty. Two fresh jobs, a clean-job transition and a difficulty
change are present. Each candidate's coinbase, merkle root and block target
were checked. This review does not claim invalid/duplicate/stale/out-of-range
rejection coverage from this session.

The strict Rust audit checked 1,349 actual computations, 12 firmware submissions
and 12 complete hardware serial results. The programmed/adapted filtering and
10 H/s aggregate cap remain explicit. Minimum recorded grant separation was
131,057,633 ns against the 100,000,000 ns requirement. Physical timing and exact
board identities remain unvalidated.

The [Core review](shared-recovery-core-independent-466.json) checked both new
isolated node runs. It rehashed their candidate bytes, source/audit links,
copied source evidence, process specifications, result/log envelopes and
actual mounted Core executable. The daemon is the unchanged archived-source
build a22238f0a1ea0b0a67a17032d2df0039e9d3ac946f2e86e0634ea47448692fae.
Each run starts from genesis height 0, imports the firmware-produced candidate
to height 1, stops cleanly, then restarts without loadblock and loads the same
height 1 tip. All three phases stop cleanly. Core generated neither candidate.
The original 32-process bound remains in the new process specifications.

| Candidate | Accepted and retained block hash |
| --- | --- |
| 0 | `1c5d5a21017e0d57efd5a80fde5a88f965b65dcd1d97352361538927bdf0aded` |
| 1 | `06024c0b03f2cb77c1040fcdb38fc05c15013f9f48980aad7f7503ab917403e3` |

Core attempts 461 and 464 failed before node execution and remain failures.
The inspector 464 and runtime 466 bootstrap fixes moved the same NPROC bound
after trusted user-namespace creation. The [inspector review](elf-inspector-independent-464.md)
and [runtime review](runtime-nproc-independent-466.md) record their source and
actual regression checks. Fresh Core results 466 establish acceptance only
after those fixes; they do not relabel the failed earlier attempts.

The final adjacent manifest pins these reviews, the source recovery record,
both strict audits and both Core acceptance reports. The full simulator and
milestone 2 remain incomplete.
