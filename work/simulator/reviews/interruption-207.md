# Author review: controlled storage interruption

This is an author review. The independent review gate remains open. All three
review agents have the same service quota block until 25 October 2026.

The selected helper must own a live emulator. It sends SIGKILL, drains the
serial pipe to EOF, and checks the reaped signal. An old exit, a different
signal, changed serial bytes, or native restart text fails the check. Tests
exercise actual owned shell processes. They are component tests, not firmware.

Snapshot schema 4 separates the interruption from schema 3 native restarts.
Both media must refer to the same observed termination, execution result,
process specification, and actual exported bytes. Mutation tests reject changed
receipts and recomputed sidecars that differ from the parent output. Archived
inputs stay read-only. Recovery uses the original interrupted snapshots.

Run 206 exposed incorrect free-block and free-inode counts after journal-only
replay. The strict check rejected them even though e2fsck returned zero. Source
207 adds a separate pre-recovery observation. It records these defects and
rejects other diagnostics, repeated counters, malformed values and accepted
repairs. It grants no clean-filesystem pass. Default inspection and the signed
update cycle retain their strict checks. Both later recovery phases require
strict inspection without replay on the inspection copy.

The manual recovery boot from run 206 passed NAND data readback, erase, native
reboot and strict filesystem inspection. Fresh-work verification, actual rate
grants and both Core acceptance/restart checks passed. The new complete
automated cycle 207 is still running. Its outcome is not assumed here.

Source 207 also replaces the fixed 300 dashboard readiness attempts with a
bound from the selected process wall budget. The polling interval, three-second
HTTP timeout, complete HTTP 200 requirement and archived-body hash stay in
place. The host verifier checks the recorded policy against that process.
The actual native dashboard must still pass a new execution test.

All 532 Rust tests in 96 binaries pass, with no failures or ignored tests.
Formatting, strict Clippy, release builds and guide links pass. The current
source matches the 377-file source 207 manifest. No C code was added here.

Limits remain explicit. SIGKILL is a software interruption after completed
native writes. It does not establish interrupted-write atomicity or physical
power-loss behavior. The SD model uses writethrough storage. NAND geometry,
physical board identity, calibration, the original-board gate, native remote
dashboard and full exact-target matrix remain incomplete.
