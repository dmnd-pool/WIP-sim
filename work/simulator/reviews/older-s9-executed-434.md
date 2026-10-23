# Older S9 archive check 434

The selected mixed 2019/2020 SSH installer does **not** contain the retained
`zynq-io-bin` revision `9250b1e63102fcb824bf52c2d9ef914df5ef6dfc`
configuration in its indexed `firmware/system.bit.gz` member. This is a byte
identity result for that member. It does not establish hardware incompatibility.
Other archived candidates remain untested.

The original archive is unchanged. The isolated extraction, isolated Rust
inspection and isolated gzip expansion all exited with code zero. None
truncated its trace. No installer or firmware code ran. No simulator source
changed and no build was needed.

## Exact inputs and results

The artifact is `braiins-feed-aee8d7f2ed41b10b2e6f`. Its archive is
`artifacts/files/firmware/braiins-feed-aee8d7f2ed41b10b2e6f/braiins-os-plus_am1-s9_ssh_2019-02-21-0-572dd48c_2020-03-29-1-6b4a0f46.tar.gz`.
Its rechecked SHA-256 is
`ad2e0119a32b70c790b57afc719d283c66a4bfb9206e1602159071e20c0aece7`.
Its size is 38,793,347 bytes. The exact extracted member is
`braiins-os_am1-s9_ssh_2019-02-21-0-572dd48c_2020-03-29-1-6b4a0f46/firmware/system.bit.gz`.

| Data | Bytes | SHA-256 |
| --- | ---: | --- |
| Extracted gzip | 477,812 | `9482ef607556a168e5b9e9ecc6de3198578cec9fb2187ed7d16ea451952b8552` |
| Expanded bitstream container | 2,083,852 | `b147f09919fb40317c27f57e76fd39380a114d6236535068fb4618a28c9e09b5` |
| Complete configuration, offset 112 | 2,083,740 | `361d7083ca761ffb4f57df346caf41c2683c5bbc7b311c2057e2c829fd012166` |
| Legacy 9250 configuration | 2,083,740 | `720d532d79ade36f0a320a03d0e5c114440c33a82933578e641a1ed7cfed5d55` |
| Selected 2020 configuration | 2,083,740 | `49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78` |

The independent gzip expansion and the Rust probe give the same container
hash. The complete configuration hash is from the exact range above in the
expanded bytes. The mismatch does not depend on the container date or filename.

The static probe checks the expected `03722093` silicon identifier. Both
configuration checksum checks pass. It maps 5,144 frames with their error
correction checks and checks 808 padding words against the pinned part geometry.
The part file SHA-256 is
`1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095`.
These checks do not reconstruct logic or establish firmware execution.

The header reports `design_1_wrapper;UserID=0XFFFFFFFF;Version=2017.2`,
`2017/10/28`, and `19:45:06`. Those strings are retained reference data.
The baseline/update association is not inferred from them. The conditional
CONTROL inspection was not needed to resolve this mismatch and was not run.

## Executed commands and evidence

The six exact objects from
[the preparation manifest](older-s9-extract-434-inputs.json) were imported with
`md-sim objects import PATH --sha256 HASH --size SIZE` into the new
`work/simulator/older-s9-inputs-434` state directory. The selected CLI SHA-256
is `0f36a5779b450ca838ed8b2c5f23835fc9bb2e02db15c6229f75fbdf2c624bd6`.

```sh
work/simulator/check-target-085/release/md-sim \
  --state-dir work/simulator/older-s9-inputs-434 \
  process execute work/simulator/reviews/older-s9-extract-434-spec.json \
  --out work/simulator/qemu-s9/older-s9-extract-434

work/simulator/check-target-085/release/examples/s9_bitstream_probe \
  work/simulator/qemu-s9/older-s9-extract-434/system.bit.gz \
  work/simulator/qemu-s9/older-s9-frames-434 \
  --series7-idcode 03722093 \
  --part-json fixtures/fpga/xc7z010clg400-1/part.json \
  --part-sha256 1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095

work/simulator/check-target-085/release/md-sim \
  --state-dir work/simulator/older-s9-inputs-434 \
  process execute work/simulator/reviews/older-s9-expand-434-spec.json \
  --out work/simulator/qemu-s9/older-s9-expand-434
```

The output directories must be new on reproduction. Reimport the recorded
inputs first; their completed temporary Store copies were reclaimed.

The probe executable hash is
`108715e948b013372e02b5479db75c51b42f4b79275f6eb6431b626b66f75d5f`.
This is the actual captured executable, not the earlier 291 probe identity.
All three preparations use closed input sets and network isolation. Their
wall times were 120, 91 and 41 milliseconds. Limits use per-process resource
limits; no aggregate cgroup limit is claimed. The extraction has a 20-second
wall limit, 10-second CPU limit, 256 MiB memory limit and 1 MiB trace limit.
Expansion has a 4 MiB trace limit for its exact 2,083,852-byte output.

Retained records:

- [Actual result summary](older-s9-executed-434.json), SHA-256
  `6861dd72e1522a7275f7f7d78b1792fe11ff9248257d0b81d4021c09e5c35708`.
- [Complete evidence file manifest](older-s9-executed-434-inputs.json), SHA-256
  `6d28970684f429e9f00e223eeec4bd9ae4a86d6e2e80e838d55b8e7d72e1f5c5`.
- [Extraction result](../qemu-s9/older-s9-extract-434/result.json) and
  [derived gzip](../qemu-s9/older-s9-extract-434/system.bit.gz).
- [Probe result](../qemu-s9/older-s9-frames-434/result.json) and
  [complete frame report](../qemu-s9/older-s9-frames-434/header.json).
- [Expansion result](../qemu-s9/older-s9-expand-434/result.json) and
  [derived container](../qemu-s9/older-s9-expand-434/system.bit).
- [Completed cleanup record](older-s9-cleanup-434-complete.json), SHA-256
  `44c07326ea76cc5bb973bd6fbee6692d8343ef64eac7a06a001cec0c412c6815`.

Cleanup removed 13 temporary object copies, totalling 47,343,068 logical bytes.
Each source and copy was checked for exact size and SHA-256 first. Exclusive
Store locks covered verification and deletion. Original archive files,
derived results, Store policy metadata and unrelated stores remain.

## Next action and remaining gates

The next bounded archived candidate is `braiins-feed-d96e8b988aef3a521fe0`,
the 20.04 SSH installer. Its indexed `firmware/system.bit.gz` is 135,748 bytes.
The catalogue archive hash is
`4f28138a27869247038bec3dcd433b4ade8084e9d14d6f0f3cf64ac8a0220806`.
That archive has not been read or extracted in this check. Recheck its bytes,
capture an exact isolated extraction and compare the complete configuration.
Only a byte match would justify checking its CONTROL/update association and
following its source/build reference. Sizes or dates cannot replace this check.

External evidence gaps remain: the complete source gitlink behind the observed
`e80262a` prefix, exact generated build inputs, and exact board component and
physical timing evidence. Implementation gaps remain in the original-board
execution and model path. This mismatch resolves neither category. It is not
an incompatibility exclusion. The selected newer image and all other required
targets remain in scope.
