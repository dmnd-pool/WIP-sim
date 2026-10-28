# Original Zynq ROM read routine: run 513b

The unchanged public ROM read routine passed 28 cases in the isolated ARM
emulator. The process exited 0 after 217 ms. No output was truncated. The
compiler and emulator diagnostic streams were empty.

The complete 128 KiB ROM has SHA-256
`c58e2bc1d837654457ed47064accb26c4fb48307737e60d6c2d05253e7ee3aea`.
It is copied without changes into the test ELF. The test calls its original
`f_read` instructions at `0x8f7c`. A C fixture supplies the ARM32 call ABI,
filesystem objects and a cached physical sector. Rust checks the returned
status, byte count, file position, exact file bytes and unchanged destination
tail. C and the linker script express the existing ARM calling and memory
layout interfaces; Rust owns input checks, capture, isolation and results.

The selected SD image contains a 79,263-byte `BOOT.BIN` in 39 FAT16 clusters.
The Rust reader follows the chain and matches the prior isolated 7zip
extraction byte for byte. It records the MBR FAT32 label versus FAT16 BPB
conflict. The 609 allocated slack bytes are separate from the logical file.
The last physical sector is used as saved; the test does not invent padding.

The original routine clamps each request to the bytes left before logical
EOF. Requests include zero, one byte, a one-byte overrun, the declared image
length and `0xffffffff`. Tests start at four positions through EOF. Four
additional cases check invalid filesystem identity, denied read, a file
error and an uninitialized disk. All bytes after the returned count remain
`0xa5`, including when the physical sector contains zero slack.

The header source offset is 2,240 and its declared length is 79,263. A full
copy of that length from that offset would exceed logical EOF by 2,240 bytes
and the allocation by 1,631 bytes. This routine result rules out such a copy
through this ROM's `f_read` path. Static instructions show that the SD wrapper
does not use the returned read count. That complete wrapper and its hardware
path have not yet run.

The public author identifies XC7Z010-CLG400A. The dump is not yet tied to the
selected S9 silicon revision. This is an original-ROM routine test in a
declared fixture. It is not a reset, full SD transfer, SPL boot, original-board,
physical timing or mining pass.

The helper passed 10 focused tests, strict scoped Clippy and formatting.
Its [freeze manifest](../qemu-s9/bootrom-driver-513/manifest.json) records 412
source inputs with equal before/after maps. The
[source review](zynq-rom-read-path-independent-source-513-root.json) is closed.
The [input and result manifest](zynq-rom-read-path-executed-513b-inputs.json)
binds all captured inputs, limits and outputs. See the
[assessment](../original-zynq/rom-eof-513b/assessment.json) and
[complete evidence archive](../original-zynq/rom-eof-513b/evidence.tar).

The first preparation, `rom-eof-513`, failed because the command selected the
stable 1.94 linker path instead of the pinned 1.99 closure. No isolated process
started. Its log and captured files remain saved. The successful command
changed only that argument. Use a new output directory for another run:

```sh
work/simulator/qemu-s9/bootrom-driver-513/s9_bootrom_oracle \
  artifacts/files/source/source-zynq-bootrom-3f64d2f/bootrom/zynq_bootrom.bin \
  artifacts/files/firmware/braiins-feed-4366569e66ab54bc0d15/braiins-os_am1-s9_sd_2022-04-29-0-5eb2c5fe-22.02.4-plus.img \
  work/simulator/original-zynq/sd-bundle-291/boot/boot.bin \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/original-zynq/rom-eof-513b
```

The next step is a separate original-ROM reset mode in the existing QEMU
board. It must model the observed ROM/OCM mapping and execute the ROM's own
CPU1 path. It must preserve the existing direct-SPL mode and keep unresolved
silicon identity and private remap behavior explicit.
