# External pool audit review384, closed386

The private-channel agent reviewed the separate pool evidence join and the
shared submission, rate, and regtest audit commands. This is a source review.
It does not establish a new shared firmware execution or Core acceptance.

The join keeps the original native worker result bytes. It checks the exact
process, run, token, lock, native and pool retention receipts, support inputs,
actual prepared mounts, raw pool logs, and raw session traces. Strict mode
checks pool completion, native completion, and carrier policy. Shared audit
commands are explicit. The existing default commands keep their own path.

The submission audit still compares the saved native logs to the process
result. It independently reconstructs headers, computes SHA-256d, replays pool
verdicts, and correlates submissions with actual ASIC work and serial replies.
The regtest audit prepares one independently checked candidate per fresh job.
It does not itself claim node acceptance.

Two strict-success findings were sent to the implementation agent:

1. `verify(true)` binds the outer result hash but does not require that result
   to have status `exited`, exit code 0, and an untruncated trace. The live
   parent assessment checks these conditions, but the standalone audit must
   also reject a failed outer execution.
2. The parent now validates and retains the provider receipt. The standalone
   strict join must check that receipt against the actual outer log and the
   selected process, configuration, support identity, and carrier policy.
   Otherwise the strict audit omits one link that the live assessment checks.

Both findings are resolved in checkpoint386. The join bounds and parses the
actual outer result. Strict mode now rejects failed and truncated outer
execution. The provider record must equal the single actual host record in
that result's stdout. Its configuration, selected process, support input,
worker identity, policy, and traffic must match. The worker identity must also
match the actual outer `/worker` mount. The provider configuration must match
its retained file and actual outer mount. Retention mode can still preserve
failed results without asserting success.

The new regression rejects cancelled, nonzero, missing-exit-code, and
truncated outer results. After the last mount check, the
[affected tests](../checks-2026-10-21-001/shared-evidence386-tests.log) pass:
50 audit tests and 48 worker probe tests. The ignored child entry points are
invoked by their descriptor tests. [Focused strict Clippy](../checks-2026-10-21-001/shared-evidence386-clippy.log)
passes. Broad check385 failed on concurrent helper and retention work; its
failure log remains unchanged.

No correctness finding remains in this source-review scope. These checks do
not establish a new shared firmware or pool run. The
[final source and evidence manifest](external-pool-independent-private-channel-386-inputs.json)
records the exact reviewed state. The initial384 manifest preserves the
earlier review state.
