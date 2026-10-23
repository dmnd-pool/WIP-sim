# Network baseline evidence correction

The saved scripts are the source of truth. This review corrects their description.
The original manifests and scripts remain unchanged.

`qemu-s9/automated-update-cycle-004/console-adaptation.json` says all phases
require saved static network settings without a network restart. This is wrong.
Its initial script writes those settings and restarts the native network service.
Its resume script checks them without writing or restarting the network.

The initial script has SHA-256
`27b18f1737f84b523473124ef95b76a94d78a4f19a50d9d8495a37773e56798f` (3,267 bytes).
The resume script has SHA-256
`0c96c2774df7da2544976622060b80ea83b721c3604ecb476ff84f2b62cec586` (3,765 bytes).
The generator now states the two behaviors separately.

`qemu-s9/console-existing-pool-129.json` also has an imprecise operation field.
Its script checks the saved pool configuration and stable MAC. It then writes
static network settings and restarts the native network service. It does not
test saved static network settings. Its SHA-256 is
`7b8c883abf56e637f2b6fbf468a4742ba6a3fb88ada1bb3a20b39c7fe736bfe9` (3,033 bytes).
The 22:56 UTC progress entry must be read with this correction.

The provision stage changes the native boot environment to use a stable MAC.
Native preinit can then clear the previous network configuration on the next
boot. The initial phase sets the test network after that native action.
An initial DHCP readback alone does not establish a persistence defect.

The active local and SSH runs use the exact saved initial script derivative.
They can test pool configuration readback and mining after network setup.
They cannot establish network configuration persistence across restart.
No firmware or exact-target gate is closed by this correction.
