# Carrier service startup fix and actual VM test

The production Supervisor and service now start and stop in the isolated
VM. This result covers the real launch, account change, namespaces and
broker transport. Broker replies are explicit test data. No firmware,
HTTP, TLS or browser acceptance is claimed from this test.

Native access421 failed before any carrier service request. The captured
bwrap error said that --uid requires a user namespace. A new user mapping
would change the real peer account and file owner meaning. The fix keeps
the support user namespace. bwrap retains only SETUID, SETGID and SETPCAP
for the trusted Rust entry. That entry drops every bounding capability,
clears supplementary groups, sets all real/effective/saved IDs to1000,
clears remaining capability sets, sets no-new-privileges and disables
dumpability. It verifies the result before reading target metadata or
contacting the broker.

Actual startup424 exposed a second launch error. Root with these limited
capabilities cannot enter UID1000-owned0700 /work/access. The command now
starts at /. The trusted entry enters /work/access after the account drop.
No directory permission was broadened. Both failed421 and failed424 remain
available with their original evidence.

Actual startup426 passes. Its captured production helper made11 real Unix
broker exchanges. The accepting broker observed UID/GID1000. The service
records all six ID slots as1000, no supplementary groups, all five Linux
capability sets zero, no-new-privileges1 and working directory/work/access.
The mount, PID, IPC and UTS namespaces differ from the parent. The user and
network namespaces are preserved. A deliberately non-close-on-exec parent
file descriptor is absent in the child. Parent pool scratch, broker inputs,
client keys and server TLS keys are excluded. Root-owned selected metadata
was read successfully. The real endpoint and ready record have the selected
identity. Stop closes listeners, removes the Unix socket and records no
error. The outer VM exited0 without truncation; its maximum RSS was371964
KiB. The guest kernel was7.2.9-arch1-1. The production integration kernel and
its complete firmware execution remain separate evidence.

The focused helper suite passes90 tests. Its three ignored outer entries
are the existing two child helper entries and this explicit VM test. The
VM executes the new ignored test and reports one pass. Strict production
and test-target Clippy427 and formatting checks pass.

Reproduction uses the captured inputs:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 cargo build -p md-sim-firmware --example remote_namespace_check --offline --locked
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 cargo test -p md-sim-firmware --example remote_namespace_check --offline --locked
work/simulator/check-target-085/debug/examples/isolated_network_vm NEW_OUTPUT work/simulator/carrier-startup-vm-426-inputs/scenario.json
```

The actual command used NEW_OUTPUT=work/simulator/network-vm/carrier-startup-426.
That output directory must not be reused. The scenario captures the exact
production and test helper files. Current test source differs from its
captured source by one formatter line wrap only. Reversing those20 whitespace
bytes reproduces the captured SHA-256 exactly; both source forms are saved.
Production source is unchanged after the VM test. Source, binaries, inputs,
results and check logs are pinned in
[the manifest](carrier-startup-426-inputs.json).

Next: freeze the fixed source and selected release binaries, then repeat
the actual archived firmware HTTP/TLS workflow. Keep failed421 separate.
