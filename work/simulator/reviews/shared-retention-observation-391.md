# Shared retention observation 391

This check independently re-read the outer retained artifacts from
`work/simulator/network-vm/native-worker-firmware-391/`. The reviewer wrote the
guest namespace helper. This is not an independent review of that helper.
The separate source review is in
`shared-guest-independent-private-channel-390.md`.

The actual SSH transfer retained 48 destination-verified objects with a total
size of 168,074,593 bytes. The pool lease began at guest monotonic time
219.064805114 seconds. The real pool process started at 219.092907425 seconds.
Its actual readiness marker was observed at 219.465793625 seconds. The gate
then forwarded the selected execution at 219.465812939 seconds. The pool
completed naturally with exit code 0 at 519.501005143 seconds. It was not
cancelled. The bundle retains all ten session files.

The outer guest record has `passed: true`. The native result records exit code
0, status `exited`, and no trace truncation. This observation is separate from
the independent mining audit and Bitcoin Core checks.

The complete retained bytes were hashed again with SHA-256:

| Artifact under `execution/` | Bytes | SHA-256 |
| --- | ---: | --- |
| `worker-result.json` | 105424186 | `152ff586fe592184fb801a051fa89d0033a966f01cc0f763d24af84d754f08c2` |
| `shared-pool/pool-bundle.json` | 105365 | `0c5dcd05f8bf9fad864e69b2419264a0f555548a292d669805d3e025f2410d4c` |

The native host receipt and worker receipt agree. The joined receipt agrees
with both byte identities above. The actual cleanup response names the same
run and exact result, and says `removed: true`. The final guest record carries
the same joined receipt.

The selected locked-run hash is
`2e86c24b5655571b0e6a051c55f3f35f2c1fe302288908c21c06bc99cd8d3588`.
The native run is
`run-50964620a12583b48e7af37dd2e9bb8842412fc2be9dc5d999453ac91b56c130`.

The topology was a native worker outside a local support VM. That VM held
separate SSH client and server namespaces and the independent pool. This was
not an external physical host test. It does not establish original-board,
exact physical target, dashboard, whole-VM-loss recovery, or full-matrix gates.

## Follow-up defect and fix

A later source review found that three root reads of broker-owned control
files could block while opening a writerless FIFO. Frozen source 390 and run
391 remain unchanged. Current source now opens these files without following
symlinks and in nonblocking mode. It requires a regular file, the expected
owner, one link, protected mode, and the declared size bound. A real-file
regression checks those properties. An independent reviewer confirmed the
defect before the fix.

Current source also adds diagnostic-only progress records every 15 seconds.
They report actual child status, complete broker audit events, and retained
file sizes. They do not change timing limits or acceptance decisions. This
addresses the absence of visible progress during the 219-second preparation
and SSH transfer stage.

Focused current-source checks:

- `work/simulator/channel393-shared-clippy.log`: strict Clippy passed.
- `work/simulator/channel393-shared-tests.log`: 82 passed, 2 ignored child
  helper entries.

These later fixes still need a new frozen integration run. The pass from 391
cannot be reused as execution evidence for changed source.
