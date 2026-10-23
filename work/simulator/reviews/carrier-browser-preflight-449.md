# Selected carrier browser preflight

This is a read-only source and evidence review for planned run451. No code,
build, test, VM or deletion was performed. Run445 did not execute the guest.
Run441 supplies HTTP/TLS measurements, not a browser pass. The repaired image
selector has a separate review and is outside this preflight.

No concrete blocking defect was found in the reviewed selection, credential,
resource or retention paths. Completion within the unchanged native lifetime
still requires an actual run. Input hashes are in
[the review manifest](carrier-browser-preflight-449-inputs.json).

## Selected service and browser origins

The observer waits for the actual worker-run record. It checks its token,
run ID and complete locked-run hash before starting the browser. The browser
receives a `CarrierSelection`, not a fabricated runtime dashboard binding.
That selection binds the exact policy bytes, provider configuration, worker
process, private endpoint and authenticated TLS publication. It requires the
observed provider to be active and the selected worker identity to agree.

The two tested origins are `http://127.0.0.1:18090` through the SSH tunnel
and `https://192.0.2.1:9443` through the private TLS gateway. The live gateway
continues its worker/provider lease checks during the browser test. The
browser rejects responses that leave the selected origin or path. Its TLS
GraphQL and asset responses must carry the actual selected-run header.
Neither URL is a host-browser or external-host deployment address.

The native UI must render three boards with 63 chips each and the selected
private pool. The browser must observe two distinct native WebSocket samples
through each origin. It also checks the archived application hash and live
telemetry. Positive firmware share counters are required but are not accepted
as an independent mining oracle. Mining and Core audits remain separate.

## Credentials and isolation

The captured445 scenario requests both `MD_SIM_BOARD_BROWSER=1` and
`MD_SIM_CARRIER_BROWSER=1`. It includes the selected browser helper and the
payload that contains Firefox, certutil and pk12util. It leaves
`MD_SIM_S9_BROWSER=0` in the native process: browsers run in the support client.

The client mounts only the CA certificate and client certificate/key. The
browser checks that neither server-key path is visible. Root copies the client
credentials into a new private directory before dropping to UID1001. Each
profile is new, mode0700, and owned by that UID. Its separate NSS certificate
database receives the CA and client identity. The import checks the client
certificate purpose and compares public certificate readback bytes. Firefox
must report `acceptInsecureCerts=false` and the exact selected profile.

The two Firefox processes have separate profiles and cookie databases. For
each origin, the second profile must have no cookies, show the native login
form and receive an authentication rejection after the first profile logs in.
The first profile must retain its own authenticated session. Thus different
ports are not used as proof of cookie isolation. The two origins are tested
in order with these two profiles. The TLS gateway's recorded
`upgrade-insecure-requests` response policy preserves the original asset bytes
while allowing the archived application's insecure WebSocket URL to use TLS.

The browser setup requires only `client0` and loopback in its network
namespace and no default route. It creates a private mount namespace, drops
privileges and enables no-new-privileges. Each owned browser cgroup has a
768MiB resident-memory limit, no swap, 256 tasks and a two-CPU quota. Cleanup
must empty both groups. The client has no direct route to the miner network.

## Lifetime and explicit resources

The browser-only support profile selects 4096MiB guest RAM, two virtual CPUs,
ramfs for the root, a 6GiB outer address-space limit, 1200 seconds wall time
and 2000 CPU seconds. These are explicit support limits. The native ProcessSpec
from445 still has 360 seconds wall time, 330 CPU seconds and 3GiB address
space. Its pool interval is 300 seconds with the existing five-second
observation margin. The browser does not restart or extend that process.

Browser preparation starts after worker admission, in parallel with HTTP
readiness and API checks. The two Firefox sessions start in order. Each has
an existing 300-second startup bound. Browser selection is published only
after the native HTTP and TLS API checks pass. The parent then has a separate
360-second browser wait. Those support limits do not promise that the native
service remains alive for their full duration. The final selected worker
check rejects an access result that finishes after the native process ends.

In441, HTTP requests used 6542ms in total and observed native uptime70 to74.
TLS requests used 10225ms and observed uptime88 to91. TLS listener readiness
completed at2744ms within its 30-second startup budget. These measurements
show the previous API path's cost. Native uptime is not an exact remaining
host-wall budget, and441 had no concurrent Firefox work. Older browser
measurements use another topology and do not establish a451 deadline pass.
No timeout, CPU or memory increase is recommended from this preflight.

## Retention and acceptance

The selected browser result is retained in the client report and joined access
bundle. Raw `board-browser.stdout`, `board-browser.stderr` and
`board-browser-process.json` are retained under `client-observer/`. The
browser process record must have exit0, no error and exactly one result.
The host joins those records to the original request's browser driver and
the exact lock, policy, provider, endpoint and publication. It reruns the
same full browser report verifier and compares the raw result marker.

The exporter has a512-file bound, an8MiB per-file bound, a15MiB raw aggregate
bound and a32MiB encoded bundle bound. Run441 retained260 files and7935256
raw bytes, leaving7793384 raw bytes before the same aggregate bound. Browser
reports add data and copies of selected metadata. Their actual total still
needs measurement. An over-limit export fails; it does not drop evidence or
become a browser pass. Browser profiles and private TLS keys are not exported.

After451, inspect both origin results, all WebSocket samples, both profile
cleanup records, actual helper/payload identities and group memory/task/CPU
evidence. Check joined retention and the final native status. Then run fresh
independent mining and Core checks on that run's own outputs. Until then,
the browser gate remains open.
