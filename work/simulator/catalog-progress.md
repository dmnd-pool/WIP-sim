# Catalogue implementation checkpoint

Date: 19 October 2026.

The simulator is incomplete. These checks do not prove firmware execution.

Implemented in `crates/md-sim-catalog`:

- Import and verify a frozen catalogue. Copy the exact export and evidence bytes. Freeze available profile, scenario, and backend records.
- Keep two identities. `catalogue_data_hash` excludes the registries. Exact targets can reference it without hashing their own reference. `snapshot_hash` includes all frozen files.
- Check selected archive bytes against the frozen size and SHA-256. Resolve workspace links only under the archive roots in the frozen storage record.
- Preserve all record fields and all candidate constraints. A firmware-family edge never makes an attached artifact runnable.
- Account for all entity and artifact records. Retain missing payloads and unresolved LFS pointers as blocked.
- Represent every Cartesian pair with explicit rows or an explicit default rule. Preserve both denominators.
- Resolve an explicit version 2 target. Check typed hardware fields, evidence IDs, component parents, byte verification, constraint review, backend builds, and model builds.
- Keep original, rebuilt, patched, and nonexecuted identities separate. Reject an API model that claims firmware execution.
- Hash complete resolved run inputs for resume decisions. Keep the suite, verifier, seeds, budgets, policies, caps, clocks, and input hashes in that identity.
- Preserve version 1 validation claims as historical data during migration. Do not promote them to version 2 gate passes.

CLI integration is in `crates/md-sim/src/catalog_commands.rs`.
It implements catalogue import, selected artifact verification, exact-target listing and explanation, complete matrix planning, retention of unresolved run outcomes, and an offline HTML report.
The matrix command does not execute unresolved targets. It reports zero executed targets.

Commands run:

```text
cargo test -p md-sim-catalog --offline
cargo fmt -p md-sim-catalog
cargo clippy -p md-sim-catalog --all-targets --offline -- -D warnings
cargo run -p md-sim-catalog --example catalog_audit --release --offline -- . work/simulator/catalog-audit
```

Results:

- Fifteen catalogue tests passed.
- Formatting and lint checks passed.
- The real catalogue snapshot contains 4,354 entities and 13,506 artifacts.
- Both plans contain 79,599 explicit rows. This includes 61,739 candidate rows.
- Relevant plan: 1,377 blocked, 71,437 unknown, and 6,785 not-applicable rows.
- Cartesian plan: 2,798,806 additional pairs use the explicit unknown rule.
- Executed tests: zero. Passing firmware targets: zero.
- The complete-coverage check fails. This failure is required while applicable targets remain unresolved.

The final audit snapshot is `catalog-audit/snapshots/9ce76a88bb3287b15d508c14856111f17d418e9ad72e6b2d27fee262c8ba0dcb`.
Plans and summary files are in `catalog-audit/`.
These are metadata snapshots. The import did not read the full 150 GiB payload archive.

Independent review found weak hardware field types, unchecked evidence IDs, duplicate component IDs, and parent directory links. The implementation now rejects these cases. Regression tests cover the identity findings.
The review also found that rebuilt source could declare no execution entry or no adaptation record. Both cases now fail validation.
The root agent found a circular snapshot reference. The separate catalogue data identity resolves it. A regression test proves that adding a target changes the full snapshot hash and preserves its catalogue data hash.
The real import found a path ordering defect. Snapshot entries now use the same string order for creation and verification.

Unresolved work:

- The catalogue does not contain complete executable profiles for the requested matrix.
- Most device revisions, wiring, boot dependencies, and implementation models remain unresolved.
- The CLI has no verified backend registry to make these exact targets runnable.
- Complete firmware preparation must verify every derived component and baseline before execution.
- The matrix runner still needs an execution scheduler and target gate results. Classification is not an execution result.
- Native dashboard, original-board boot, mining, remote transport, and regtest gates require separate evidence.

Next action: complete a real exact target and its verified component closure. Register its implemented backend and device models. Execute its gates. Then expand the same path without removing the unresolved matrix rows.
