# Independent browser evidence addendum 419

Reviewer: `channel_review`. Authors: the parent and `private_channel`.

This addendum updates the browser part of
[review 414](shared-carrier-access-independent-414.md). No unresolved source
finding remains in this scope. It does not claim that the new browser path has
executed against firmware. The frozen integration run remains required.

The parent now captures the requested browser mode and the payload and driver
hashes before the preparation helper runs. The support proof compares the
prepared scenario and its unique captured root entries to those original
values. It does not replace the requested mode with the helper's resulting
mode. The regression rejects a removed mode flag, substituted input bytes,
missing input, and unrequested browser capture.

The live client and retained host assessment now call the same browser report
verifier. It checks the selected lock, policy, provider, publication, both exact
origins, isolated fresh profiles, cleanup, WebSocket observations, board and pool
views, and live mining sample count. A passed flag alone does not satisfy it.
The regression removes or changes each relevant observation and rejects a
missing origin.

The host also joins the report to the exact retained process record and raw
stdout and stderr identities. The raw stdout must contain exactly one matching
result marker. The selected policy and provider bytes must match the imported
metadata. A final review finding identified contradictory process outcomes:
the host now requires exit code zero, no process error, and one reported result.
This matches the existing live process-success check.

The final affected test log is `carrier-browser-retention418-tests.log`. It
records 90 namespace-helper tests, ten browser tests, and 58 probe tests passed.
Four descriptor helper entries are ignored in this invocation; this is not a
claim that those entries ran here. The new browser request and retained-report
regressions ran in the applicable binaries. Strict affected-example lint passes
in `carrier-browser-retention419-clippy.log`. The adjacent manifest pins the
reviewed source and final check logs.

All six manifest entries still match after the parent's final workspace format
pass for source freeze 420. No source hash update was needed.

These fixes strengthen evidence selection and assessment. They do not establish
an external-host result, full VM loss recovery, an original-board path, or exact
physical-target coverage.
