# Shared CLI reader review

Status: source and affected checks passed. Promotion to a new cohort and the
actual full CLI execution remain pending. The [review record](shared-cli-independent-488.json)
pins the source and check logs. No source finding remains open.

The native assessment from run 479 is 49,409,766 bytes because it contains
embedded artifacts. The generic 16MiB report reader correctly rejected that
size but could not read this valid native report. The new typed streaming
reader retains a 128MiB file and read-growth bound. It skips payload values
while requiring the verdict and identity fields. It rejects symlinks,
non-files, duplicate or missing verdicts, excess bytes and trailing JSON data.

The CLI checks the assessment's support specification and result hashes against
the actual files. It also checks that the native process matches its lock and
does not request host virtualization. Its result records both outer support
and native identities. It grants no cached result reuse.

Seven regression tests passed. The explicit test of the actual retained run
479 report passed and preserved its failed verdict. Strict Clippy passed.
The positive metadata fixture tests the reader and identity checks only. It
does not establish firmware execution. The full command still requires a new
passed native/browser cohort and fresh mining and Core evidence.
