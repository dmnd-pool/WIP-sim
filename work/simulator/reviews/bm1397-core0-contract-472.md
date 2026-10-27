# BM1397 core0 continuation 472

Status: the original formatter execution and narrow readback view passed
in [oracle472](t17-core0-executed-472.md). The live core-write implementation
remains incomplete. The original T17 core0 formatter is at virtual address
`0x238f0`. Its 1,024 bytes have SHA-256
`8d3b1b4e80ef91084c23018fc6055b6bd5249b0e0f91119919f2fb20ef0f8245`.
It comes from the retained miner
`ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367`.
The existing source and result pins in audit437 were rehashed: all27 match.

## The next test

The wrapper retains original instructions, literal pools, strings and virtual
addresses. Bounded callbacks capture all `fopen`, `fprintf`, `fclose` and
`fputc` arguments. They supply only explicit test file handles or an open
failure. They do not supply the decoded register fields. No actual guest log
service, chip or clock is modeled.

The inputs are the 5,120 combinations already specified in analysis442,
eight explicit vectors, and64 logging cases. The latter cover debug levels
0,3,4,5, a second log, two independent open-failure bits, and software status
0 or255. Each caller slot and surrounding guards must remain unchanged.
The output verifier checks complete callback order and all argument words.
The C wrapper is necessary for the existing ARM32 ABI; the installed Rust
targets have no ARM32 core library. The CLI and verifier remain Rust.

The raw register word and software `FOUND` flag are separate inputs. The
executed collector446 established that `FOUND` can be set for a wrong chip
or chain without a copied word. No formatter output can fix that defect or
establish response provenance. A later semantic view must require a pending
read of core register0 and the expected ASIC address. It must preserve the
entire raw word, requested selector, reported core field and reserved bit.
It must not equate the encoded selector with physical core geometry.

## What an opaque write can establish

Probe035 observed `55aa5109003c8000807410`. The archived ESP driver sends the
same command at lines214–215. The independent archived Kano driver sends it
at line1528. Their member hashes are respectively
`9bd28dff43cb3c07a63c4843dba2704f7e071b1bfc7fb310cfd925cb35be2f56`
and `1ee1b0c2e6e99d212890edf8b31e312ad6f3b3927f4adc9b31c14826c9598ab7`.
The ESP driver then sends its ticket mask without reading completion. Kano
waits100ms. This is driver delay policy, not measured ASIC completion time.

A separately declared exploratory adapter could retain the posted core word
for each ASIC selected by the outer packet. It could then let the original
CPU reach its next ticket and PLL3 writes. This would observe later firmware
instructions. It would not implement the core-register effects.

Such an adapter must obey this limited transition contract:

| Input | Permitted observer behavior | Required limit |
| --- | --- | --- |
| Valid core0 write while powered and reset released | Retain exact posted word for selected ASICs and record the adaptation | No internal core fan-out or completion inferred |
| Write with no matching ASIC address | No selected observer state changes | No response or success event |
| Power loss or reset assertion | Invalidate retained observer state | This is loss of observer knowledge, not a silicon reset value |
| Core or mailbox read | Remain unsupported | No guessed readback, ready bit or reply |
| Job or clock-status request | Remain unsupported | No hashing, lock or enabled-core claim |
| Unknown control form, CRC error or partial packet | Existing strict error path | No state change from an invalid frame |

The existing strict path must continue to reject the core write until the
effects are supported. An exploratory adapter needs its own explicit option,
adaptation manifest, source hash, events and result identity. It cannot close
the original ASIC or board gate. This note does not enable that adapter.

For functional mining, the unresolved effects include multi-midstate mode,
clock gating, internal write fan-out, reset, operation completion and readback
provenance. In particular, `0x74` has the bit labelled `HASH_CLKEN` clear.
The label alone does not establish its polarity or its interaction with other
clock controls. It must not be converted into a guessed hashing-enable rule.

Once a supported state model exists, tests must cover addressed and broadcast
writes, two different chip states, repeated writes, absent and ambiguous
addresses, malformed controls, reset/power during a pending operation,
readback selection, and operations before/after completion. Clock events must
use the declared device clock, independently of the functional hashing cap.
Tests of an opaque observer remain separate from those device behavior tests.

The current public [BM1397 register page](https://github.com/skot/BM1397/blob/master/registers.md)
still marks reverse engineering as incomplete. The separately published
[BM1362/1370 mailbox layouts](https://github.com/256foundation/mujina/blob/main/mujina-miner/src/asic/bm13xx/REFERENCE.md)
do not resolve this BM1397 contract. No new public source identity was adopted for the model.
See the existing exact-source audit437 and executed collector446 for the
proven facts. Avoid repeating their packing implementation or execution.
