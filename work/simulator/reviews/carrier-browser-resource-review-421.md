# Shared carrier browser resource estimate

This is a support resource proposal. It does not prove a browser run passes.
The basic access run421 and frozen420 source are unchanged.

The actual native024 browser report records two cgroup memory peaks:
355,323,904 and335,122,432 bytes. Both have zero OOM events. The outer VM
used4096M of RAM. Its process record reports2,971,368 KiB maximum RSS.
That older topology included a nested firmware VM. The new shared carrier
runs the firmware outside this support VM. Browser022 separately measured
314,753,024 and318,074,880 byte leaf peaks without OOM events.

The exact native024 payload manifest has4548 files and916647483 file bytes.
Current browser leaf limits remain768 MiB each, with swap disabled. Using
both limits in the estimate gives1536 MiB, rather than assuming measured
peaks are a safe future maximum. Current basic421 root manifest contains
214,147,308 bytes; its worker artifact transfer adds a second roughly169 MB
copy. Retained native result, raw logs, serial export and browser profiles
need separate headroom. No large result JSON was parsed during run421.

Proposed explicit browser-only profile:4096M guest RAM and6 GiB host QEMU
address space. This accommodates about874 MiB browser payload,1536 MiB
browser leaf capacity,204 MiB base root,161 MiB transferred objects, at
least200 MiB retained result/logs and more than1 GiB guest margin. The
address allowance also covers the compressed initramfs, QEMU mappings,
translation buffers and host process overhead. It is not a measured peak
or an aggregate cgroup guarantee. Preserve all native execution, mining,
leaf, CPU and timing bounds. Record the selected support policy in the
input identity so resume cannot reuse the smaller basic profile result.

Use the existing explicit browser rootfstype=ramfs policy for this profile.
Old browser004 failed initramfs extraction under the default half-RAM tmpfs
root limit. The later repaired browser runs used ramfs with finite physical
RAM. Current worker_channel_probe does not yet set this browser policy.
The separate isolated_network_vm selected-browser6144M profile applies to
a different nested-board path and does not apply to worker_channel_probe.

Before acceptance, measure the new run's outer RSS, guest free memory and
leaf peaks/events; retain any OOM, extraction or startup failure. The
900-second support workflow budget and native360/305-second bounds are
independent. This proposal changes no CPU allocation or deadline.

Exact source and report hashes are in
[the input manifest](carrier-browser-resource-review-421-inputs.json).
