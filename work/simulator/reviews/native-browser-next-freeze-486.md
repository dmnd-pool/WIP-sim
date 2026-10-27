# Next selected browser execution

This is the execution checklist after failed run 479. It is not a test result.
Use fresh output directories. Preserve freezes 468 and 478 and all failed runs.
Proposed numbers are freeze 486 and run 487; confirm they remain unused before
capture. All commands run from the repository root.

## Dependencies

1. Complete the explicit runtime KVM tests on the host that exposes `/dev/kvm`.
   Keep the UID0 diagnostic and the failed missing-device test. Both UID0 and
   UID1000 must pass the final grant tests. Ordinary runtime entries must reject
   a KVM request. The streamed-input case must preserve its exact bytes.
2. Complete the actual isolated support boot with KVM. An API12 ioctl alone
   does not prove that the support guest can boot.
3. Complete the new support backend 485 build. Verify its original QEMU source,
   recorded patches, toolchain and overlay, and frozen model checkpoint. Retain
   the build report, UBSan result and binary hash. The report must show KVM
   support. Backend 328 remains unchanged and cannot run KVM.
4. Close review findings and make all production source stable. Runtime tests
   and the six release helpers must include the corrected explicit zero third
   argument to `KVM_GET_API_VERSION`. Check errno on ioctl failure.
5. Run the required workspace checks on the final source. Record each command,
   its exit code, test counts and complete log. Keep intentionally ignored
   child-entry tests separate from the explicit executed KVM tests.

## Check and build commands

```sh
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_INCREMENTAL=0
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo test --workspace --all-targets --offline --locked
cargo build --workspace --release --bins --offline --locked -j2
cargo build -p md-sim-firmware --release \
  --example checkpoint_qemu_core --example isolated_network_vm \
  --example node_prerequisite_probe --example remote_namespace_check \
  --example s9_asic_audit --example s9_boot_bundle --example s9_browser_check \
  --example s9_execute_prepared --example s9_pool_guest --example worker_channel_probe \
  --offline --locked -j2
work/simulator/check-target-085/release/examples/checkpoint_qemu_core \
  work/simulator/core-checkpoint-486
```

Check every captured source against the live source and the manifest. Copy the
same twelve helpers listed in freeze 478 to `channel-target-486`; use separate
files, not hard links. Check the source and destination hashes. Write a new
`SHA256SUMS`. Create the shared pool manifest with the new pool verifier,
remote helper and support backend 485 identities. Keep the existing kernel and
carrier limits. Bind the new manifest hash in the carrier dashboard policy.
Save all checks and source identities in the new freeze record.

## Prepare the selected native path

```sh
work/simulator/channel-target-486/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-291 \
  work/simulator/qemu-s9/native-browser-prepared-486 spl sd2022-v1 \
  backend=work/simulator/qemu-s9/backend-build-343-ubsan \
  budget=360 pool-seconds=300 presence-pin-fixture=e0 fan-fixture \
  pic-contract asic-contract configuration-contract asic-packets local-pool \
  shared-pool=work/simulator/channel-target-486/shared-pool.json \
  shared-dashboard=work/simulator/channel-target-486/carrier-dashboard.json \
  temperature-fixture pool-difficulty=-31,-30 regtest-genesis \
  hash-cap-hps=10 adapt-fixed-filter-bits=1 batch-console \
  console=work/simulator/qemu-s9/console-startup-diagnostic-133.sh \
  adapt-monitor-fan-source=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  sd-state=work/simulator/qemu-s9/automated-update-cycle-004/provision/sd-snapshot-0.json \
  adapt-clock-host-realtime prepare-only
```

Independently check every prepared input mapping. Verify that the process
declares `MD_SIM_S9_HOST_REALTIME_CLOCK=1`, the adaptation manifest identifies
the execution-host clock, and all native limits remain unchanged. The new
browser and retained verifier must require the selected clock policy.

## Execute and assess

```sh
work/simulator/channel-target-486/release/examples/worker_channel_probe \
  work/simulator/channel-target-486/release/md-sim-worker-ipc \
  work/simulator/qemu-s9/backend-build-485-pool-kvm \
  work/simulator/network-vm/native-browser-487 \
  prepared=work/simulator/qemu-s9/native-browser-prepared-486 \
  ssh-helper=work/simulator/channel-target-486/release/examples/remote_namespace_check \
  ssh-browser-payload=work/simulator/network-vm/native-driver-024/payload.json \
  support-accel=kvm
```

Use the actual backend 485 directory if its final name differs. Run without
concurrent heavy builds. Record actual VM start and stop. Require successful
browser preparation before dispatch, and all existing HTTP and TLS origin,
fresh-profile, WebSocket, live DOM and shutdown-acknowledgment checks. Check the
original guest date/readback and both origins' five-second clock comparisons.
Verify every retained access file and the joined native, pool and access
identities. The outer KVM process and host receipt are part of that evidence;
the native hash alone cannot certify the support accelerator.

Run the frozen independent ASIC/mining audit and both Bitcoin Core candidate
checks against this exact result. Retain the complete headers, jobs, blocks,
device responses and node chain changes. A prior run's mining pass does not
replace these checks. Promote the one-command CLI manifest only after the new
cohort and independent review pass. Then execute the actual CLI command with
fresh outputs. This still does not establish an exact physical S9 board or
complete-matrix acceptance.
