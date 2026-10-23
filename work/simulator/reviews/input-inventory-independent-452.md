# Independent input inventory review, cycle 452

The inventory fix and its focused checks pass. No unresolved finding remains in this change. The [exact source and execution manifest](input-inventory-independent-452.json) pins the two changed source files, checks, captured test ELF, libraries and real child result.

The entry limit is selected once for the complete inventory. Every root and recursive call uses it. The separate per-path browser flag still controls symlink acceptance and the 256/512 MiB file limits. Strict shared inputs retain their original policy. Revisited paths and empty directories consume no extra entries; a new record at the limit fails before insertion. Aggregate byte limits and native execution budgets are unchanged.

The ordinary example suite passed 101 tests with zero failures and six explicit ignores. A separate isolated regression passed one outer test and one actual child test, with zero ignored tests in either run. The child exited zero in 180 ms without truncated output. It used the existing isolated runtime with a 30-second wall limit, 20 CPU seconds, 256 MiB address-space limit and 64 MiB scratch limit. The oversized negative creates only a sparse file length. All four distinct captured input objects match their expected hashes.

The real-file tests cover a browser inventory with 1,200 entries followed by a strict shared input; both exact entry limits; duplicate and empty-directory cases; rejection of an extra entry without mutation; strict symlink and sparse-size rejection; and strict revalidation of a previously allowed browser link. The actual child also checks that the host sentinel is absent. Its raw stdout matches the retained ProcessResult and contains the required one-test pass evidence.

The [failed run 451 evidence](browser-inventory-failure-independent-451.json) remains failed. Its combined image was correctly selected and the support VM booted. Initial inventory reconstruction gives 4,619 regular entries and 1,105 links. The first strict shared call then incorrectly applied the 1,024 limit. Firmware, pool and browser orchestration had not started. The outer process exit code of zero did not produce a passing assessment. No native result or pool/access bundle exists for that run.

These checks prove the input-capture fix. A fresh complete native browser run is still required. They do not grant a firmware, browser, exact-board or full simulator completion result.
