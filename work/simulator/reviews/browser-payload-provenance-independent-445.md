# Browser image preparation review, cycle 445

Run 445 failed before guest boot. The prepared image was the combined archive. The worker driver compared that identity with the base archive and rejected it. This is an image-selection defect. It is not a quota failure.

The independent [byte and identity checks](browser-payload-provenance-independent-445.json) pass. The combined image has SHA256 `0765830df3072c8f270be2b43baf470b9075b502cf163d7ba7927a71d370b365` and 499,952,513 bytes. It is exactly the base gzip member followed by the payload gzip member. It fits the existing 512 MiB limit for one Store object. The default aggregate Store limit is separate.

The original requested payload manifest equals the retained append manifest and the payload report identity. All 4,548 regular payload files occur with the same identities in the final root manifest. The payload records 1,105 symlinks separately. No substitution occurred in this failed run. These checks establish preparation only.

The reviewed append code validates path and symlink graphs, modes, exact source objects, base conflicts and output bounds. It preserves the base image and manifests. No additional blocking finding was found in that code.

Two fixes remain before a new execution. The driver, support proof and mining audit must select the same image from the explicit browser request. The proof must also bind the actual append manifest to the original requested payload identity. The current proof only checks the root mount of `/carrier-browser-payload.json`; that alone does not identify the manifest selected by `scenario.payload_manifest`. The copied payload manifest and payload report must match the request, and the combined report identity must match the selected prepared image. Reject payload selection when no browser was requested. Preserve no-fallback behavior.

A small regression should execute the new driver selection and Store import with a real prepared combined image. Put a unique marker in the second newc archive and read it through an independent archive tool or a small isolated boot. Test base selection, missing combined image, changed selected bytes and a substituted payload manifest. A test of append alone would miss the driver defect. The full native browser run remains required afterward.

This review made no source changes, builds or guest runs. Source identities are in the linked JSON. The selector changes and their tests were still pending at this checkpoint. Full simulator and exact-board completion remain open.
