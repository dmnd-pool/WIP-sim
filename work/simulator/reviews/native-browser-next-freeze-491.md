# Selected browser retry after transport repair

This is the proposed execution checklist after failed run 487. It is not a test
result. Preserve failed runs 469, 479, 487 and frozen 486. Proposed new paths are
freeze 491 and run 492; check that they remain unused before capture.

## Required evidence before capture

1. The actual KVM trace prerequisite 489b passed. The independent review
   rehashed 71 inputs and checked the complete fresh 8 MiB JSON payload, SHA-256,
   eight challenge-bound kernel records, and separate stdout/stderr channels.
   See [the receipt](support-trace-independent-489b.json). The failed 489
   noise test remains available. Its fix uses one syscall per kmsg record.
2. The actual ordinary worker transport 490 passed through the new live trace
   path, including destination hashes, receipt acknowledgement, result
   retention and cleanup. See [its independent review](worker-trace-independent-490.json).
   Complete the affected wrong-receipt and disconnect regressions as well.
3. Close independent source findings and check the final source state. The
   frozen 486 workspace baseline passed 1009 tests. New source includes the
   narrow cleanup 481 path, the FASM export component and the trace repair.
   Run affected checks and final full workspace formatting, lint and tests.
4. Preserve native firmware bytes, backend 343, backend 485, the original
   native 360-second resource budget, browser deadlines, the 300+5-second
   bridge window, 10 H/s cap and filter 1 adaptation. The explicit host-clock
   policy remains enabled and enforced for both real browser origins.
5. Keep all compiler workloads stopped during the measured browser run.

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
  work/simulator/core-checkpoint-491
```

Check every captured source against the live source and the manifest. Copy the
same twelve helpers listed in freeze 486 to `channel-target-491`; use separate
files, not hard links. Check the source and destination hashes. Write a new
`SHA256SUMS`. Create the shared pool manifest with the new pool verifier,
remote helper and support backend 485 identities. Keep the existing kernel and
carrier limits. Bind the new manifest hash in the carrier dashboard policy.
Save all checks and source identities in the new freeze record.

## Prepare the selected native path

```sh
work/simulator/channel-target-491/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-291 \
  work/simulator/qemu-s9/native-browser-prepared-491 spl sd2022-v1 \
  backend=work/simulator/qemu-s9/backend-build-343-ubsan \
  budget=360 pool-seconds=300 presence-pin-fixture=e0 fan-fixture \
  pic-contract asic-contract configuration-contract asic-packets local-pool \
  shared-pool=work/simulator/channel-target-491/shared-pool.json \
  shared-dashboard=work/simulator/channel-target-491/carrier-dashboard.json \
  temperature-fixture pool-difficulty=-31,-30 regtest-genesis \
  hash-cap-hps=10 adapt-fixed-filter-bits=1 batch-console \
  console=work/simulator/qemu-s9/console-startup-diagnostic-133.sh \
  adapt-monitor-fan-source=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  sd-state=work/simulator/qemu-s9/automated-update-cycle-004/provision/sd-snapshot-0.json \
  adapt-clock-host-realtime prepare-only
```

Independently check every prepared input mapping. Verify that the process
declares `MD_SIM_S9_HOST_REALTIME_CLOCK=1`, the adaptation manifest identifies
the execution-host clock, and all native limits remain unchanged. The browser and retained verifier must require the selected clock policy.
The support init must include the validated md-sim.trace redirect. The host
launcher must put kernel UART bytes in backend.log and guest userspace bytes
in guest.log. ACK framing and the 4096-byte limit stay unchanged.

## Execute and assess

```sh
work/simulator/channel-target-491/release/examples/worker_channel_probe \
  work/simulator/channel-target-491/release/md-sim-worker-ipc \
  work/simulator/qemu-s9/backend-build-485-pool-kvm \
  work/simulator/network-vm/native-browser-492 \
  prepared=work/simulator/qemu-s9/native-browser-prepared-491 \
  ssh-helper=work/simulator/channel-target-491/release/examples/remote_namespace_check \
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
