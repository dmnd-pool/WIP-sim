# Source preparation process-limit fix 467

Both preparation paths pass after the narrow namespace-bootstrap fix. The
installed-tool preparation and frozen-toolchain replay produced the same
515,224-byte cgminer binary, SHA-256
`88602d125e46310d0e61f64622d50be25df285085c2fe09c63070d852c64a90a`.
The frozen replay matches the earlier validated build exactly. Both also ran
the existing C regression under UBSan and passed. No C source was changed.

The change is limited to prepare.rs and freeze_cgminer_toolchain.rs. Their
trusted launchers now create the user namespace before prlimit sets the same
256-process bound, then execute Bubblewrap. CPU 120 seconds, address space 2 GiB,
wall 180 seconds, scratch 512 MiB, temporary 64 MiB, file-size bounds, read-only
mounts, capability removal and private networking remain unchanged. The
installed-tool manifest records all three supervisor hashes. The frozen
build's existing preparation-inputs manifest adds the prlimit identity.
No installed /usr was added to the frozen replay's guest filesystem.

The original archive remains unchanged at SHA-256
`4b3515ec072904e81e91182f5f8d5f8b5b87361ef2a7e2131643d8207badb996`.
The selected source is cgminer commit
`b8491c66e7e22f23a9edf095dd1337ee581e88bd`. The patch and adaptation records
remain separate. These are source-adapted preparation results, not original
firmware execution or exact physical-target acceptance.

Focused checks passed: one preparation provenance test, four freezer tests,
strict Clippy, release builds and scoped formatting. The actual frozen replay
completed in 19.497 seconds with exit 0, complete evidence and no truncated
stream. It verified the captured tree before and after its build. Its pinned
closure is e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a.
The installed-tool build also exited 0; it retains its existing limited
installed-toolchain claim.

These exact commands ran from the repository root:

```sh
work/simulator/prepare-tools-467/cgminer-lab prepare . \
  work/simulator/firmware/prepared-host-467
work/simulator/prepare-tools-467/freeze_cgminer_toolchain build . \
  work/simulator/firmware/toolchain-snapshot-003 \
  work/simulator/firmware/prepared-closed-467
```

The outputs now exist. Use new output paths to repeat them. The frozen
supervisor tools are in prepare-tools-467; the source/check/output hashes are
in the adjacent input manifest. All compiler and preparation processes are
stopped. No archive, earlier result or unique source input was removed.
