# Native browser preparation failure445

Run445 failed before the support virtual machine started. The retained root
command log says `Error: prepared client image identity differs`. There is
no outer process result, guest log or browser result. This run does not
establish firmware, browser, HTTP or TLS acceptance.

The explicit browser request and capture are present. `ssh-inputs.json`
records `browser_requested: true`. The scenario enables both browser flags.
The captured driver is
`e4fa7c16e781adca06276d669bfb36e55c80ddbb45f58fd9716d96313d69bb13`
(2,640,928 bytes). The payload manifest is
`e0bbf3d513f489df55b8838dc2b972a1340d27c4ba2e7fddf25d6336ddafc9a5`
(1,729,217 bytes). The initial selected process keeps wall360 seconds,
CPU330 seconds and3GiB address space. No native timing limit was extended.

The builder completed its base image, appended the declared browser payload,
and wrote `prepared-image.json` for the combined image. The probe still
compared that identity with the base image. Both actual files were rehashed
by this review:

| File in `client-image/` | SHA-256 | Bytes |
| --- | --- | ---: |
| `initramfs.cpio.gz` | `cd77bdaa9c422d49413799885e0d23b3878a5fe390443ba014ba24fc7487bdb8` | 93,609,485 |
| `combined-initramfs.cpio.gz` | `0765830df3072c8f270be2b43baf470b9075b502cf163d7ba7927a71d370b365` | 499,952,513 |
| `root-manifest.json` | `89aed860b484f244994917005bce4e6d58b712f5ffa6fffd20954ee3c54f2a88` | 874,638 |
| `prepared-image.json` | `e9c45511e5893c3a890c69a9e8bde8f1663b8372586848fb65ca8e16e1d77e08` | 628 |

The prepared-image record exactly matches the combined image and final
combined root manifest. The base image has a different identity. The root
command log has SHA-256
`de853c9067fb8fa30355af90d85dfc2e994cc694642730245e497b8f86f3562a`.
All files are retained under
`work/simulator/network-vm/native-browser-445/` and
`work/simulator/checks-2026-10-21-001/carrier-browser-445.log`.

The first failing boundary is the prepared-image check in frozen440
`worker_channel_probe.rs`. That code also later imports the base image path.
The builder already selects the combined path when a payload is declared.
The minimal fix must use that declared selection for both hash verification
and the mounted input. No-payload mode must retain its original base path.
Keep the full image, kernel and root-manifest identity checks. A regression
must distinguish base and payload images and reject a substituted selection.

The reviewer changed no source and ran no build, test or VM. The parent was
notified of the exact cause before any fix. The failed445 evidence remains
separate from the next run.

## Dependent path and resource review

Two later checks also name the base image: `worker_channel_ssh::support_proof`
and the independent `s9_external_pool` audit. They must use the same declared
selection as the launcher. The proof must still bind the selected image to
the actual outer `/initramfs.cpio.gz` mount. The audit must rehash that exact
selected source. No other dependent `client-image/initramfs` reference was
found in simulator source.

No Store quota increase is needed for the measured445 image.
`Store::open(..., 512 << 20)` sets the maximum size of one object. Its default
total quota is16GiB. The499,952,513-byte image is below the536,870,912-byte
object bound. Kernel, QEMU and helper bytes are separate objects. The parent
confirmed that this bound and the default mode will remain unchanged.

The874,638-byte combined root manifest fits the4MiB proof and independent
audit readers. The1,729,217-byte payload manifest fits the4MiB payload
metadata reader. The256MiB outer file limit applies to writes; it does not
prevent reading the larger immutable image mount. The prepared payload
archive has906,580,572 uncompressed bytes, within the declared1280MiB payload
limit. The combined compressed image also fits the builder's2GiB limit.

The existing explicit browser profile uses4GiB guest RAM,6GiB outer address
space, two virtual CPUs, a1200-second outer wall limit and2000 CPU seconds.
It keeps each browser leaf at768MiB and256 tasks, and keeps the native
360-second wall/330-second CPU/305-second observation limits unchanged.
These are declared bounds, not measured browser resource results. The next
actual run must retain peak memory, task, CPU and cleanup observations.
