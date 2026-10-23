# Component cleanup extension: source checkpoint

Follow-up: focused checks, independent review and authorized cleanup are
complete. See `component-cleanup-444.md` and
`component-cleanup-444/postcheck.json`. The text below preserves the earlier
source-only checkpoint while native441 was running.

This change is separate from frozen440 and native441. Native441 was still
running when this checkpoint was written. No build, test, virtual machine or
cleanup command was run for this change. Only the source formatter ran.

The only changed source is
`crates/md-sim-firmware/examples/reclaim_qemu_inputs.rs`.
Its SHA-256 is
`fce6f9ad044d23a8754e4ea526d9e2b37b7cbe2204a9aa72ab072ffb002d7938`.
Its size is 52,322 bytes. Independent source review found no blocking
finding. The requested real CLI regression is now written; its source
review and execution remain pending.

The new scope is exactly three-digit `byte-stream-NNN`,
`worker-timeout-NNN` and `tls-readiness-NNN` run directories directly below
`work/simulator/network-vm`, with the leaf `input-store`. Nested stores,
other prefixes, archive paths and path aliases do not qualify.

Each run must have a bounded regular assessment, process result, process
specification and guest log. The reader opens each file once with
`O_NOFOLLOW | O_NONBLOCK`. It checks the opened file type, one link, size
bound and exact bytes. All three assessment ObjectIds must match. The
complete process result must have a known terminal status. Failed terminal
runs can qualify; active or incomplete runs cannot. Failed435 and437 have
no required identity fields, so they remain ineligible. Their assessments
have not been changed.

The command checks this evidence before Store acquisition, again while
holding the exclusive Store lock, and once more before the restore journal.
The journal records these exact evidence identities. The new modes consider
only duplicate CAS objects. They retain adjacent derived archives, all run
evidence and all unique objects. The canonical copy must match both hash and
size. Each deletion now also checks both copies against the recorded
journal identity. Equal but substituted copies cannot authorize deletion.
The initial and completed journal files and their directory are synced.

New tests cover exact path scope; failed and active results; missing identity
fields; substituted result, specification and log bytes; mismatched sizes;
partial terminal results; path aliases; symlink, hard-link and FIFO records;
bounded reads; active Store ownership; changed evidence after locking; and
two identically substituted copies. These tests are written but not run.

An explicit ignored test takes `MD_SIM_RECLAIM_EXECUTABLE`, which must name
the separately built example. It launches the actual command in a new
temporary repository, with a five-second limit for each command. It first
checks active Store lock refusal with no deletion. It then checks exact
duplicate deletion, preserved unique inputs and adjacent archives, all
unchanged evidence, the exact completed journal and restoration by copy.
The ignored marker declares the separate binary prerequisite; this test
must be run explicitly before any real cleanup is authorized.

After the parent confirms that native441 has stopped, run the focused
example test, strict example Clippy and format check. Investigate every
failure before any cleanup. Then review the actual test result and obtain
the parent's cleanup timing. Do not reclaim canonical342/375, prepared390,
430/431, frozen440, active441, original archives or unique captured helpers.

The earlier plan remains at
`work/simulator/reviews/tls-readiness-cleanup-plan-440/plan.json`.
Its proposed copies are still present. This checkpoint records zero
deletions and zero reclaimed bytes.
