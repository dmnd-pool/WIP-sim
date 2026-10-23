# T17 core-register protocol oracle review

This is an author review. Independent review remains blocked by the account
limit. No exact-board or mining gate passes from this experiment.

The exact acquired T17 update is89677e26b8fb4b25cd8b3157b0a73b4df88f10f9994f0e7f8102af9d11f99772.
Static inspection verifies the archive, nested gzip and U-Boot legacy header
and payload CRCs. Its100MiB disk yields the ARM miner binary
ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367.
The original archive is unchanged. Only derived evidence is written in work/.

Capstone5.0.9 linear disassembly identifies two64-byte routines at file offsets
117936 and118000. Their call sites place the resulting word in ASIC register
0x3c. Linear decoding alone can misread embedded data, so a separate execution
test calls both exact routines under the captured QEMU ARM user emulator.

The derived static ELF contains exactly one unchanged copy of each routine.
The read routine hash is57a96a1cf41038322a9ce46cf615bf161fdc528f5089a4c0e50bfeff4266b715.
The write routine hash is6914a2d19ac20c011eb801a32b04fc859604490ed2b83451968038414a8ecc89.
Oracle272 checks32768 reads and16384 writes. All49152 returned words agree
with the separately coded field rules. The complete196608-byte output, test
ELF, original input identity, tool versions and process limits are retained.

The C wrapper compiles with -Wall -Wextra -Werror and undefined-behavior
sanitizer traps. The vendor instructions stay unchanged and are not sanitized.
Only a native and an AArch64 Linux Rust target are installed; no ARM32 Rust
core library is installed. The small C wrapper supplies the ARM32 call and
Linux syscall ABI without a C standard library. Its embedded assembler copies
the original bytes with incbin. Rewriting those routines in C or Rust would
remove the independent vendor-code oracle. The orchestration and checks use
Rust. No acquired installer or complete miner runs on the host.

Early attempts remain failures. The ordinary ELF collector rejects Rust's
relative RUNPATH. The oracle instead captures two exact Rust1.99 linker files
and preserves their private directory relationship. Their fixed dependencies
come from the captured system library closure. The general collector is not
changed. The linker uses one thread under the process-count bound. The test
ELF must be ARM32 ET_EXEC with no interpreter or dynamic segment. QEMU11.1.2
needs the full32-bit address range for its high helper page. A256MiB reserved
range hit its assertion; a2GiB host address-space bound also failed. The
successful run has a separate6GiB virtual-address bound, a1MiB guest stack,
60 CPU seconds,90 wall seconds and no network. This is not6GiB resident use.

The confirmed driver word layouts do not establish silicon reset values,
core count, per-core timing, or readiness. Those remain model requirements.
Next: add a protocol decoder and compare it with the retained original-code
outputs. Implement the core-register effects before accepting them in the
running identification model. Do not turn a decoded command into readiness.
