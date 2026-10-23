# Next exact-target dependency after SSH and TLS

Date: 2026-10-23. Scope: read-only evidence review during the frozen integration
run. No simulator source, catalogue, original archive, or selected run changed.

**Prioritize the source identity of the selected FPGA configuration.** Locate
the exact binary-history commit that contains its configuration bytes. Only
then resolve that commit's complete source Git link and build inputs. The old
public S9 binary archive is not a direct match. Resolving its old source link
alone would not close the selected-image dependency.

## New bounded finding

I re-read the retained `am1-s9/system.bit` member and the selected extracted
image. Both containers are 2,083,858 bytes. Both configuration streams start
at byte 118 and contain 2,083,740 bytes. Their differences extend into FPGA
configuration data; they are not confined to the container's date and time.

| Input or range | SHA-256 |
| --- | --- |
| Original `source-braiins-zynq-io-bin/bos-release` archive | `1d73e72fdc56a55eddce77d7e63e24fc62b231a57c38f744c5f26bc5ea612ba6` |
| Its `zynq-io-bin-bos-release/am1-s9/system.bit` member | `a843bc80acc3a076749bb70e36da1714d2f5fadf4b1b6d485c02411dda4610e3` |
| That member's complete configuration stream | `720d532d79ade36f0a320a03d0e5c114440c33a82933578e641a1ed7cfed5d55` |
| Selected `sd-bundle-291/boot/system.bit` | `980532de5eb666508dafb111466f30169a70b8b6ea662bdda61e17dbd55a4421` |
| Selected complete configuration stream | `49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78` |

The selected `native-shared-prepared-390/process-spec.json` pins the last
container hash at `/pcap-reference.bit`. Run 349 also recorded delivery of
the complete selected configuration stream. This review does not reuse those
execution results for the current frozen run.

The configuration prefix before frame data is byte-identical. Using the
already checked selected frame ranges, 1,964 of 5,144 ranges differ. The first
changed frame address is `0x00000904`; the last is `0x00401ba7`. The only four
changed configuration bytes outside the frame-data range are at offsets
2,081,648 through 2,081,651, the selected parser's first CRC word.

This is a bounded byte comparison. The archived third image has not received
a new isolated packet/CRC/ECC inspection in this task. It proves different
configuration bytes, not different externally observable logic or incompatibility.
The two frame-data hashes are `233b4b6e7c90244d1d0e7d7e05e4d3fafbcddc5086958821330fdc8a08c3c0c6`
and `2b3cb98f401cd97231d71603d080483a36a9d35f917345b2e3bd435892a09905`.
The complete list of changed ranges and both hashes for each range is saved in
`selected-fpga-frame-comparison-422.json` (585,280 bytes, SHA-256
`de57b4c03f05993a21c0cd22cbacb637d91ea7570399d57a49976796713eda23`).
Every selected range hash was also checked against the retained frame map.

## Concrete next step

Use the public `braiins/zynq-io-bin` history for the exact path
`am1-s9/system.bit`. Keep every candidate archive or blob's commit, size and
hash. Reuse the existing isolated `s9_bitstream_probe` and pinned part file
to inspect candidates. Compare the complete configuration stream with
`49d0faa4…bea78`; do not match by filename, reported S9IO version, or date.
If a matching commit exists, resolve its full 40-character `am1-s9/src` Git
link. Obtain the source, generated IP settings, constraints, tool version and
build recipe for that commit. Require a checked association to the selected
configuration before assigning its reset or FIFO behavior to this target.

Use a bounded path-history query, not an unrestricted mirror or a new archive
scan. The archive member database has already been queried for the retained
build sources. The old build configuration selects a platform binary repository;
it is not the missing selected FPGA build provenance.

One new public query was attempted:
[exact archived-commit contents API](https://api.github.com/repos/braiins/zynq-io-bin/contents/am1-s9/src?ref=9250b1e63102fcb824bf52c2d9ef914df5ef6dfc).
The provider returned “not accessible via this tool.” No bytes or full Git
link were obtained. No unchanged private-server failure was retried. Resume
remote source acquisition only with a new source or changed access condition.

The follow-up used a new path-history lead, not the inaccessible private source.
The [public repository root](https://github.com/braiins/zynq-io-bin) was available
as a cached page. It listed `bos-release` and 13 commits. It supplied no commit
IDs or binary bytes. The provider returned cache misses for the exact bitstream
history, branch list, and branch history pages. Exact-hash and repository/year
searches returned no results. One bounded public Git `ls-remote --heads --tags`
attempt failed immediately because `github.com` could not be resolved.
No matching commit or full source Git link was acquired. These access failures
remain external blockers; changing the URL repeatedly cannot establish source
identity.

## External dependencies and implementation work

| Priority | Type | Required result |
| --- | --- | --- |
| 1 | External source evidence | Exact selected FPGA source/build association, or the complete pinned tile, connectivity and feature databases needed for a validated reconstruction. The retained part geometry alone is insufficient. |
| 2 | Implementation plus independent oracle | Replace the inferred S9IO 1.0.2 FIFO/reset behavior with behavior checked against that source or reconstructed logic. Exercise boundary queues, destructive disable, warm/cold reset, clock gating and interrupt timing. Generic PCAP transfer success does not close this gate. |
| 3 | External component identity and traces, then models | Resolve one controller PCB, ordered hashboard revisions and a PSU suffix/PCB. Obtain reset and power traces for those exact parts, then implement and validate their behavior. The APW7 series manual is not a measured waveform or a chosen PSU revision. |

The fallback FPGA reconstruction needs the missing Project X-Ray tilegrid
whose retained publisher hash is
`db16874f2827fc05248ad4a7ef5769deaa8e70158a60c8dd40194c48713479ee`,
plus matching feature and connectivity data. No guess from the G19 hardware
metadata can supply the S9 reset parameters. Unimplemented models remain work;
they are not incompatible-target exclusions. No exact-target completion date
can be derived from these inputs.

## Checked evidence

These hashes were re-read from retained bytes:

| Record | SHA-256 |
| --- | --- |
| `reviews/exact-target-gap-338.md` | `cbe50fa5a8025ddc871e4c5cd3ef4a4dd2d47ae09922463fa482e3b68ceaabf0` |
| `reviews/s9-psu-identity-352.md` | `03870b6bdaac4ac338a813e02048e892be2844eda0186991cf4349a2bb3b2b73` |
| `reports/series7-frame-mapping-291.json` | `d9a675e3b06517f7236334e83d058ff69dc304ec1423847946ef688182b74f3f` |
| `qemu-s9/frames-selected-291/header.json` | `a5e5a889a42bc929ff5c67cce88d5d60d2ef05c5682a6f1339cf1ec47128acbe` |
| `qemu-s9/reset-reference-302.json` | `5f11510659399ad5f4ce2a105e86f7ce4646e648ac4494e593cb63a2723a2f62` |
| `qemu-s9/pcap-reference-313.json` | `f244b1215964544f9580524688c42f15d0ee39b9f6c786dd95edc8310fb814d8` |
| `fixtures/fpga/xc7z010clg400-1/part.json` | `1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095` |

Report paths are relative to `work/simulator/`; the fixture path is relative
to the repository. The retained OEM S9 manual hash remains
`3784d09ae7bbe0574f9dd94e3a1cd2bc279caf8d1b8242f3825e222f3cbc255d`.
The APW7 manual remains
`72e66019d859496f339f54271730893b3a9ddf3da4ededbec1d7030eb4258adb`.
These documents do not resolve an installed PSU revision.
