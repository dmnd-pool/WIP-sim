# Independent review of browser run 453

Run 453 failed. Firefox did not complete its first NewSession request. No browser
result was emitted. The native HTTP and TLS component checks passed, and the
exact native, pool and access bytes were retained before worker cleanup. This
review grants no mining, Bitcoin Core, exact-board or complete-dashboard pass.

The selected run is
`run-d7b13b55416780e0443c175955b8ec9dc008b888a58bcb8bedd330e6b5315cc4`.
Its complete locked-run hash is
`460917fcc32ef1bf7829075124499366e0ba409ff32db4c6f1c690906412b930`.
The [independent byte audit](native-browser-retention-independent-453.json)
and [284-input manifest](native-browser-retention-independent-453-inputs.json)
record the checked evidence. Original run files were not changed.

## Retention and execution

Both native result copies match the acknowledged result. All 251 extracted access
files, ten pool session files, and pool stdout/stderr bytes match their bundle
objects. Decoded native and outer process logs match the retained raw logs.

| Object | SHA-256 | Bytes |
| --- | --- | ---: |
| Native result | `c18c239c1a3417a3f7573d75a443dce30763a7d38cc5562007fcbb05f87190c7` | 105459647 |
| Pool bundle | `a86232130a35647f6abe6bc593ed22fd34940fd50e47232301ab24ecb66891f7` | 102679 |
| Access bundle | `b46e64375fc5df3760e2ca2482a54845d60b8567c2deb40b39ad03042b845ffb` | 11206012 |

The actual runtime stdin contains the exact joined receipt. It agrees with the
guest final receipt and both retained bundle receipts. Host and worker native
receipts agree. The worker cleanup record names this run and acknowledged native
result, and records removal of its run storage. This is artifact retention and
cleanup evidence, not a successful browser result or host-crash recovery test.

The native process exited zero after 305140 ms. The outer process exited zero
after 759518 ms. Neither trace was truncated. The final shared marker and outer
assessment both remain false. The native command and limits match prepared 452:
10 H/s, 360-second wall limit, 330-second CPU limit and 3 GiB address space. The
native observation and pool budgets were not increased for this browser run.

## Selected HTTP and TLS path

The actual client and server have different network, mount, PID, IPC and UTS
namespaces. The client has only loopback and its private veth interface, with no
default route or route to the miner network. The worker runs outside the support
VM on the same execution host. No external physical-host deployment is claimed.

The selected tunnel uses `http://127.0.0.1:18090`. The selected TLS URL is
`https://192.0.2.1:9443/` inside the support VM. The publication, provider lease,
lock, process, policy, carrier record and tunnel destination name the same run.
The native destination is `10.0.2.15:80`. All 15 TLS response headers identify
that selected run.

The review checked all 15 HTTP and 15 TLS raw bodies, headers, status files and
request objects against the retained reports. It independently checked these
response values:

- A client without its certificate received HTTP 400 and curl exit 22.
- Missing and invalid guest authentication returned AuthError. Login, session
  extension and logout returned VoidResult. The expired session was rejected.
- HTTP and TLS issued different native session values in fresh cookie stores.
- Both fetched the original 425409-byte application asset with SHA-256
  `1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423`.
- Uptime advanced from 85 to 90 seconds over HTTP and 132 to 138 over TLS.
  Both named the same boot start, host and native firmware version.
- Both changed the theme from null to DARK, read it back, and restored null.

TLS readiness recorded nine actual connection refusals, followed by success at
6105 ms. It finished at 6126 ms within the unchanged 30-second startup budget.
These TCP observations precede the separate authenticated TLS exchanges.

The gateway lifecycle has an unresolved client acknowledgment failure. The
client records `gateway_stopped=false` and `carrier gateway did not acknowledge
listener shutdown`. The server records its owned nginx PID 94 stopped and names
the same selected run in its gateway stop receipt. The tunnel stop succeeded.
The carrier service later recorded normal worker termination. These server
receipts do not convert the missing client acknowledgment into a passing check.

## Browser failure and remaining work

The browser process emitted zero result reports. Its stderr records
`Firefox NewSession did not complete`, caused by `browser command deadline`.
The [reconstructed startup diagnostics](browser-start-failure-453/provenance.json)
retain hash-checked Firefox logs and the complete failure snapshot. Profile 0
reached the request and waited for its initial application window. No native
browser UI, WebSocket, second-profile or persistence pass is established.

Mining and Core need separate audits of these exact inputs. The completed pool
process and native exit do not establish independently verified Bitcoin work.
The overall run remains failed. Full-target and original-board gates remain open.
