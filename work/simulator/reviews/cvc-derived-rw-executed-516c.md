# CVC callback scheduler component516c

The derived CVC scheduler passes all33 backend comparisons. The unchanged5ps
oracle passes on Icarus, compiled CVC and interpreted CVC. Thirty more comparisons
cover future and zero-delay callbacks, new NBA work, nested callback batches,
read-only observation, cancellation, and designs with and without delayed NBA.
NBA means a nonblocking assignment in the HDL event scheduler.

All four final invocations use the same frozen helper. They exit0 with complete
logs. Each actual program, plugin and fixture is retained. The largest archive
is2,203,289bytes, below the unchanged8MiB limit. Each run took about3.9seconds.
These are component results. They do not establish selected FPGA equivalence,
CPU coupling, firmware boot, physical timing or mining.

The [joined receipt](cvc-derived-rw-executed-516c.json) binds the helper, source
freeze, actual inputs, all33 observations and retained artifact hashes. The
[derived tool build](cvc-derived-tool-executed-516b.json) keeps the exact kernel
patch and original-source references. The original source archive is unchanged.

The [original CVC failure](cvc-rw-executed-516.md) remains failed. A later report
label error and a separate prelaunch store-lock error also remain recorded.
The latter attempt started before the previous preparation helper released its
store; it executed no guest. The final no-NBA run uses a new output directory.

Next: independently audit these results, then test the existing live VPI bridge
with its unchanged event, X/Z, clock and72-operation selected FPGA assertions.
