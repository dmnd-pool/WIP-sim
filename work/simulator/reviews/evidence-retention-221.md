# Retained execution observations

This is an author review. Independent review remains unavailable because
the review agents reached the service usage limit. No review gate is closed.

The native dashboard gate for local208 failed. The inner firmware process
completed, but the strict outer report rejects the failed prerequisite.
The failure extractor retains original process bytes and input metadata.
It does not generate the pool reports needed by the independent mining audit.

The new Rust helper reads those retained files. It validates the locked run,
the process specification, four pinned input objects, the selected execution
identity, the isolation tool hashes, the exported process hash, and both
execution streams. It then copies these objects to a new directory. It uses
the existing report generator and checks every original and copied hash again.
The retention manifest pins the helper, inputs, and selected run. A generator
error stays an error. The helper grants no firmware, dashboard or mining pass.

Actual negative CLI tests reject changed pool input, selected execution
identity, and guest bytes before the output directory is created. Two actual
successful invocations preserve all original objects. One uses the frozen
helper executable. Full Rust formatting, strict Clippy, and553 tests across98
test binaries pass. The helper uses no network or guest execution interface.

The unchanged208 mining audit verifies both completed sessions. Four separate
Core instances accept the resulting blocks and retain them after restart.
The aggregate mining report explicitly preserves dashboard_passed=false.
This is useful evidence from an incomplete run, not reuse as a complete target
or remote dashboard result. The full target matrix remains incomplete.
