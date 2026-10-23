# Independent component cleanup source review 441

No blocking source finding remains. Build, tests and actual cleanup are still
pending. This is a read-only source verdict for the exact input below. It does
not authorize a claim that the new command path has run.

The extension admits only direct `input-store` paths under
`work/simulator/network-vm/{byte-stream,worker-timeout,tls-readiness}-NNN`, where
NNN is exactly three digits. It rejects aliases and directory symlinks at every
component from the canonical repository boundary. The active native-access-441
path and protected channel, firmware and archive paths are outside this new scope.

Admission requires bounded regular, no-follow, nonblocking, single-link reads of
assessment, result, process specification and guest log. All three retained file
identities must match the assessment, and the full ProcessResult must have a
recognized terminal status. A failed execution may qualify for duplicate
reclamation; it does not become a passing test. Failed 435/437 lack these complete
assessment identities and remain ineligible.

The tool holds exclusive Store locks for the retained and reclaimed stores. Store
inventory rejects nonregular object entries and external hard links. Component
evidence is read again after locking and before journal publication. Deletion is
limited to duplicate published CAS objects. A CAS is the content-addressed store.
Incoming objects, unique objects, adjacent overlay/keycode archives, binaries and
run evidence remain. Every deletion rechecks both exact copies against the
journaled identity. The journal and completion record are synchronized to disk
with their parent directory.

The new real CLI regression uses a fresh scratch repository and separate bounded
child commands. It checks active-store rejection without a journal or deletion,
one duplicate removal after a terminal failure, preservation of unique inputs,
adjacent archives and all evidence, exact tool/component/journal identities, and
byte-for-byte restoration by copy. It is deliberately ignored by ordinary test
runs. It must be run explicitly with `--exact --ignored` and
`MD_SIM_RECLAIM_EXECUTABLE` set to the separately built utility. A recorded
one-test pass is required before using the extension on retained stores.

Reviewed source SHA-256:
`fce6f9ad044d23a8754e4ea526d9e2b37b7cbe2204a9aa72ab072ffb002d7938`
(52,322 bytes). No source, build, test, VM or stored input was changed by this
review. The parent kept the active native 441 run separate.

## Check 444 addendum

The source hash above is unchanged. The author then built the standalone utility.
The ordinary tests passed nine tests, with the CLI entry ignored in that run.
A separate explicit CLI run passed one test with zero ignored tests. Its output
confirms the actual active-lock rejection, exact duplicate removal/restoration,
and preservation checks. Strict focused Clippy and formatting passed. The
adjacent manifest now pins those five logs, the actual standalone executable and
test executable. No review finding remains before the parent's authorized,
whitelisted duplicate cleanup. The reviewer did not delete any retained input.
