# CVC tool prerequisite516

The pinned CVC source built and ran inside the isolated preparation runtime.
The run passed in 52.916 seconds with exit0 and complete retained output. It
compiled and executed a four-state arithmetic and clock fixture. Invalid HDL
was rejected. The exported tool was restored after the source and first install
were renamed. The restored tool compiled and executed the same fixture.

This is a tool prerequisite. Selected FPGA logic, CPU coupling, firmware and
mining acceptance remain open. The unchanged CVC scheduler has a known source
conflict with the required future-read/write ordering. The next test compares
its actual compiled behavior with Icarus at the same 5ps timestamp.

The source is the cambridgehackers mirror at
`1c5e043ec33ef6f1fdbf38565501c944893a83cf`. Its modified Artistic license,
README and change record are preserved. It is not labelled GPL or an official
current release. No source file was changed for this build. All compiler calls,
including the upstream conversion utility, received undefined-behavior trap
checks. Generated assembly is outside C instrumentation.

The original C build emitted 253 warning lines. All 85,046 stderr bytes remain
in the report. These include possible uninitialized values, array bounds,
formatting and pointer conversions. This is not a warning-free build or a proof
that those code paths are safe. The smoke compiles and executions had no such
diagnostic. The added Rust helper passed seven tests, strict scoped Clippy,
formatting and a release build. Its 596 source inputs match before and after.

The [execution receipt](cvc-tool-executed-516.json) pins all reports, source
reviews and tool outputs. The [build report](../qemu-s9/cvc-build-516/build-report.json)
contains the exact commands and full step diagnostics.

Executed command:

```sh
work/simulator/qemu-s9/cvc-driver-516/build_cvc_tool \
  /home/user/src/md-sim work/simulator/qemu-s9/cvc-build-516
```

Use a new output directory to repeat it. No host installation was performed.
