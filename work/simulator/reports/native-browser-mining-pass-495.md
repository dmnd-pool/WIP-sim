# Selected native browser and mining run495

The adapted S9 run passes native browser access, fresh mining, independent
header verification, and two Bitcoin Core acceptance and restart checks.
It does not establish an exact physical target or an original-board boot.

The selected run is
`run-2d806d4f216edd3a517e68f5c4de9df129aef9a63219b994fa14a0bd166d9b56`.
It uses the original archived Braiins S9 image
`2022-04-29-0-5eb2c5fe-22.02.4-plus`, three chains of63 BM1387 chips,
and a10H/s aggregate cap. The
[input freeze494](../reviews/native-browser-freeze-494.json) and
[independent input review](../reviews/native-freeze-independent-494.json)
pin the source, firmware, device, helper, policy and pool identities.
The run retains its complete
[adaptation manifest](../network-vm/native-browser-495/execution/adaptation.json).
The archive is unchanged. Adaptations include the reference devices, explicit
ASIC filter, monitor script offsets and execution-host wall clock.

The [native browser review](../reviews/native-browser-pass-495.json) checks
278 retained access files, four complete Firefox logs, the selected run and
its exact573-byte retention receipt. Both origins pass15 API checks, actual
native assets, login, live tables and WebSocket telemetry. Two separate cookie
stores establish session isolation. Native timestamps stay within the fixed
5,000ms client bound. Gateway and tunnel shutdown acknowledgments pass.

The worker and KVM client VM use one execution host. SSH and TLS use separate
client and server network namespaces. The native ARM firmware uses its own
isolated emulator process. This is not an external physical-host test.
The tested origins were `http://127.0.0.1:18090` through SSH and
`https://192.0.2.1:9443` through the authenticated gateway, inside the isolated
client. Both endpoints stopped with the test. They are not current host URLs.

The [rate audit](../network-vm/native-browser-495/execution/rate-audit495.json)
checks1,664 computations across all three chains. Its smallest grant interval
is101,804,220ns, above the100,000,000ns cap interval. The
[submission audit](../network-vm/native-browser-495/execution/regtest-audit495.json)
joins14 real firmware submissions to computed work and complete CRC-checked
hardware replies. Seven shares pass; seven fail for low difficulty. Two fresh
jobs, a clean-job transition and a difficulty change are verified.
The [independent raw-message review](../reviews/native-mining-independent-495.json)
reconstructs all14 Bitcoin headers and checks SHA-256d and the pool targets.

Both resulting blocks pass original Core acceptance. Each fresh chain moves
from height0 to1 and retains the exact tip after restart. Core generated
neither block. The [independent Core review](../reviews/native-core-raw-independent-495.json)
checks exact daemon and block bytes, all captured inputs and raw chain logs.

| Candidate | Block hash | Core evidence |
| --- | --- | --- |
| 0 | `0fa43c6301ac8340b9b51203efedfe5aeb5232c01e89310e9526a33d31333764` | [acceptance/restart](../bitcoin-core/native495-candidate0-495/block-acceptance.json) |
| 1 | `303b705e03e9379932a6f860644f3ce4e2eb5aca94ef4b2bfabcc5ff16562bdf` | [acceptance/restart](../bitcoin-core/native495-candidate1-495/block-acceptance.json) |

These commands ran from the repository root. They require the prepared
workspace and captured inputs. Output paths now exist. A new preparation must
generate fresh pool inputs for a new random-work test. Replaying prepared494
reuses its recorded seed and must retain that execution identity.

```sh
work/simulator/channel-target-494/release/examples/worker_channel_probe work/simulator/channel-target-494/release/md-sim-worker-ipc work/simulator/qemu-s9/backend-build-485-pool-kvm work/simulator/network-vm/native-browser-495 prepared=work/simulator/qemu-s9/native-browser-prepared-494 ssh-helper=work/simulator/channel-target-494/release/examples/remote_namespace_check ssh-browser-payload=work/simulator/network-vm/native-driver-024/payload.json support-accel=kvm
work/simulator/channel-target-494/release/examples/s9_asic_audit rates-shared work/simulator/network-vm/native-browser-495/execution work/simulator/network-vm/native-browser-495/execution/rate-audit495.json
work/simulator/channel-target-494/release/examples/s9_asic_audit regtest-shared work/simulator/network-vm/native-browser-495/execution work/simulator/network-vm/native-browser-495/execution/regtest-audit495.json
work/simulator/channel-target-494/release/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/native495-candidate0-495 shared work/simulator/network-vm/native-browser-495/execution/regtest-audit495.json 0
work/simulator/channel-target-494/release/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/native495-candidate1-495 shared work/simulator/network-vm/native-browser-495/execution/regtest-audit495.json 1
```

Source freeze494 passed formatting, strict Clippy, release builds and1,033
tests, with49 explicitly ignored integration entries. The ignored entries
are not execution passes. Later source work requires its own checks.
Earlier failed browser runs remain failed. The next CLI integration must
generate fresh pool entropy, bind its parent and child inputs, and pass the
same selected-run checks. Exact components, original-board execution, physical
timing, clean installation and the complete target matrix remain open.
