# S9 PSU evidence512

The missing original ACRT1901202 report is now archived and hashed.
This closes the download dependency recorded in review352. It does not
resolve the exact PSU revision or physical behavior gate.

Original PDF: `artifacts/files/document/bitmain-apw7-acrt1901202-20261028/ACRT1901202.pdf`.
SHA-256: `65b6188f0c0f43567af73c2b758189529c3a0803005c09655d97b88343b169bf`.
Size:2,936,196bytes. Pages39–46 contain disassembly and sample photos.
The six suffixes listed by the report are candidates, not one selected PCB.
No unreadable marking is treated as a component identity. Material tests do
not measure power-on, load changes, protection, reset or thermal dynamics.

The frozen Rust CLI ran the captured Poppler renderer in private namespaces
with no egress. The first render retained missing-font diagnostics. A separate
explicit font input set produced eight clean page renders. Original document
bytes were unchanged. Direct encrypted stream extraction produced no images;
its failed guards were not bypassed. The raw specifications, results, image
hashes, font substitution and process limits are pinned in
[s9-psu-inspection-512.json](s9-psu-inspection-512.json).

Next: obtain one evidenced suffix/PCB and revision-specific measured traces.
Keep this source evidence separate from the original S9 firmware, FPGA
execution results and reference power assumptions.
