# Completed HDL duplicate cleanup503

The frozen Rust utility removed333 verified duplicate input copies from failed builds502c,502d and502e. This recovered1,322,525,463 logical bytes. Build502b, the successful502f restore store, unique guards, logs, results, source freezes and original archives remain. The larger concurrent change in filesystem free space is not attributed to this cleanup.

The [source review](hdl-cleanup-independent-source-503.json) and [pair review](hdl-cleanup-independent-plan-503.json) closed before removal. The [actual journal](../cleanup-hdl-503/reclaim.json) exactly matches the reviewed plan and bound source evidence. The [postcheck](../cleanup-hdl-503/postcheck.json) confirms333 missing duplicates,148 unchanged retained files,111 restore files,41 bound evidence files and5 free store locks. Failed build verdicts remain failures.

For restoration, take the destination Store lock. Verify the retained `restore_copy_from` file against the journal SHA-256 and size. Create a new regular byte copy at the missing path and verify it again. Do not hard-link files or replace a different destination. The emptied source stores require restoration before preparation can use them again.

See the [complete record](hdl-cleanup-completed-503.json) for tested checks, helper identity and all retained records.
