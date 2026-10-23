# Persistent worker broker, cycle361

The real-worker regression passes. Two separate client processes use an
owner-only socket broker in one disconnected Linux VM. The broker owns the
worker connection for its complete lifetime. It drains each worker response
before it tries to deliver the response to the client. A lost client therefore
cannot leave a stale response for the next client. Complete worker errors keep
the native channel. An uncertain exchange closes the broker.

The test uses the production `WorkerExecutor`, a real isolated support process,
and a freshly generated input file. It kills client PID62 after the job prints
its first output. The same job's output grows from 135 to 810 bytes. Separate
client PID77 finds that same run still active, stops it, retrieves its complete
result, verifies its identity, and requests cleanup. Eight sequential output
records contain the fresh input's actual hash. A wrong-UID connection is
rejected. One client sends a real Status request and closes without reading
the response. The broker records this lost reply and continues correctly.
A complete error for an absent run also permits later valid requests.

The retrieved result is 2,880 bytes, SHA-256
`ac6b359dc38ec6265c5dc5ca800ad2ebc05add535a6e086502dc07277668997c`.
The host independently decodes the exact bytes, checks this hash against the
cleanup acknowledgment, and checks the output sequence and fresh input hash.
See the [assessment](../network-vm/worker-broker-361/broker-regression.json),
[exact result](../network-vm/worker-broker-361/retrieved-worker-result.json),
[broker audit](../network-vm/worker-broker-361/broker-audit.jsonl), and
[frozen input](../network-vm/worker-broker-inputs-361/fixture.json).

This is an inside-one-VM test with a real worker and separate client processes.
It does not establish SSH, external-host deployment, native outside-VM worker
integration, firmware execution, dashboard access, or whole-VM recovery.
The saved result reached the host after VM-local cleanup. This regression does
not claim an outer durable receipt before cleanup.

## Failures and fix

Cycle358 failed at its 35-second job-start deadline. Its guest child logs were
not exported, so its exact delayed phase is not established. Cycle359 adds
bounded failure-log export and a separate 90-second transfer/start diagnostic
budget. It shows every artifact transfer and both probes succeeding. The
debug worker then exceeds the unchanged five-second native request timeout
on ExecuteToken under software CPU emulation. The broker rejects the uncertain
exchange. Both failures remain saved.

Cycle361 builds the same current worker source in release mode. It passes with
the same five-second request deadlines and continuation assertions. The support
VM took 40.111 seconds. The diagnostic start budget is 90 seconds. This budget
includes fixture transfer and is separate from the job's 25-second wall cap.
No firmware timing or acceptance assertion changed.

## Commands and checks

These commands passed from the repository root. The named output paths now
exist; use new paths for another run.

```sh
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_BUILD_JOBS=1
cargo build -p md-sim-coordinator --bin md-sim-worker-ipc --release --offline --locked
cargo build -p md-sim-firmware --example worker_device_broker --example isolated_network_vm --offline --locked
work/simulator/check-target-085/debug/examples/worker_device_broker --prepare-test work/simulator/network-vm/worker-broker-inputs-361 work/simulator/check-target-085/release/md-sim-worker-ipc
work/simulator/check-target-085/debug/examples/isolated_network_vm work/simulator/network-vm/worker-broker-361 work/simulator/network-vm/worker-broker-inputs-361/scenario.json
cargo test -p md-sim-runtime --lib relay_frame_tests --offline --locked
cargo clippy -p md-sim-firmware --example worker_device_broker -p md-sim-runtime --lib --offline --locked -- -D warnings
```

The three framing tests check bounded requests, faithful complete-error
envelopes, and dispatch completion before a failed output write. The real VM
test provides the separate lost-client evidence. The tests do not substitute
protocol peers for firmware acceptance.

Source SHA-256 at cycle361:

| File | SHA-256 |
| --- | --- |
| `examples/worker_device_broker.rs` | `14a18b283414ebb1528742c8bb17df1573b53e69c10c93ca059383939a3cb65a` |
| `examples/support/worker_device_broker.rs` | `e6927b7ca2e588b529172508426b9b82b1db74c8ca73060ce334039de2617647` |
| `examples/support/worker_broker_regression.rs` | `b3acc82516a2963aa65b7427322bb3b715020dd71e6bb4d5e7762cd22a199303` |
| `md-sim-runtime/src/worker.rs` | `ff8caf219a39a3e8d044da62a15d085932f3980ab07db671c9e415112ae2f580` |
| `md-sim-coordinator/src/executor.rs` | `7cfd10ed085dd6afc4df8a1584052b2f234a2273df695140b9de14714fefad7b` |

The broker author is not its independent reviewer. The parent must review the
integration and run the final workspace checks after the other active changes.
