# Exact S9 FPGA source and reset gap, review 338

Date: 2026-10-23. Scope: one required dependency for the exact S9 board gate.
No source, catalogue, original archive, or model was changed. No downloaded
program was executed. No exact-target gate passes from this review.

The retained Braiins binary archive does not supply the generated S9 reset
configuration. Its generated hardware metadata belongs to other boards.
This narrows the input search. It does not prove that S9 is incompatible.

## Checked archive identities

The following SHA-256 values were read again from the saved bytes.
The Git revisions come from the retained archive identity records in
`catalog/parts/bitmain/source-archive-identity.jsonl`.

| Input | SHA-256 | Retained Git revision |
| --- | --- | --- |
| `artifacts/files/source/source-braiins-zynq-io/master` | `d2fd4a51f1966fcf7161aa02f6ca838e8594694e5eb46e2fa3a6d4d952831010` | `7f8e9a564af46984c520a9ddc755080220031592` |
| `artifacts/files/source/source-braiins-zynq-io-bin/bos-release` | `1d73e72fdc56a55eddce77d7e63e24fc62b231a57c38f744c5f26bc5ea612ba6` | `9250b1e63102fcb824bf52c2d9ef914df5ef6dfc` |

Exact archive members were read as data. No extracted copies were retained.

| Member | SHA-256 | Observation |
| --- | --- | --- |
| `zynq-io-master/readme.md` | `e2aae9029d52ae85e00c913287b56104409b44c3a081d6efcbd98d4000e919b3` | Identifies the target board as G9/G19, the part as `xc7z010clg400-1`, and Vivado as 2017.4. This is not an S9 design identity. |
| `zynq-io-bin-bos-release/.gitmodules` | `1b9f38b53965c421dd4d6b326f03280a7c6142994ce882dd41a7181e701d5a7b` | The `am1-s9/src` source URL is `git@gitlab.ii.zone:x/braiins.git`. The archive has only an empty directory at this path. |
| `zynq-io-bin-bos-release/am1-s9/system.bit` | `a843bc80acc3a076749bb70e36da1714d2f5fadf4b1b6d485c02411dda4610e3` | This member is 2,083,858 bytes. It is not either expanded bitstream in the existing frame report. |
| `zynq-io-bin-bos-release/dm1-g19/system/system.hwh` | `93342ba569b9cd5319666add8be44815d7930620345e92053495b38737f55127` | Contains two generated `proc_sys_reset:5.0` instances. Both specify external and auxiliary reset width 4 and active-low reset inputs. These are G19 parameters, not evidence for the S9 parameters. |

The complete archive member list has three generated `.hwh` files:
`dm1-g9/system/system.hwh`, `dm1-g19/system/system.hwh`, and
`dm1-g29/system/system.hwh`. Its only S9 entries are the `am1-s9` directory,
the empty `am1-s9/src` directory, and `am1-s9/system.bit`.

The retained [frame report](../reports/series7-frame-mapping-291.json) identifies
the selected expanded bitstream as
`980532de5eb666508dafb111466f30169a70b8b6ea662bdda61e17dbd55a4421`.
It identifies the `frames-sd2020-291` input as
`569597aa2f59d69d135b5024016f31232bb2e54b0d2c00f5bdb29853ab564713`.
These hashes keep the image identities distinct when date labels differ.
Byte differences alone do not establish different behavior. No logic
equivalence or source-to-bitstream match was shown here.

## Public source observation

The [maintainer's S9 directory](https://github.com/braiins/zynq-io-bin/tree/bos-release/am1-s9)
shows `src @ e80262a`. This is a useful search key for the source dependency.
It is only a seven-character reference on a moving branch page. The web
provider reported a cached page. It did not provide the full Git object ID,
the source bytes, or proof that this page matches the archived revision.
It must not become a pinned source identity.

The provider could not fetch these more precise paths:

- `https://api.github.com/repos/braiins/zynq-io-bin/git/trees/9250b1e63102fcb824bf52c2d9ef914df5ef6dfc?recursive=1`: inaccessible through this tool.
- `https://github.com/braiins/zynq-io-bin/tree/9250b1e63102fcb824bf52c2d9ef914df5ef6dfc/am1-s9`: cache miss.
- `https://github.com/braiins/zynq-io-bin/blob/bos-release/am1-s9/src`: cache miss.
- `https://github.com/braiins/zynq-io-bin/commits/bos-release/am1-s9`: cache miss.

No remote byte-level acquisition or hash is claimed for these observations.
No unchanged failed request was retried.

## Remaining blocker and next action

The selected S9 image still lacks a source/build identity and generated reset
parameters. The older S9 source model and the G19 metadata do not close this
gap. The restrictions in [reset reference 302](../qemu-s9/reset-reference-302.json)
still apply. Physical reset timing and independent HDL execution remain open.

Obtain the full `am1-s9/src` Git link at binary revision
`9250b1e63102fcb824bf52c2d9ef914df5ef6dfc` and the corresponding source from
an authorized public source or the archive. Check the `e80262a` lead against
that full ID. Then obtain the exact source/build inputs and generated reset
configuration for the selected bitstream hash. Establish the association by
build provenance or independently checked logic. Do not assign the G19
reset values to S9 by similarity.

## Reproduce the local checks

Run these read-only commands from the repository root:

```sh
sha256sum artifacts/files/source/source-braiins-zynq-io/master artifacts/files/source/source-braiins-zynq-io-bin/bos-release
tar -tf artifacts/files/source/source-braiins-zynq-io-bin/bos-release | rg 'am1-s9|\.(hwh|xci|bd)$'
tar -xOf artifacts/files/source/source-braiins-zynq-io-bin/bos-release zynq-io-bin-bos-release/.gitmodules
tar -xOf artifacts/files/source/source-braiins-zynq-io-bin/bos-release zynq-io-bin-bos-release/am1-s9/system.bit | sha256sum
tar -xOf artifacts/files/source/source-braiins-zynq-io-bin/bos-release zynq-io-bin-bos-release/dm1-g19/system/system.hwh | rg -n 'C_EXT_RESET_HIGH|C_AUX_RESET_HIGH|C_EXT_RST_WIDTH|C_AUX_RST_WIDTH|MODTYPE="proc_sys_reset"'
```
