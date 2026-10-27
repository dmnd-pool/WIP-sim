# Native browser attempt 492 failed

The browser reached the original native page before the miner returned its
first accepted solution. It logged in and received two real WebSocket samples.
Both samples reported three boards with 63 chips each and zero accepted work.
The strict positive-share assertion correctly failed. The clock and TLS browser
checks were not reached.

The second sample had timestamp 1793045848. The native API still reported zero
accepted work at 1793045857. At 1793045873, it reported one accepted solution.
The pool independently recorded a real submission and a true response. The
first positive native snapshot was 25 seconds after the failed second sample.

The fix moves the existing 60-second native DOM readiness check before the
unchanged 20-second WebSocket check. The DOM check already requires a real
accepted solution from the selected private pool. Both WebSocket samples must
still pass the same topology, sensor and positive-share assertions. The native
firmware, hashing cap, filter adaptation and all deadlines remain unchanged.

The transport repair retained this failure in full. The independent audit
checked all 274 access files, the exact 573-byte acknowledgement, the native,
pool and access hashes, and worker cleanup. HTTP and TLS API suites each passed
15 observations. Gateway and SSH tunnel stop acknowledgements passed. These
results do not change the failed browser or `complete_result_exported=false`
verdicts.

See the [exact identities and timing evidence](native-browser-failure-492.json).
The frozen source is in [checkpoint 491](../core-checkpoint-491/manifest.json).
