# Explicit support VM virtualization 484

The component checks pass after one production fix and two test corrections.
KVM access is optional and explicit. Ordinary firmware and worker execution
APIs reject a declared KVM request before opening the device. Their omitted
field keeps the existing JSON form. Only the trusted trace and streamed-input
support launch APIs can grant access.

The runtime opens only `/dev/kvm`, rejects symlinks, checks character device
10:232, checks the opened node against its path again, and calls the real API
query. It requires API version 12. The only added mount is that device. Private
network, PID, user, mount, IPC and UTS namespaces, closed file inputs, read-only
root, scratch limits, process limits and trace limits remain in place. The
runtime does not pass the host device descriptor into the guest.

A fresh receipt records the actual API version, kernel identity, unique CPU
signatures, processor count, CPU snapshot hash, boot identity hash, and time.
`cache_eligible` is false. This receipt does not establish that another host or
boot has the same capability. The launchers select `x86-64-v2` explicitly for
KVM and keep the ARM miner engine unchanged. No automatic accelerator fallback
is present.

The initial runtime call omitted the third variadic ioctl argument. The first
real root prerequisite failed before VM execution. The kernel requires a zero
argument for this request; the runtime and regression now pass an explicit
unsigned-long zero and preserve errno on failure. This was a production bug,
not a missing host prerequisite. See the [Linux v6.12 implementation](https://raw.githubusercontent.com/torvalds/linux/v6.12/virt/kvm/kvm_main.c)
and [KVM API contract](https://docs.kernel.org/virt/kvm/api.html#kvm-get-api-version).
The failed run and failed check logs remain saved.

The first new test incorrectly imposed the existing UID1000 bounding-capability
policy on UID0. Diagnosis found only CapBnd nonzero; permitted and effective
sets were zero. The corrected tests retain strict zero inherited, permitted,
effective, bounding and ambient sets for the actual UID1000 browser profile.
The separate UID0 prerequisite test measures its ordinary bounding set and
requires the KVM run to retain that exact set. Both profiles require zero
inherited, permitted, effective and ambient sets, NoNewPrivs=1, and actual
attempts to regain CAP_SYS_ADMIN through capset and ambient raise to fail with
EPERM. No production privilege policy changed for this test correction.

The second test correction concerns a protected proc file. This environment
returns an empty `/proc/net/route`; the first fixture expected a header line.
The final assertion permits an empty file or exactly the known header, and
rejects every route record. It also checks that no host network interface is
visible. The filesystem, resource, descriptor and device checks remain strict.

Results:

- Runtime library: 37 passed, four existing opt-in tests ignored.
- Default support integration: one passed. Ordinary requests are rejected;
  the isolated default guest has no KVM device and no host files or devices.
- Existing isolated-worker and private-channel regressions: 21 passed.
  This includes FD3 behavior, supervisor loss, cancellation and process bounds.
- Strict runtime Clippy with warnings denied: passed.
- Actual exposed-device integration on the parent host: one passed in 3.17s.
  It exercised both UID0 and UID1000, both grant APIs, real API12 in the guest,
  exactly one KVM mount, closed host descriptor222, bounded stdin, private
  filesystem/network, and unchanged limits. The child sandbox lacks `/dev/kvm`;
  its separate ENOENT result is retained and is not a host incompatibility.
- The six affected release helpers rebuilt successfully after the ioctl fix.

Verified check commands use the shared target directory and prerequisites in
the README. The first actual-device attempt below was unavailable inside the
child sandbox; the parent ran the same compiled test on its exposed-device
host and saved `kvm484e-root-actual.log`.

```bash
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=1
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo test -p md-sim-runtime --lib --test support_virtualization --offline --locked
cargo test -p md-sim-runtime --test private_channel --test isolated_worker --offline --locked
cargo clippy -p md-sim-runtime --all-targets --offline --locked -- -D warnings
work/simulator/check-target-085/debug/deps/support_virtualization-a45cdbe40c783af6 \
  --ignored --exact explicitly_granted_kvm_preserves_isolation_limits_and_stream_protocol \
  --nocapture --test-threads=1
cargo build -p md-sim-firmware --release --offline --locked \
  --example isolated_network_vm --example worker_channel_probe \
  --example build_qemu_backend --example s9_boot_bundle \
  --example s9_browser_check --example remote_namespace_check
```

The [input record](kvm-runtime-tested-484-inputs.json) pins frozen sources,
actual test executable, failures, successful checks, and rebuilt helpers.
The independent source review found no open issue after the ioctl fix. The
parent owns the separate Linux boot/TCP prerequisite and custom backend tests.
This component result does not establish miner firmware execution, a dashboard
pass, an external physical host, or an exact-target acceptance gate.
