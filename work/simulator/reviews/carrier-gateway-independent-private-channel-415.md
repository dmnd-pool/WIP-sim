# Independent carrier gateway review 415

No open correctness finding remains in the reviewed gateway component. This
review is read-only. It does not establish native dashboard operation, actual
remote TLS publication, or an external physical host test.

The new carrier API remains separate from the existing runtime dashboard
Binding. It verifies exact policy/provider/full-lock objects fetched through
bounded worker Get requests. The client transfers those objects through the
authenticated worker transport. The server does not assume access to the
outside worker's object-store path.

Endpoint inspection waits for the explicit atomic readiness record. It checks
its complete selected identity, provider configuration and endpoint-object
hash. Endpoint and socket ownership, modes, canonical paths and inode identity
are checked. Actual worker and provider queries use an absolute exchange
deadline. Waiting can delay readiness but never produces an authorization.
Publication checks the selected upstream, run, full execution identity,
browser policy, allowed client network and browser context. It retains the
explicit statement that listener creation is not native-service readiness.

Three findings were fixed before the final source review:

1. Direct SSH child output files had no resource ceiling. The caller now uses
   the coordinator's bounded owned-service spawn. Logs have a one-MiB ceiling;
   memory, CPU and descriptors are bounded. Parent loss kills the owned leader.
2. Connected startup watched stdin only after metadata transfer. It now starts
   the watcher first. Each bounded metadata exchange checks cancellation, and
   readiness/start checks cancellation before publication and after startup.
3. Cleanup could reap the SSH leader before killing its remaining group. It
   now reuses OwnedSshChild. Exit observation uses waitid with WNOWAIT. Cleanup
   stops the reserved private group before reaping, avoiding recycled-PID
   signals and surviving descendants.

The final tests415 pass 22 access tests. One existing VM-only byte transport
entry remains explicitly ignored by the host harness; its earlier real VM399
result is a separate component record. Strict focused lint and formatting pass.
The new tests include actual log-size enforcement and a real leader that exits
while its sleep descendant remains alive. Owned cleanup stops that descendant.
Metadata negatives cover readiness, cancellation before control access,
publication substitutions and false readiness claims. These tests do not
replace the upcoming real selected firmware/SSH/TLS run.

The adjacent input manifest pins the reviewed gateway, CLI, carrier, downloader
and coordinator guard source, plus final access test/lint/format logs. Default
gateway paths remain distinct. All three findings are resolved for this scope.
