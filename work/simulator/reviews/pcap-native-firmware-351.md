# Archived S9 execution on reviewed PCAP backend 343

Date: 2026-10-23. Author: `exact_target_gap`.

The archived firmware completed a fresh run on backend343. The independent
verifier checked 1,326 actual computations. Two jobs produced blocks that
separate Bitcoin Core chains accepted and retained after restart. This is
an adapted S9 reference environment. The exact-board gate remains open.

## Identity and scope

The selected firmware artifact is `braiins-feed-4366569e66ab54bc0d15`.
The [boot bundle](../original-zynq/sd-bundle-291/boot/bundle.json) records its
original SD image and extracted SPL, U-Boot, FIT, and FPGA inputs. The run
starts at the original SPL payload. It does not execute BootROM. The
original payload bytes remain unchanged. The native monitor script has a
separately recorded fan-index patch. The fixed ASIC filter is explicitly
adapted to one zero bit. Guest ticket-mask writes and readback remain
active. The aggregate functional cap is 10 H/s across three chains of
63 BM1387 chips each. Chip counts are a reference profile, not a resolved
physical hashboard revision.

| Input | SHA-256 |
| --- | --- |
| Original SD | `97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef` |
| Boot bundle | `a7f940cb1ee4e690e5027d518768d8be6d98cc3c14e4493f9fbc21d17b4fb129` |
| QEMU binary, backend343 UBSan | `be5b22536955634b5c0e9609d000d0e0c982c5d1707dc39b3cc8b1b564c49274` |
| Final PCAP patch | `e90cd612c70536b8c054a2aae91cef1c9fcaa87e195bdc89cfc5b145489a0cdc` |
| Derived DevC C source | `864a58b1070e35ab93b9bf31bfda6df326254fa9e58ed61adaee72f135112505` |
| Immutable process specification | `06fb67ca8705a9e1f0cb0e2abd61749538439909a8cef4a4cc47766083b639b8` |
| Fresh pool input | `05f244b9e5db9c8ee703897a70b9a5b4ca9bc8e796cd58c87cd54e7550ed0cdf` |
| Adaptation manifest | `4ddc77ac1f870a7646d52c5d3344e7e7f6a3de5996cd4cfebbbf295f06673d53` |
| Restored SD snapshot manifest | `3ab9726f783cd3995c6708b78868acdc3d104dd44c31a020a01e2f6aa0fec8c6` |
| Restored SD snapshot gzip | `f4fb5a8b6980ac7f6e220f604bc1d6c981e08ed7ccc877d5a956a80c0fadd6b1` |
| Frozen independent audit binary | `2e1f772cb60dc230c873b0a602dbba014977670fa703e79e5655fe922365bf05` |
| Frozen Core checker | `83aaefb0e9d4a60ec7226a54acf0e40ca473a99f57d33c3c92b223cbeac84b82` |
| Bitcoin Core daemon | `a22238f0a1ea0b0a67a17032d2df0039e9d3ac946f2e86e0634ea47448692fae` |

All backend patches and model inputs are retained in the
[build evidence](../qemu-s9/backend-build-343-ubsan/).
[Derived source provenance](../qemu-s9/pcap-derived-343/provenance.json)
links the final patch, pinned QEMU source, and compiled regression results.
The [adaptation manifest](../qemu-s9/native-pcap-349/adaptation.json)
records remaining device, reset, timing, storage, and firmware adaptations.
The prepared process and its objects were not changed after execution.

## Execution and independent results

[Execution](../qemu-s9/native-pcap-349/result.json) exited zero in
307,544 ms with an untruncated trace. User, mount, network, PID, IPC, and
UTS namespaces isolated the processes. Inputs were read-only. Storage was
run-owned. There was no network egress. The pool ran in a separate Linux
VM over a private Ethernet socket pair on the same host.

The [guest log](../qemu-s9/native-pcap-349/guest.log) records original SPL
and U-Boot banners, kernel entry, Linux 4.4 boot, and actual native service
processes. Lines 1187 onward list `boser`, `uhttpd`, and `bosminer` listeners.
Early API samples returned connection refused while services started.
Later API samples and the final console script completed. This run did
not exercise login, browser assets, configuration restoration, or restart
persistence through the dashboard. It grants no dashboard gate.

The [configuration audit](../qemu-s9/native-pcap-349/configuration-audit.json)
passed. One actual guest DMA delivered all 2,083,740 configuration bytes.
Their SHA-256 is
`49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78`.
The selected bitstream container hash is
`980532de5eb666508dafb111466f30169a70b8b6ea662bdda61e17dbd55a4421`.
This checks configuration delivery. It does not reconstruct FPGA logic.
The backend and stderr logs contain no UBSan runtime-error diagnostic.

