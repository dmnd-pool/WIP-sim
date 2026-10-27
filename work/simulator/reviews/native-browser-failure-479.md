# Native browser run 479

Status: failed. This run does not pass browser or complete-target acceptance.
The [input freeze 478](native-browser-freeze-478.json) and
[checked evidence identities](native-browser-failure-479.json) are retained.
The selected run is
`run-2360e47fa0d091b32eb802104a8f52b77de94e1359e05c117937ea7d267218fc`.

Both real Firefox sessions started before firmware dispatch. NewSession took
55,147ms and 57,683ms. Preparation took 116,243ms. Both profiles had empty
cookies and an empty page. Preparation finished at VM monotonic time 168.872s;
dispatch followed at 168.927s and native execution at 294.390s. The pool then
completed its unchanged 300-second interval.

The first HTTP browser profile passed its real native checks in 140.145s.
It loaded the archived assets, logged in through the native form, received
two live WebSocket samples, checked advancing HTTP uptime, and read the
rendered board and pool tables. Those tables showed three boards, 63 chips
per board, 100MHz, and one private-pool row with accepted work. The test did
not fill or replace those values. The earlier GraphQL timeout from run 469
did not recur after the second profile's navigation moved after these checks.

The second profile still had empty cookies. Its navigation began after the
primary checks. WebDriver command 7 returned its unchanged 90-second navigation
timeout after 90.866s in the browser log. The fresh-profile rejection check
did not run. The primary session check after isolation did not run. The TLS
browser origin did not run. The whole browser suite therefore failed.

The bridge ended after 305.357s. It retained 104 connection attempts: 44
failures before readiness, 58 complete streams and two streams cancelled at
shutdown. These timings indicate that the second navigation crossed the end
of the native observation window. This is an inference from the browser and
VM records; those records do not provide a simultaneous clock sample.
The logs do not identify all causes of its slow
page load. No browser memory or task limit was hit. Both owned browser groups
were empty after cleanup. No deadline was increased.

The native API checks passed 15 requests through SSH and 15 through the
authenticated TLS gateway. They included login, original asset hashes,
telemetry and configuration write/readback/restoration. The SSH tunnel stopped.
The server retained a successful gateway stop record, but the separate client
did not receive the required acknowledgment. That check remains failed.

The native and browser wall clocks still differed. This frozen cohort did not
declare a clock adaptation. A separate change now binds the browser clock
check to the selected process and requires the same five-second request
interval tolerance in live and retained verification. It has no retrospective
effect on run 479. Support-VM hardware acceleration is under investigation to
reduce browser scheduling cost while preserving the ARM firmware budgets.

All 270 retained access files match the bundle bytes, sizes and SHA-256 hashes.
The joined native, pool and access receipt identities also match. The four
complete Firefox logs were decoded and checked against their recorded hashes.
These checks do not independently audit mining or Bitcoin Core acceptance.
The command exited 1 after evidence retention. All owned VMs stopped.
