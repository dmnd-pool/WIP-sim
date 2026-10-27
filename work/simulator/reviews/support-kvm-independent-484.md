# Support KVM review

Status: runtime and corrected launcher source reviewed. The actual grant test,
scoped strict Clippy and corrected helper builds passed. The separate
[Linux boot and custom backend review](kvm-boot-build-independent-485.json)
also passed. The selected native/browser run is still pending.
The [review record](support-kvm-independent-484.json) pins source and logs.

The ordinary execution and worker APIs reject a support virtualization grant.
Only explicit trusted support calls can expose `/dev/kvm`. The runtime checks
the fixed character device 10:232 and API version12, and records kernel, CPU
and boot context. It grants no host directory, physical miner, external network
or emulator control port. Its receipt cannot justify cached result reuse.

The direct test passed four actual cases: UID1000 and UID0, each through direct
or streamed support execution. It checked inaccessible host paths and devices, closed
unrelated file descriptors, private networking, memory/file/process limits,
zero current capabilities and rejected privilege regain. UID1000 also retained
a zero capability bounding set. UID0 retained the same bounding set as its
ordinary isolated baseline. Exact streamed input and host receipts passed.

The initial UID0 test had incorrectly required a zero bounding mask. Its
diagnostic remains saved. The final test checks both actual privilege sets
and failed attempts to regain administrative capability. The separate failed
missing-device invocation also remains saved. The ioctl now supplies an
explicit zero third argument and records errno on failure. The corrected
actual host invocation passed; no missing-device result became a pass.

The first Linux attempt exposed another defect: `x86-64-v2` names an ABI level,
not a QEMU CPU model. Both support launchers now use `Nehalem-v1,enforce`.
The exact pinned QEMU archive was hash checked, and its CPU definition includes
the seven v2 additions and version1. Its `enforce` setting rejects unsupported
features. The [QEMU model table](https://www.qemu.org/docs/master/system/qemu-cpu-models.html)
also identifies this version as v2-compatible. The [ABI definition](https://gitlab.com/x86-psABIs/x86-64-ABI/blob/master/x86-64-ABI/low-level-sys-info.tex)
lists those required instructions.

KVM selection is explicit. It requires the selected shared browser profile
and excludes fault mode. There is no automatic TCG fallback. The ordinary TCG
settings and the separately isolated ARM firmware limits are unchanged. The
outer support identity must accompany the native identity in retained and
resumed results. These support checks establish no firmware, mining, dashboard,
exact-board or external physical-host acceptance.