The [rate audit](../qemu-s9/native-pcap-349/rate-audit350.json) checked
442 computations on each of links 6, 7, and 8. Each computation had a
unique, contiguous aggregate grant. The shortest observed grant interval
was 131,506,952 ns, above the required 100,000,000 ns. This is evidence
for the software cap, not physical performance calibration.

The [independent mining audit](../qemu-s9/native-pcap-349/regtest-audit350.json)
reconstructed firmware-submitted headers, checked SHA-256d, applied
filters, nonce ownership, serial CRCs, and pool verdicts. It retained
11 matching serial results and 10 matched submitted computations.
The selected pool connection completed two fresh jobs, a clean-job
transition, and a difficulty change. Five submissions were accepted and
five were rejected as low difficulty. This run alone does not exercise
all invalid, duplicate, stale, or out-of-range rejection cases.

Both candidates below came from this firmware execution. Core did not
generate them. Each check used a separate isolated chain, with states
height 0, then height 1 after import, then height 1 after restart without
the import argument. Each phase stopped cleanly. This establishes Core
acceptance and process-restart persistence. It does not claim RPC checks
or host-crash durability.

| Job | Block hash | Core evidence |
| --- | --- | --- |
| `2807a8975b33c76d-0` | `19b2b64a01838bf83e02240a5fdf9d3707b0e4be329416a2ab2ad883bff47240` | [Candidate 0](../bitcoin-core/pcap349-candidate0-351/block-acceptance.json) |
| `2807a8975b33c76d-1` | `36fbaa4401af881beb028a1f5411bb2eeb13853c800f39ce51c46dc7b9d6664f` | [Candidate 1](../bitcoin-core/pcap349-candidate1-351/block-acceptance.json) |

## Commands that completed

Run from the repository root. Existing output paths are immutable results.
Use new output paths for a rerun. The prepared run retains its exact pool
input. Generate a new preparation for fresh jobs in another mining test.

```sh
work/simulator/pcap-target-322/release/examples/s9_execute_prepared work/simulator/qemu-s9/native-pcap-prepared-348 work/simulator/qemu-s9/native-pcap-349 local
work/simulator/ssh-target-263/release/examples/s9_asic_audit rates work/simulator/qemu-s9/native-pcap-349 work/simulator/qemu-s9/native-pcap-349/rate-audit350.json
work/simulator/ssh-target-263/release/examples/s9_asic_audit regtest work/simulator/qemu-s9/native-pcap-349 work/simulator/qemu-s9/native-pcap-349/regtest-audit350.json
work/simulator/ssh-target-263/release/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/pcap349-candidate0-351 work/simulator/qemu-s9/native-pcap-349/regtest-audit350.json 0
work/simulator/ssh-target-263/release/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/pcap349-candidate1-351 work/simulator/qemu-s9/native-pcap-349/regtest-audit350.json 1
```

The preparation command and resume state are recorded in
[checkpoint348](../qemu-s9/native-pcap-prepared-348/checkpoint.json)
and [preparation log](../qemu-s9/native-pcap-prepared-348.log).

## Evidence hashes and remaining work

| Report | SHA-256 |
| --- | --- |
| Execution result | `7f373b898257b73cc0d7136a21109539e6f26fac8ec829fac5d46c4f9c050fc1` |
| Backend trace | `d66ac267d3e2045aafea9ba1213d6f49b177d6935fad5ddc6ed59b009820ca8d` |
| Guest log | `d2936debcdd09bd420f2704bfe1443fa10a2090cd21d5229f75250137a1efc04` |
| Configuration audit | `0cc6c96fa762428d0ee59ed912a81cf6de6d794a1fa3f9c09a6f5fe885290ba7` |
| Rate audit | `7b3915ae16b06e245f4b0b5337f8bcc7a33c5ea8d5823d0dd3b9ce2e4b130f79` |
| Mining audit | `a0f9bb9c22be0faa36d5371467ff8cd3538dcd5f77485553ef18d6efbb78ef33` |
| Core candidate 0 | `875cf3af0daaee3533afc8a25ce2c967ede83caa9db018ba31fe6b3602861312` |
| Core candidate 1 | `dcf52749f45d97ae2ce862487823ca5741b83a976611d61fcf475b7c9e7a31cc` |

Six compiled PCAP regression suites passed on the same backend. See
[review338](pcap-independent-338.md). Independent review of the new
migration test remains required. Original-board execution, exact physical
component identity, remote integration, dashboard checks for this run,
and full-matrix completion remain open. No previous run result is reused
as evidence for this fresh execution identity.

The guarded cleanup helper did not yet accept these numbered native/Core
store paths at this checkpoint. Their inputs and duplicate Core evidence
were preserved. Cleanup must use checked hashes and a restoration journal.
The original archive and all unique run evidence must remain intact.
