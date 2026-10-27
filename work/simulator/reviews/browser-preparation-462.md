# Selected browser preparation before firmware dispatch

Source changes462 move the two fresh Firefox sessions into support preparation.
The selected native job is dispatched only after both sessions have completed
NewSession, passed empty-cookie and about:blank checks, and remain in their
owned browser cgroups. The parent checks the exact helper hash, distinct
session and process identities, UID1001, unchanged preferences and selected
mode. An incomplete readiness line, a duplicate line or a record for another
helper cannot release dispatch.

Preparation has a650-second overall deadline. Each profile keeps its existing
300-second startup budget. Cancellation is checked before polling and again
when a readiness record arrives. A browser failure returns before dispatch;
the owning client destructor stops the helper and its two cgroups. A shared
cancellation token is passed to preparation and the existing worker executor.
The support server and outer run still have1200-second limits. Native firmware
360-second wall,330-second CPU and305-second observation limits are unchanged.
The independent pool starts only after the subsequent ExecuteToken lease;
browser preparation does not consume the pool's300-second work interval.

The browser helper already waited for a selected publication after startup.
The changed ordering uses that interface. Its readiness record now includes
both actual NewSession results and both fresh-profile states. The receipt and
client dispatch record include the same support-VM monotonic clock. These
records are retained in the existing selected access bundle. They do not
claim that firmware or its dashboard has become ready.

Browser support now uses a256MiB QEMU translation cache. Its two virtual CPUs,
4GiB guest RAM and6GiB outer address bound are unchanged. Basic non-browser
support is unchanged. [Comparison458b](browser-startup-resume-458.md) supports
this choice; it does not establish the sole cause of the earlier453 failure.

Focused checks462c passed:104 namespace tests,10 browser-helper tests and64
worker-channel tests. Eight namespace and three worker-channel VM helpers
remain explicitly ignored in ordinary tests. Three new ordinary checks cover
partial/duplicate readiness, wrong identity/reused sessions/dirty profiles,
and cancellation/deadline/owned-process failure before dispatch. StrictClippy
and all three release builds passed. The initial462 test compile failed on a
JSON fixture expression; that failure log is retained. It was fixed without
changing any assertion. Logs are under `checks-2026-10-21-001` with prefix
`browser-preparation462c-`.

No new complete native browser run has passed yet. Actual preparation ordering,
selected HTTP/TLS browser operations, cleanup and gateway shutdown still need
fresh run463. Earlier453 remains failed. Current changes do not close exact
hardware, physical timing or full simulator gates.
