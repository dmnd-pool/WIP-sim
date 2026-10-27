# Fresh CLI helper freeze 499

The helper cohort is ready for the fresh CLI test. It does not yet report a
CLI pass. The next action is to promote the CLI manifest, test the final
source, and execute the complete run 500.

The [cohort receipt](../channel-target-499/cohort.json) identifies all 13 helper
binaries and their build origins. Eleven helpers retain their exact bytes
from cohort 494. The worker transport and fresh preparation helper use the
reviewed source in [checkpoint 499](../core-checkpoint-499/manifest.json).
All 538 captured source files and the model library were rehashed.

This distinction is required. The original prepared shared-pool manifest
pins the remote support runner and pool verifier from cohort 494. Their
identity must not change during a derivation that changes only pool seed
and time. The native process still uses prepared input 494. Its x86 support
backend is the pinned KVM build 485.

The fresh preparation helper already passed a real derivation from 494.
See [the preparation audit](fresh-preparation-executed-498.json).
The final current-source formatting, Clippy and workspace tests remain due
after the CLI manifest promotion. The final CLI source and binary need a
separate capture because they will include the new cohort identities.

Use these paths in the CLI promotion:

- Helpers: `work/simulator/channel-target-499/release`
- Original preparation: `work/simulator/qemu-s9/native-browser-prepared-494`
- Support backend: `work/simulator/qemu-s9/backend-build-485-pool-kvm`
- Worker SHA-256: `72bef33479774d5979b79d5e86b723f4d6cb618e34543d1e46462b78bfd4cf4f`
- Fresh helper SHA-256: `ed84d3c7a998c64fc3b06d73d38d6dddf2afa68ef17ba804fd92d81783c0513a`

Run 500 must use a new output root. It must execute the new preparation
helper, the selected native browser suite, independent rate and mining
audits, and both Bitcoin Core acceptance checks. Join its executed lock to
the new preparation receipt. Keep any failure explicit. After this test,
save the requested checkpoint and pause. Do not start another work stream.
