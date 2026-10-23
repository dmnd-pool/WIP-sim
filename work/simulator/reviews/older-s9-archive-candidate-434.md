# Older S9 archive candidate 434

State: preparation only. Native run 431 is active. No new firmware archive
extraction or simulator build has started. Frozen source 430 is unchanged.

No checked older image currently matches configuration SHA-256
`720d532d79ade36f0a320a03d0e5c114440c33a82933578e641a1ed7cfed5d55`
from the retained `zynq-io-bin` revision `9250b1e63102fcb824bf52c2d9ef914df5ef6dfc`.
This is not a statement that no archived candidate can match.

The previously extracted 20.03 SD image is already ruled out by configuration
bytes, not its filename or date. Its artifact is `braiins-feed-0f081f628a7021a9149e`.
Its container is `569597aa2f59d69d135b5024016f31232bb2e54b0d2c00f5bdb29853ab564713`.
The re-read configuration hash is
`1be02e841bea3e51ae3f9367c8dfc4b014f91c5fb6b410032e3765795c8183ac`.
The existing bundle manifest independently records the same hash.

The retained catalogue and archive-member indexes identify one separate
pre-2020-labelled S9 baseline inside the mixed 20.03 SSH installer:

- Artifact: `braiins-feed-aee8d7f2ed41b10b2e6f`.
- Archive: `artifacts/files/firmware/braiins-feed-aee8d7f2ed41b10b2e6f/braiins-os-plus_am1-s9_ssh_2019-02-21-0-572dd48c_2020-03-29-1-6b4a0f46.tar.gz`.
- Re-read archive SHA-256: `ad2e0119a32b70c790b57afc719d283c66a4bfb9206e1602159071e20c0aece7`; size 38,793,347 bytes.
- Exact member: `braiins-os_am1-s9_ssh_2019-02-21-0-572dd48c_2020-03-29-1-6b4a0f46/firmware/system.bit.gz`.
- Indexed member size: 477,812 bytes. Its hash and configuration are still unknown.
- Adjacent `firmware/CONTROL` is 3,021 bytes. Its update/baseline role must be checked before treating this package as executable firmware for that configuration.

The catalogue query found no separate downloaded 2018/2019 Braiins S9 image
outside that mixed installer. Nested-member metadata also supplied no separate
old image. These are bounded index findings, not a full archive scan. Later
20.04, 20.06 and 20.10 installers have different indexed compressed sizes;
size differences alone do not exclude them.

## Extraction ready after run 431

`older-s9-extract-434-spec.json` is the exact isolated process specification.
It selects only the member above. It captures the installed tar/gzip executable
closure with exact hashes. No archive program or installer will run.
The process has no network, a 20-second wall limit, a 10-second CPU limit,
256 MiB address-space limit, 16-process limit and 1 MiB trace/file limits.

After importing its six exact objects to a new run-owned Store, run:

```sh
work/simulator/check-target-085/release/md-sim \
  --state-dir work/simulator/older-s9-inputs-434 \
  process execute work/simulator/reviews/older-s9-extract-434-spec.json \
  --out work/simulator/qemu-s9/older-s9-extract-434
```

The selected CLI hash is
`0f36a5779b450ca838ed8b2c5f23835fc9bb2e02db15c6229f75fbdf2c624bd6`.
Check exit zero, no truncation, and exactly 477,812 output bytes. Retain that
exact stdout as a separately hashed derived gzip. Then run the existing
isolated bitstream probe with IDCODE `03722093` and pinned
`fixtures/fpga/xc7z010clg400-1/part.json` hash
`1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095`.
Compare the complete configuration stream with `720d532d…fd5d55`.
Do not infer a match from the container header or compressed size.

The current probe binary has hash
`108715e948b013372e02b5479db75c51b42f4b79275f6eb6431b626b66f75d5f`.
Its actual process specification must retain that executable identity.
This preparation command is not yet an executed check. Preserve any failure.

## Source association and board gates remain separate

Even a matching firmware configuration would not yet associate that image
with the retained 0.2 RTL. The RTL archive has SHA-256
`dc5f58721f596d2fc36b50d5049461264986ac339c4b4ff4d383749b98f3a363`
and full commit `74b80e7a6d862e49355b79b45302842bdc58c116`.
The old binary archive's source link has only the observed short prefix
`e80262a`. Generated build history, dirty-tree diff, exact generated IP settings,
and a source-to-configuration check are still absent. The retained RTL README
describes those build outputs; the archive does not contain them.

Exact controller and hashboard revisions, PSU identity, physical clocks/reset
evidence and BootROM behavior remain open. Starting at SPL is still a distinct
boot entry. An older candidate would remain a separate execution identity.
It would not replace the selected 2020 configuration or reduce matrix scope.
