# Independent browser selector review, cycle 450

No unresolved source or focused-test finding remains in this change. The final workspace checks and the explicit test against retained run 445 pass. The [exact source manifest](browser-selector-independent-450-inputs.json) pins all ten reviewed source files.

The launcher captures the original browser payload identity before helper preparation. It verifies the selected source path then. The shared selector uses retained bytes afterward, so a retrieved-result audit does not depend on the source host path. The copied payload manifest, append report, prepared image, kernel and root manifest must agree. The captured root mounts also require the exact original browser driver and payload identities. Missing, changed, duplicated, unexpected or downgraded browser inputs fail.

The driver, support proof and independent mining audit use that same selector. The driver imports the archive with the selected expected ObjectId and mounts that same object. There is no silent fallback to the base image. The five additional consumers only add a root module declaration. Each binary now includes one selector implementation. The audit retains the selector source identity.

The tests use real small gzip/newc archives, the actual content store and installed cpio to read the marker in the second archive. They check that base mode does not include it. They also test changed image/kernel/root identities, substituted payload metadata, mismatched flags, duplicate mounts and removal of the original source path. The fixture metadata is specific to selector tests; it is not a complete production payload manifest. A separate explicit test reads the actual retained run 445 preparation and binds its kernel to the captured support kernel. Neither test is a guest boot.

The earlier wiring error placed the original identity capture in the guest function. It is corrected in the launcher. The duplicated nested selector module is also removed. Preliminary 449 tests passed 18 executions before that module cleanup. All ten pinned source identities still match after the final checks.

Run 445 remains a failed pre-boot attempt. Its [separate byte review](browser-payload-provenance-independent-445.md) found no actual payload substitution or quota overflow. A fresh native browser run is still required. No exact-board or full simulator completion follows from this component review.

## Final checks

Workspace 450 passed formatting, strict Clippy, 908 test executions in 108 suites, and 545 guide links. The suite had zero failures and 33 explicitly ignored tests. The separate retained-run test then ran one test with zero failures and zero ignores. It selected the exact 499,952,513-byte combined image from run 445 and rejected base-mode selection.

The first explicit invocation failed because its relative environment path was resolved from the Cargo crate directory. That failure log is preserved. The absolute-path invocation passed with unchanged source and assertions. Both logs and the final checks are pinned in the input manifest. These checks grant no guest boot or browser pass.
