# Completed Core duplicate cleanup503

The frozen Rust481 utility removed151 verified duplicate copies, totaling2,013,780,471 bytes. It kept every original source run, unique input, result, node verdict and frozen helper. No firmware or mining verdict changed.

The [independent review](core-cleanup-independent-plan-503.json) approved the exact plan. The [completed journal](../cleanup-core-503/reclaim.json) equals that plan. The [postcheck](../cleanup-core-503/postcheck.json) rehashed234 retained files and56 restore files, checked19 store locks, and confirmed the exact remaining object sets. The frozen shared CLI input check also exited0 after cleanup.

To restore a removed file, take its owning Core input-store lock. Read its path, `restore_copy_from`, SHA-256 and size from the journal. Verify the retained copy. Create a new regular byte copy at the missing path and verify it again. Do not hard-link files or replace a different destination. Keep the source run and canonical store listed in the journal.

The [complete record](core-cleanup-completed-503.json) pins commands, logs, inputs and postchecks.
