# S9 PSU identity dependency, review 352

Date: 2026-10-23. Scope: power supply identity for the selected original
S9 firmware artifact `braiins-feed-4366569e66ab54bc0d15`. Its SD image has
SHA-256 `97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef`.
This research changes no catalogue records or runtime models.

The retained OEM documents support an APW7 candidate, but do not establish
one exact PSU revision for the selected firmware and board. A newly found
Bitmain-hosted report supplies six concrete APW7 suffixes to investigate.
This is a narrower search target, not a compatibility or execution pass.

## Retained primary evidence

These files were read and their SHA-256 values checked again. Catalogue
download records identify their OEM endpoint and model group. The dates
and filename labels do not establish firmware compatibility.

| File under `artifacts/files/document/` | SHA-256 |
| --- | --- |
| `bitmain-oem-ca8587658740cfdba3cb/S9_Server_Manual.pdf` | `3784d09ae7bbe0574f9dd94e3a1cd2bc279caf8d1b8242f3825e222f3cbc255d` |
| `bitmain-oem-9c52d72eaaf1dd825a05/S9_Server_Manual.pdf` | `cb0c40acb8a06b3a367a06149fbbecf94a8b1a2e6b03767e429c7428c32ad52b` |
| `bitmain-oem-92c823f02e4e8a320e04/APW7_manual.pdf` | `72e66019d859496f339f54271730893b3a9ddf3da4ededbec1d7030eb4258adb` |

The two S9 manual files differ. This review used the first file and its
existing text extraction, SHA-256
`adafdcad147e1d69c57607efdcf64e0461bba72fb85d6a53d9c1455a6f4b3f40`.
Its cover states document version 0.1, January 2017. Pages 4, 6, and 7
state that the customer supplies the PSU, give a rated input range of
11.60–13.00 V, and show three power connectors per hashboard plus one
controller connector. The APW3 efficiency example does not identify
an installed PSU. Power-supply identity cannot be inferred from this
manual or from the selected SD image.

The retained APW7 manual identifies an `APW7-12-1800` series. Its existing
text extraction has SHA-256
`e471a9c32c947c1ff7bffacb297fa7baf8a0522c2a34f206449bb1f4403c5e85`.
Pages 3–5 describe a fixed 12 V output, ten standard PCIe connectors, and
different ratings at 100–140 V and 200–240 V input. They specify series
limits for startup and protection. They do not identify an A/B suffix,
PCB revision, component list, or measured transient waveform. These
limits are useful constraints for a future model. They are insufficient
to calibrate an exact PSU model or to select its revision.

OEM download metadata:
`catalog/parts/bitmain/oem-download-index.jsonl`, records for the three
artifact IDs above. The S9 manual endpoint is
[the OEM S9 manual](https://assets-product.bitmain.com.cn/shop-product-s3/firmware/a96e87f6-563a-4a2f-813a-957ef8592732/2023/07/27/16/S9%20Server%20Manual.pdf).
The APW7 endpoint is
[the OEM APW7 manual](https://assets-product.bitmain.com.cn/shop-product-s3/firmware/APW7/Manual/007201810121123402484V85eL4G0612/APW7%20manual.pdf).

## New primary-source lead

The [OEM PSU compatibility table](https://support.bitmain.com/hc/en-us/articles/360034197034-Choose-the-correct-PSU-model-for-your-ANTMINER)
lists APW7 as an option for S9, with ten connectors and 11.6–13.0 V miner
input. Its page shows an update date of 2025-06-05. This is a vendor
model-level claim. It does not associate a PSU suffix with this archived
firmware, an exact controller, or a hashboard PCB revision.

[Bitmain-hosted report ACRT1901202](https://support.bitmain.com/hc/en-us/article_attachments/360042490854)
names six submitted models on page 1:
`APW7-12-1800-A3`, `APW7-12-1800-A5`, `APW7-12-1800-A6`,
`APW7-12-1800-B3`, `APW7-12-1800-B5`, and `APW7-12-1800-B6`.
It lists sample receipt dates in January–April 2019 and an approval date
of 2019-04-23. The report concerns material tests. It is not a power,
timing, firmware, or miner-compatibility test. Its text identifies sample
photos on pages 39–46.

The web provider returned parsed report text. Its page-image requests for
pages 39 and 40 failed with a cache miss. No raw PDF was acquired, so no
PDF hash, observed photo label, or PCB marking is claimed. Do not use the
live URL as a pinned release input. No unchanged failed request was retried.

## Exact blocker and next action

The selected target has no evidenced PSU suffix, physical PCB revision,
component inventory, or measured response trace. Six published suffixes
are concrete candidates. None is selected by the evidence reviewed here.
The current reference power assumptions remain an adaptation. Missing
models remain implementation work, not evidence of incompatibility.

Acquire and hash the complete ACRT1901202 PDF through the normal archive
workflow. Inspect its sample labels and board photos. If they identify a
specific suffix and PCB revision, link that identity to its exact OEM
electrical limits and S9 wiring. Then obtain an independent power-on,
load-change, overload, and reset trace for that same revision. A model
must preserve the physical supply behavior separately from the low-rate
hash budget. Do not assume the APW7 series limit is a measured waveform,
or that a material-test sample represents every PSU in that series.

## Reproduce the retained checks

```sh
sha256sum artifacts/files/document/bitmain-oem-ca8587658740cfdba3cb/S9_Server_Manual.pdf artifacts/files/document/bitmain-oem-9c52d72eaaf1dd825a05/S9_Server_Manual.pdf artifacts/files/document/bitmain-oem-92c823f02e4e8a320e04/APW7_manual.pdf
jq -c 'select(.artifact_id=="bitmain-oem-ca8587658740cfdba3cb" or .artifact_id=="bitmain-oem-9c52d72eaaf1dd825a05" or .artifact_id=="bitmain-oem-92c823f02e4e8a320e04")' catalog/parts/bitmain/oem-download-index.jsonl
```

No large payload, original archive, or executable was created or changed.
