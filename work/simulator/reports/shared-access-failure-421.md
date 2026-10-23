# Selected access run421 failed

Build 420 ran the archived S9 firmware in the adapted reference environment.
The access suite failed before HTTP. This is not a native dashboard, exact
physical target, or milestone pass.

The root launcher passed `--uid 1000 --gid 1000` to bubblewrap without a user
namespace. The captured launcher rejected those options. The child did not
start. The client's selected endpoint inspection later timed out. The precise
error is saved in
[carrier.stderr](../network-vm/native-access-421/execution/shared-access/server-root/carrier.stderr).
The fix must keep the support namespace's real UID mapping. Mapping host root
to guest UID 1000 would change socket peer credentials and file ownership.

The native process ran for 305,149 ms, exited with code 0, and kept complete traces.
The pool ran for 300,512 ms, ended normally, and saved 10 sessions. These observations
do not establish mining or access acceptance. New independent mining/Core
audits were not granted for this failed access run.

The outer run took 829,296 ms. Its QEMU process exited with code 0, but the actual guest
suite and outer assessment failed. An emulator exit code is not a guest pass.

The joined retention contract worked on this failure. Before cleanup, the
host saved and verified the exact native result, pool bundle, and failed
access bundle. The runtime's stdin acknowledgment and the worker cleanup
name the same run and identities. The independent check rehashed all 24
access files and 10 pool-session files. See
[the independent retention check](../reviews/failed-access-retention-independent-421.json).

| Input or result | SHA-256 | Bytes |
| --- | --- | ---: |
| Native result | c5efb13b5978f36c6a8793e43a9073aa0485ba6cef7104b180ecd896e47e247a |105141709 |
| Pool bundle |7c9c0be6438e2c7d74d6febb85a7b0f9f0f9de7ea15caf72d892ad8794acc251 |107513 |
| Failed access bundle |fed463a21e71ae5ce984055cbbeab6507111d1275920ae6380099d8e4d77f693 |448850 |
| Outer result |1a6329197aeb7027221655121e32732eb628bd528b8c4898a0540925b686eaf3 | See saved file |

The actual run was
`run-4d850e0ab5e4948a70a4ca71e4048bf70b0cc52b5e3c50c29fc5e495fc39e803`.
Its lock hash was
`e6cdea6f0d59f2b36ce3142f38e098aa023d59903ac753f6ec617f07870dd1af`.
The [frozen tool hashes](../channel-target-420/SHA256SUMS),
[source checkpoint](../core-checkpoint-420/manifest.json),
[prepared process](../qemu-s9/native-access-prepared-420/process-spec.json),
and [actual inputs](../network-vm/native-access-421/ssh-inputs.json)
remain saved. Original downloaded firmware was not changed.

Next: fix the trusted service UID drop, test the actual production launcher
in a small isolated VM, review it, freeze new inputs, and repeat the full
firmware SSH/TLS run. Browser testing is separate and was not requested in run 421.
Its [measured support resource basis](../reviews/carrier-browser-resource-review-421.md)
does not change the native firmware's 360 s limit or 305 s observation period.
