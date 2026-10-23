# TLS component input cleanup plan440

No files were deleted. The existing reclaim_qemu_inputs path policy does
not admit tls-readiness-435, tls-readiness-437 or tls-readiness-438. The
directories were not renamed and the utility was not changed.

A read-only inventory held exclusive nonblocking Store-compatible flock
leases on all three finished stores and all three protected canonical
stores during hash and size checks. Each candidate has an exact existing
restore-copy path. The plan was flushed and synced before the leases were
released. These are candidates only, not completed cleanup.

| Finished store | All input bytes | Matching objects | Matching bytes retained |
| --- | ---: | ---: | ---: |
| tls-readiness-435 | 128,834,750 | 34 | 38,275,736 |
| tls-readiness-437 | 128,910,185 | 34 | 38,275,736 |
| tls-readiness-438 | 128,854,137 | 34 | 38,275,736 |

The102 verified duplicate candidates total114,827,208 bytes. All remain
present. Original archives, unique inputs, captured test helpers and
sources, failed435/437 results, passed438 evidence, logs and protected
canonical342/375 stores remain unchanged. No new build or VM ran.

[The durable plan](tls-readiness-cleanup-plan-440/plan.json) lists every
candidate, hash, size, restore-copy source and preserved input. It also
pins the terminal execution records and current cleanup utility source.

Next action, after the active release work: review a small utility change
for these explicit component run names and both success/failure evidence
forms. Require exact terminal execution and input identities, existing
exclusive locks, new durable journals and revalidation before removal.
Do not bypass the whitelist or use hard links.

Plan SHA-256: `91ee769c8975956535cd245ca6738d0a59f01ab9e7e72d006c65311609a0d0f3`.
