# Independent shared carrier access review 414

Reviewer: `channel_review`. Authors: the parent and `private_channel`.

No unresolved source finding remains in this review scope. The adjacent input
manifest pins the files read. This is a source and component-test review. It is
not evidence that the new selected SSH, TLS, or browser path has run successfully
against the archived firmware. That integration run is still required.

The review covers the shared guest service, its separate client, optional native
browser selection, bounded access evidence, and the joined host retention path.
It does not replace the separate access gateway review or the full simulator
completion review. The reviewer's own runtime deadline and metadata helpers have
their own [independent review](carrier-input-timeout-independent-410.md).

The following findings were fixed before this checkpoint:

- The access snapshot counted files but not empty directories. Every directory
  entry now counts toward the bound. The 513-empty-directory regression passes.
- A failed access service could stop the control broker before native result
  retention. The supervisor now stops only its owned service and records failure.
  The native result and pool can still reach the retention barrier.
- A closed carrier while the worker was active could be called normal completion.
  Access now closes at once. A separate exact token and run lookup must prove
  worker completion within ten seconds. Actual peer loss is not a boot success.
- Individual socket reads could extend the lease refresh indefinitely. The
  service uses one absolute exchange deadline. The final worker-end check caps
  each exchange to the time left and checks the deadline before accepting proof.
- Resumed host assessment trusted extracted access files without rechecking them.
  It now verifies every retained file against the receipt-bound bundle first.
- Optional browser execution was not an explicit host assessment requirement.
  The bundle now binds the requested mode and captured driver identity. Import
  compares both to the actual outer support proof. A requested browser needs its
  selected-run, two-origin, profile-isolation, and cleanup evidence.

The service uses the selected full lock, policy, provider configuration, token,
process, and run. Provider observations come from the actual owning relay thread.
The service has its own mount and process namespaces. Its writable access root
does not expose pool scratch, client credentials, or server TLS keys. The parent
retains protected launch and outcome records. The client uses the selected worker
attempt, the real SSH tunnel, and mutually authenticated TLS from its separate
network namespace. It tests the existing native HTTP suite. A failure remains a
failure after its artifacts are retained.

The native result, independent pool bundle, and selected access bundle share one
identity-bound receipt. Host import checks the exact bytes and input identities,
syncs the imported files and directories, and only then acknowledges cleanup.
The final assessment is separate from retention. Browser cookies and profiles
are not exported as access artifacts.

The carrier browser selection is a separate strict schema. The old selected
browser schema remains unchanged. It verifies the full lock, policy, provider,
endpoint, publication, and live authorization observation. The existing native
browser assertions run for both HTTP and TLS with distinct fresh profiles. The
payload builder rejects parent, dot, empty, and control-character path components
and resolves support symlinks only within `/usr` or `/etc`.

Checked component evidence includes `carrier-service-browser411-tests.log`
(89 namespace-helper tests and nine browser tests passed; two descriptor helper
entries are ignored in that invocation), `carrier-service-browser409-clippy.log`,
and the parent snapshot regression record from cycle 409. These checks do not
establish real dashboard execution. The source manifest also includes the
latest fixes that require the parent's final frozen-state checks.

The topology remains a disconnected support VM with separate client and server
namespaces and a native worker outside that VM. It is not an external physical
host test. Full VM loss recovery, original-board execution, exact hardware
coverage, and the complete matrix remain open gates.
