# Independent staged T17 cleanup review, cycle 455

No unresolved source finding remains in this staged change. The [exact staging manifest and review](t17-cleanup-staged-independent-455.json) pin the source. The production utility is unchanged. No build, test, integration or deletion is claimed.

The new scope is limited to direct T17 packing/readback oracle input stores with three-digit checkpoint names. Canonical store 274 is required and cannot be a deletion target. The change retains complete terminal result bytes, exact stdout/stderr and tar relations, known member names and types, expanded GNU sparse bounds, strict trailer rules, source input identities and adjacent copies. Only duplicate content-store objects can enter the existing pinned restore journal. Locked admission and the final evidence recheck remain in place.

The initial omission rule would have accepted the old output-field format for any packing checkpoint. It now allows that format only for evidenced checkpoint 272. Tests add rejection for 274 and 447. The staged CLI regression also checks refusal to delete canonical 274, an active writer, changed evidence, preservation of failed evidence/unique inputs/archives, and exact copy restoration. These tests have not run yet.

The declared potential byte count is not a verified reclaim total. Build, ordinary tests, the explicit real CLI scratch regression and actual-store hash preflight must pass before any deletion. The reviewer read only staged source and small metadata during native browser run 453. No active run, archive, input store or source file was changed.
