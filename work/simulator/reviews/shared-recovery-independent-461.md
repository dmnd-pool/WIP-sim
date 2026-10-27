# Independent shared-evidence recovery review461

The reviewed source adds a separate output path for retained run453. It does
not change the existing shared mining verifier or the Bitcoin Core helper.
No blocking source finding remains in the two new files at this checkpoint.
Actual retention, strict mining replay and Core acceptance still need their
own results. The reviewer did not run those operations.

The recovery plan has explicit per-file, aggregate and entry limits. It uses
a new owner-only output directory outside the source tree. It rejects links
in the selected relative input paths. It copies only the declared proof graph;
archive payloads and input-store trees are not admitted. A metadata plan is
labelled as a plan, without provenance or acceptance claims.

The previously identified whole-file clone defect is fixed. The copy requests
FICLONERANGE for the exact nonzero expected size. An empty extent never requests
a whole-file clone. The fallback streams at most that extent, checks its exact
length and checks for a source tail. Both source and destination hashes must
match the expected identity. The source is opened read-only, output uses
create_new and output inodes are separate. Partial failed copies remain in the
new root with a failure record. No source deletion occurs.

The unchanged external verifier checks the source before copy. Its result must
match both the copied graph and a repeated source check. Every planned input
is hashed again before publication. The recovered pool-result uses the same
verifier output. Existing strict submission audit still replays the successful
execution constraints and compares the full report; its established
outer-result normalization is unchanged.

The failed access verdict is checked against the acknowledged bundle, receipt,
native identity and exact outer process/result hashes. Both original failed
flags are required. Copies retain the failed assessment and client report.
The new recovery report explicitly says that this is not another execution,
that the access gate remains failed, and that mining/Core acceptance have not
run. Tool and source hashes, original inputs and copy methods are recorded.

The source tests cover distinct inodes, changed identities, bounds, links,
concurrent source growth, empty extents and failed-verdict binding. The parent
reports37 ordinary tests, the explicit actual453 plan test, strict Clippy and
release build passed. This review independently checked the source and log
summaries; it does not substitute for actual recovered mining evidence.

The source and inspected log identities are in the adjacent input manifest.
Full simulator, original-board and milestone2 gates remain open.
