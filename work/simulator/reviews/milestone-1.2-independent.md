# Independent milestone 1.2 handoff review

The stated milestone scope is supported by the retained evidence. This is a
checkpoint for the adapted S9 path in run441 and checks443. It is not completion
of milestone 2 or the full simulator. The frozen matrix still has 79,599 rows
and zero complete exact-target passes.

The [handoff](../milestone-1.2-handoff.md) keeps the exact frozen440 firmware,
hardware, helper and audit identities separate from current source checks. Its
claims agree with the earlier independent [441 access and retention review](native-access-retention-independent-441.md)
and [443 mining/Core audit](native-mining-core-links-independent-443.json):
15 HTTP and 15 TLS exchanges for one selected run, seven real submissions,
five accepted shares, two fresh jobs, and two independently checked blocks
accepted and retained after Core restart.

The worker and support VM used the same execution host. The test URLs are
private, stopped endpoints. The handoff makes no host-browser access,
external physical-host, original-board or complete-browser claim.

Open work is explicit. Run453 failed Firefox startup and the client gateway
shutdown acknowledgment. Test457b covers two real Firefox startups only.
Comparison458b has not run. Cleanup455 and recovery459 remain staged source,
unbuilt and untested. Recovery459 also has an open bounded-clone defect.
The full target matrix, physical timing, remaining exact models and original
boot gates remain incomplete. No prior failure has become a pass.

## Git exclusion check

The reviewer used Git's actual `check-ignore --no-index` in a temporary
repository. The project repository was not initialized or changed. The check
confirmed that root and nested target directories, raw runs, archive payloads,
local credentials and frozen helper binaries are excluded. It confirmed that
source, vendored Rust dependencies, Cargo.lock, `.cargo/config.toml`, catalogue
source records, acquisition manifests, compact reports, this handoff, the two
staged changes, frozen440 source/manifests and checkpoint logs remain eligible.

A metadata-only scan found the previously uncovered nested QEMU Rust target
directory. The new `target/` pattern excludes it. Large generated catalogue
databases, exports and the browser index are also excluded. No private-key
marker was found in the scoped source, documentation and catalogue scan outside
the excluded archive, work, vendor and build paths. This is a bounded review,
not a complete secret audit.

The allowed reports total about 3.52 MB. Root review files total about 1.52 MB.
Staged455 and459 total about 149 KB and 40 KB. Raw evidence and downloaded
payloads remain on disk. The handoff correctly states that Git does not back
them up and warns against removing ignored files. The user will create the
repository and commit. No agent commit is claimed.

## Final check boundary

Final current-source formatting, strict Clippy and workspace tests passed.
The reviewer independently counted 919 passed tests, zero failures and
38 ignored helpers across 109 suites in the retained test log. Ignored helpers
are not execution passes. The guide report records 547 checked links and no
broken links. The final Git check records 33 positive and negative cases.
The parent reported the combined check command exited zero.

The final [input manifest](milestone-1.2-independent-inputs.json) pins the
reviewed documents and check logs. No blocking scope or Git exclusion finding
remains for this defined checkpoint. The simulator itself remains incomplete.
The reviewer has no active command or VM. No source was changed by this review.
Further work stops after the handoff until the user asks to resume.
