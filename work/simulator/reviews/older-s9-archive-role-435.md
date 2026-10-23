# Mixed S9 installer roles, check 435

The mixed installer has three explicit FPGA payload occurrences and two
different expanded configurations. The 434 mismatch applies to its initial
installation payload. It does not describe the later update payload. None
of these three occurrences matches legacy configuration
`720d532d79ade36f0a320a03d0e5c114440c33a82933578e641a1ed7cfed5d55`.

No installer or firmware code ran. This check used bounded isolated extraction,
byte comparison and the existing Rust bitstream probe. No simulator source
changed. No build was needed. It does not establish board compatibility.

## Exact payload relationships

The outer artifact is `braiins-feed-aee8d7f2ed41b10b2e6f`, SHA-256
`ad2e0119a32b70c790b57afc719d283c66a4bfb9206e1602159071e20c0aece7`.
The [saved index](older-s9-archive-role-435-index.json) reports 29 members and
no truncation. The [complete indexed paths](older-s9-archive-role-435-members.json)
include `firmware/stage2.tgz` and
`post-upgrade/firmware_2020-03-29-1-6b4a0f46.tar`.
All paths below are relative to the installer's single top-level directory.

| Occurrence | Compressed SHA-256 | Complete configuration SHA-256 | Role from retained code |
| --- | --- | --- | --- |
| `firmware/system.bit.gz` | `9482ef607556a168e5b9e9ecc6de3198578cec9fb2187ed7d16ea451952b8552` | `361d7083ca761ffb4f57df346caf41c2683c5bbc7b311c2057e2c829fd012166` | Stage 1 writes the initial boot bitstream. |
| `firmware/stage2.tgz` → `system.bit.gz` | `37cff73235eb7e1236e53612fc6201ee300588a773db9e82b36fce2623ed2b87` | `361d7083ca761ffb4f57df346caf41c2683c5bbc7b311c2057e2c829fd012166` | Stage 2 writes the recovery bitstream. |
| `post-upgrade/firmware_2020-03-29-1-6b4a0f46.tar` → `sysupgrade-am1-s9/fpga` | `c606e69af819414148e1d93c4ebf2d3d3d26e9e74db5200c45562cd132f47adf` | `1be02e841bea3e51ae3f9367c8dfc4b014f91c5fb6b410032e3765795c8183ac` | Stage 3 invokes the normal system upgrade on this package. |

The first two expanded containers are byte-identical by their isolated probe
hash, `b147f09919fb40317c27f57e76fd39380a114d6236535068fb4618a28c9e09b5`.
Their gzip streams differ only at byte positions 5 through 8, the gzip
modification-time field. The isolated compressed-byte comparison exited 1.
That failed comparison is retained; it is not reported as a passing equality
test. The following probe established equality after expansion.

The later update is byte-for-byte equal to a separately archived package:

- Artifact: `braiins-feed-5ea46cd9c3bd3d2ec909`.
- Archive: `artifacts/files/firmware/braiins-feed-5ea46cd9c3bd3d2ec909/firmware_2020-03-29-1-6b4a0f46_arm_cortex-a9_neon.tar`.
- Size: 13,158,751 bytes.
- Rechecked SHA-256: `8c05a3258d857cb51bec2b8a88749fccfa967ca601f3ef29a4784d158c6e1b43`.

The equality process reads the nested tar stream and compares it with the
captured existing package, including end-of-file. Its pipeline uses `pipefail`.
It exited zero with no output or error. This establishes the relationship by
bytes rather than by the shared version string. No duplicate update payload
was acquired or retained.

The update's FPGA gzip is byte-identical to
`work/simulator/original-zynq/sd2020-bundle-001/boot/system.bit.gz`.
The new isolated probe gives the same expanded hash as the retained SD
container: `569597aa2f59d69d135b5024016f31232bb2e54b0d2c00f5bdb29853ab564713`.
The complete configuration hash above was rechecked over the exact 2,083,740
bytes starting at offset 118 in that container. This reuses a verified static
byte relationship, not another target's execution result.

## Installation and artifact constraints

The extracted `upgrade2bos.py` packages `post-upgrade/` as stage 3. The original
`stage1.sh` writes `system.bit.gz` at the offset specified by CONTROL and saves
the later stage locations. CONTROL requires the reported `XILINX` controller,
one of two NAND partition-table hashes, and its accepted miner-version strings.
These are installer checks, not evidence for an exact PCB or PSU revision.

The nested `stage2.sh` writes the recovery FPGA at offset `0x1400000`. It erases
the active FPGA and firmware slots and preserves the later update tar for the
following stage. The extracted `stage3.sh` invokes `sysupgrade` on the selected
2020 package. These scripts were read only.

The update CONTROL says `BOARD=am1-s9`. Its COMMAND records:

- Required baseline: `2018-10-24-0-9e5687a2`.
- Update version: `2020-03-29-1-6b4a0f46`.
- Major version: `2020-03-29-0-6ec1a631`.

The source uses its version checks before upgrading. This does not prove that
an arbitrary board with a similar name satisfies those checks. The outer
filename's 2019 string must not replace this required baseline. The inspected
stage-2 archive contains seven entries; its actual listing and script are
retained. Kernel and filesystem payload contents were not exhaustively scanned.
The claim is limited to the explicit FPGA members and install paths above.

## Evidence and cleanup

[The result index](older-s9-role-435-results.json) retains all 14 process
results. Thirteen exited zero. The compressed-byte comparison exited 1 for
the observed gzip header difference. All results are untruncated and have
empty standard error. Both new FPGA probes pass the static packet, checksum
and frame checks for the pinned part geometry. They do not reconstruct logic.

Each process specification captures the exact archive and trusted executable
closure. Preparation has no network, a 20-second wall limit, a 10-second CPU
limit, 256 MiB memory limit and 1 MiB trace limit. The probe uses its existing
15-second wall limit. Limits are per process; no aggregate cgroup claim is made.

The [evidence manifest](older-s9-archive-role-435-inputs.json) pins the exact
specifications, scripts, extracted data, logs and results. The
[cleanup record](older-s9-cleanup-435-complete.json) records removal of 24
temporary Store objects, totalling 67,258,320 logical bytes. Exact source and
copy hashes were checked under exclusive Store locks before removal. Original
archives and all retained results, including the failed comparison, remain.

## New public lead and remaining work

One new public lead was checked after 434. The cached
[janstoon/braiins page](https://github.com/janstoon/braiins) identifies
[papampi/braiins-os](https://github.com/papampi/braiins-os) as its fork parent.
This supplies another public history path. It does not supply the full source
Git link or a configuration association.

Exact `e80262a` commit pages in both public repositories returned cache misses.
The public `janstoon/braiins` commit API was inaccessible through the web
provider. Exact-prefix searches returned no results. No full Git object ID or
source bytes were acquired. No prior DNS or private-server failure was retried.
Do not repeat these unchanged requests as a new investigation.

The source dependency is still the full source commit and generated build
inputs linked to a byte-matched FPGA configuration. Physical component and
timing inputs also remain missing. Those external gaps are separate from
required model implementation. None is an incompatibility exclusion.

The next available bounded source-resolution step remains the archived 20.04
update's exact FPGA member, after checking its own installer/update role.
Record a match or mismatch by complete configuration bytes. Do not transfer
the 434 initial-image result to an entire package or infer source identity
from a date, version register or filename.
