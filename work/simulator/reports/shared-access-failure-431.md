# Native SSH access passes; TLS startup fails in run 431

The archived S9 native service passed its HTTP checks through SSH. The complete
access run failed at TLS listener readiness. This is an adapted reference run,
not an exact-board or milestone pass. Browser testing was not requested.

The separate client used the actual selected run
`run-751e79ad4f6e547397b5fd2dcdf2e9d6cbf3f325c2135c1dbbbe1efc4ef5d288`.
Its lock hash is
`71df5df60c9d499bb01ff98c66e1a36f9ab0cf7d979a87abb9ca1e6921ddf765`.
The support VM contained separate client and server network namespaces.
The native worker ran beside that VM on the same physical host. No external
physical host or host-browser URL was tested.

The [native HTTP report](../network-vm/native-access-431/execution/shared-access/client/http/report.json)
records 15 successful requests. These check login, original assets, API data,
configuration change/readback/restoration, logout, and rejection of the expired
session. The app asset is 425,409 bytes, SHA-256
`1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423`.
The client used a fresh cookie store. It changed the native web theme to DARK,
read it back, and restored the original null value. This is a one-boot API
test. It does not establish browser, WebSocket, or restart persistence behavior.

The [client access report](../network-vm/native-access-431/execution/shared-access/client/report.json)
then failed with `selected carrier TLS listener not reachable`. It accepted
the exact selected publication before the failed TCP connection. No TLS
request or authentication rejection test ran. The startup log is empty, and
no nginx access or error log was created before cleanup. The client made one
connection attempt and did not retain its operating-system error. A startup
race is a hypothesis; these logs alone do not prove its cause.

The native process ran for 305,154 ms and exited with code 0 without truncated
traces. The independent pool completed ten sessions in 300,418 ms. The outer
process took 834,098 ms and exited with code 0, but the guest suite and
[assessment](../network-vm/native-access-431/assessment.json) failed. New mining
and Core acceptance results are not granted for this failed access run.

The host retained native, pool, and access bytes before the joined cleanup
acknowledgment. The access bundle contains 181 files. The worker cleanup
names the same selected run and native result. The
[independent retention review](../reviews/failed-access-retention-independent-431.md)
rehashed both native copies, the two bundles, all 181 access files and ten pool
sessions. It also checked the actual stdin acknowledgment and final cleanup
record. Retention passes; the access result remains failed.

| Retained object | SHA-256 | Bytes |
| --- | --- | ---: |
| Native result | `e3aac175299649271bd985c54fd173541864d134c0c5f6bed46bf94776403cb1` | 105718948 |
| Pool bundle | `11be68bd9a2838456bab25fda1ccb945553d8bc3d9155b5ee9e9751bb490f52b` | 98537 |
| Access bundle | `d8d56b446cbc8a9b368dce25237d9570dc716ed86f6f1773243dc88040e47b9b` | 5516831 |

The [frozen executable hashes](../channel-target-430/SHA256SUMS),
[474-file source checkpoint](../core-checkpoint-430/manifest.json),
[prepared process](../qemu-s9/native-access-prepared-430/process-spec.json),
[actual transferred inputs](../network-vm/native-access-431/ssh-inputs.json),
and [joined receipt](../network-vm/native-access-431/execution/shared-pool/retention-receipt.json)
remain available. Original downloaded firmware was not changed.

Next: retain each readiness error, wait within the existing absolute startup
deadline while checking the owned process, and test delayed real nginx startup
and permanent refusal in an isolated VM. Review that fix before a fresh full
firmware access run. Keep the native boot, pool, and hashing budgets unchanged.
