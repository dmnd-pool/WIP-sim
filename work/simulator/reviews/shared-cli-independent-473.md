# Independent shared S9 CLI review 473

The component review passes after three fixes. This is a source and component
review. The full firmware CLI workflow has not run. The manifest still pins
the development cohort 468, whose browser run 469 failed. That failure remains
explicit. No exact target, persistent dashboard, or full specification gate
passes here.

The review covered the CLI dispatch, all stage arguments, frozen input checks,
report bounds, process ownership, cancellation, failure retention, mining job
requirements, and the two Bitcoin Core candidate checks. The manifest contains
3,574 repository inputs and 43 host files. The source requires fixed relative
repository paths or fixed `/usr` host paths, and checks each hash and size.
It uses a new private output directory. It retains failed reports and returns
a failed aggregate when any required stage fails.

Three findings are closed:

- A failed helper could leave a child alive because its reaped leader disabled
  process-group cleanup. The supervisor now observes the leader with `waitid`
  and `WNOWAIT`. It kills its owned group before reaping the leader. This keeps
  the group ID from reuse during cleanup. The same cleanup runs after success,
  failure, cancellation, and timeout. A real shell that starts a sleeping child
  and exits with code 7 is the regression case.
- Stage startup recorded a new helper hash without checking it against the
  frozen manifest. Each stage now receives the expected object identity and
  checks it before launch. A changed identity fails before invocation output
  or process creation.
- The JSON reader checked path metadata, then read without a byte limit. It
  now opens with `O_NOFOLLOW | O_NONBLOCK`, checks the opened regular file,
  reads at most 16 MiB plus one byte, and rejects excess data. Regressions cover
  a symlink, a FIFO, and an oversized regular file.

The four component tests passed. They also check cancellation before launch,
timeout, direct-child reaping, and mismatched or failed Core results. Scoped
Clippy with warnings denied and the release build passed. The reviewer read
these test logs and checked the final source against the fixes. No unresolved
component correctness finding remains. The tests do not substitute for the
pending full firmware run. Source and log identities are in
[the input record](shared-cli-independent-473.json).

The Core verifier requires the selected firmware candidate hash, successful
submission, height 0 to 1, restart at height 1, clean node shutdown, and a zero
exit code. It rejects a node-generated block. The fresh-work audit must retain
at least two jobs, a clean-job transition, a difficulty change, and two distinct
candidate hashes. The final input recheck precedes a successful aggregate.
Any promotion to a repaired cohort requires new input pins and another actual
CLI execution.
