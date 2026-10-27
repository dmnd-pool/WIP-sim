# Native browser run 469

Status: failed. This run does not pass browser or complete-target acceptance.
The [input freeze 468](native-browser-freeze-468.json) and
[checked evidence identities](native-browser-failure-469.json) are retained.
The selected run is
`run-dd9a2c143d7b21d21b989795732901bbf964cfc3478970247f0b2f8b804da1af`.

Both real Firefox sessions started before firmware dispatch. Their NewSession
times were 54,024ms and 54,686ms. Preparation took 111,994ms. Both profiles had
empty cookies and an empty page. The parent checked their live process owners,
groups and preferences. The same VM clock records preparation at 163.848s,
dispatch at 163.903s, pool request at 288.712s and execution at 289.186s.
The pool completed its unchanged 300-second interval. The fixed native work and
browser deadlines did not include browser startup.

The original native service passed 15 HTTP checks through SSH and 15 checks
through the authenticated TLS gateway. These include login, original asset
hashes, live API telemetry and configuration write/readback/restoration.
The gateway stop acknowledgment and tunnel stop passed. Both owned Firefox
groups were empty after cleanup. These passes do not replace the failed UI
test or establish restart persistence.

Firefox loaded the archived native assets, submitted the real login form and
rendered the dashboard. The first profile's authenticated session extension
passed. Its next command, sequence 22, fetched the subscription schema through
GraphQL. The second profile began navigation 244ms after that command started.
The command returned a 30-second script timeout after 62.957 seconds in the
browser's log clock. The WebSocket probe had not yet started. The first
dashboard's tables had not yet passed the live telemetry assertions.

The browser groups reported no memory or task limit hits. The bridge retained
102 connection attempts: 44 failures before service readiness and 58 completed
streams with no stream error. Its eight-connection limit queues new streams;
the absence of a rejection does not prove that no queue formed. The retained
logs do not fully separate queue delay from browser scheduling contention.
There is no raw HTTP packet capture for this cohort. The four complete Firefox
logs were decoded separately and checked against their recorded hashes.

The next fix keeps the first authenticated session open and completes its
telemetry checks before the second fresh profile navigates. The second profile
must still pass its real unauthenticated check. The first profile must still
pass a session extension afterward. No deadline or acceptance assertion is
removed. Bounded query-hash and phase records will identify future request,
header or body stalls without recording credentials.

All 271 retained access files match the bundle's bytes, sizes and SHA-256
identities. The joined native, pool and access receipt identities also match.
These checks do not independently audit mining or Bitcoin Core acceptance.
The command exited 1 after evidence retention. Its support VM has stopped.
