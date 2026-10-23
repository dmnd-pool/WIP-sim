# Native update telemetry review

This is an author review. The independent review gate remains open.

Cycle256 failed before the update commit. Its LuCI probe requested uptime but
omitted hostname from its first GraphQL query. The current shared telemetry
validator requires both fields. Retained responses6,7,17,19 and21 have the
same native hostname and boot epoch. Their uptimes advance from63 to121 seconds.
The first LuCI query is a different response and has no hostname field.
The query defect is in the test client. It is not a firmware identity change.

Source258 requests hostname in that query. It retains the same identity,
boot-epoch and advancing-time assertions. Cycle258 tests that change with
original firmware and backend256. Its results must stand on their own.

Further review found that the pre-commit uptime check was outside the branch
that restarts mining after failed validation. Source259 validates the first
sample before any service change. It also performs the last identity check
inside the validation operation. A failure before commit now reaches the
existing native restart path. Once the commit request starts, the probe still
leaves shutdown and restart to the native updater. It does not race the update
with a miner-start command.

Source257 also records bounded browser failure diagnostics in live remote
progress. It selects only phase, elapsed time, error and process exit status.
Raw browser pages, cookies and encoded traces remain in retained evidence.
That regression passes and does not change dashboard acceptance criteria.

The failed cycle256 and all earlier remote failures remain failed. No exact
physical target or original-board gate is closed by these test-client fixes.
