# Existing HDL tool build502f

The isolated build and tool checks pass. This is not selected FPGA behavior,
a processor model, firmware execution or an original-board pass.

The existing Icarus Verilog12.0 source is pinned to commit
`4fd5291632232fbe1ba49b2c26bb6b2bf1c6c9cf`. The original source archive has
SHA-256`690f02e6fd0e0c05e4768bcb65a2c7f1b64d20289344d1ad3ccfb69bae8bd55b`.
The original GNU gperf3.1 archive has
SHA-256`588546b945bba4b70b6a3a616e80b4ab466e3f33024a352fc2198112cdbb3ae2`.
Its detached signature verifies against the captured official GNU keyring.
See [the signature record](gperf-signature-502b.json).

The Rust driver runs the unchanged upstream configure, build and install
programs in the existing isolated preparation runtime. Inputs include the
pinned compiler archive and manifest, selected manifest-checked compiler
files, explicit Perl library path, captured bison/flex data and loader files.
No acquired source executes on the host. There is no external network,
physical device or host tool-tree mount.

Six focused tests, strict Clippy and the release build pass. The279 relevant
source and data entries are unchanged across those checks. The immutable
[helper manifest](../qemu-s9/iverilog-driver-502f/manifest.json) identifies the
actual executable. The full workspace gate is a separate check.

The run executes all upstream gperf checks serially. It builds Icarus, checks
its version, compiles and runs arithmetic, unknown-value and event-order
vectors, and requires malformed HDL to return an actual syntax error.
It packs the installation, renames the original installation away, restores
the saved archive and repeats compilation and execution. All checks pass.
No output stream is truncated. The isolated run completed in113.082 seconds.

The saved tool archive is3,828,781 bytes with SHA-256
`3c5631a6d6285affa39d9fddb9acca3758988b9c2fe6cbc4e3ce227f5fc33f61`.
It extracts to`/work/iverilog-install`. The restore check still uses the
recorded build library environment. It does not prove a standalone runtime
closure. The selected HDL check will test a smaller captured library set.

The isolated limits are3GiB address space per process,600 CPU seconds,
1,200 wall seconds,128 process slots,128MiB per file,64MiB trace and2GiB per
scratch mount. Two compiler jobs run in parallel. These are build budgets,
not firmware clocks or physical timing claims.

This exact command passed. Use a new output path for a repeat.

```sh
work/simulator/qemu-s9/iverilog-driver-502f/build_iverilog_tool \
  /home/user/src/md-sim work/simulator/qemu-s9/iverilog-build-502f
```

The [build report](../qemu-s9/iverilog-build-502f/build-report.json),
[process specification](../qemu-s9/iverilog-build-502f/spec.json),
[input records](../qemu-s9/iverilog-build-502f/input-identities.json), and
[independent execution review](hdl-tool-independent-execution-502f.json)
retain the evidence. Earlier capture, test-order and compiler-setting
failures remain in [the failure record](iverilog-build-failures-502.json).
No source, test assertion or expected result was removed to obtain a pass.

Next: use this exact tool output to elaborate the retained497 netlist with
the original vendor primitives and run separate component vectors. The PS7
vendor file has no behavioral CPU model. The missing monitor table and33
ambiguous electrical settings remain open.
