# Independent network and browser review

This file records scoped reviews. It does not mark the full simulator complete.

## Browser payload

Root reviewed `crates/md-sim-firmware/examples/isolated_network_vm/payload.rs`. The bounded encoder checks paths, file types, exact identities, overlap, link chains and archive size. Captured code does not execute on the host. A separate installed CPIO parser extracted a test archive inside strict isolation and checked the file and symlink contents. The actual Linux kernel also unpacked the appended gzip archive in `payload-boot-001`. Root reported no remaining scoped finding. Capture 003 had a missing symlink endpoint; the loader rejected it before boot. Capture 004 supplies the exact missing link chain. Actual browser execution remains a separate result.

## Browser process and protocol

Catalog agent reviewed `crates/md-sim-access/src/browser.rs` and `crates/md-sim-firmware/examples/browser_probe.rs`. The protocol client has bounded nonblocking frames, absolute deadlines, matching response IDs and explicit errors. The browser process uses an owned process group and observes exit with WNOWAIT before group cleanup. Two distinct profiles use the same origin. The second profile must not inherit the first profile's cookie. An actual DOM click must send a request that changes the separate service counter.

The first review found two issues. Parent-death signal setup lacked a check that the parent remained the same after `prctl`. The driver also requested certificate verification but did not check the returned capability. Root fixed both before the second prepared browser attempt. The test service remains clearly marked as a component fixture. It does not establish native firmware behavior or a TLS browser gate.

## S9 NAND controller

Catalog agent reviewed the initial PL353 controller and its C binding. The ECC parity packing matches the archived U-Boot correction contract. Timer and IRQ binding keep explicit state. Migration is unsupported. Initial controller validation allowed malformed end commands and incompatible data directions to reach the NAND engine. The implementation now rejects those transactions before bus effects. New negative tests cover invalid erase confirms, status writes and program reads.

Random-input support was incomplete, so the model now rejects it until a complete path is tested. The existing NAND engine commits writes at confirm time. Delayed READY alone does not establish reset or power-loss behavior. The firmware agent is implementing pending operations in the reused NAND component. Status reads must not overwrite the staged page. These storage fault and timing gates remain open. The generic NAND fixture is not a resolved physical S9 NAND identity.

The timed NAND patch now keeps pending page data separate from the live status buffer. Chip reset cancels the timer; controller reset retains the independently pending chip deadline. The independent Rust controller command `cargo test --manifest-path simulator/backends/qemu/axi-iic/Cargo.toml --lib --offline --locked` passed 16 tests. Its log is `work/simulator/qemu-s9/pl353-independent-tests-002.log`. The bare command without `--lib` also tries to build the no-std static library under the unwind test profile and fails; that failed attempt is preserved as `pl353-independent-tests-001.log`.

Review found two remaining file-backed storage concerns and sent them to the firmware agent: a spare-only write can underflow the inherited `NAND_PAGE_SIZE - off` expression, and a failed partial erase read must not proceed to write the all-ones fallback buffer over adjacent data. The in-memory qtest does not cover these file-backed failures. No storage fault gate is inferred from the passing Rust tests.


## Browser capture and host report

The capture now checks the observed metadata size against the exact object size and reads ELF magic from the verified CAS copy. This closes the review finding that a changing installed file could alter the byte budget or derived mode between reads. Capture 007 records original symlink targets and checked canonical targets separately. Package file bytes stay unchanged. The path set remains limited to the declared runtime roots; privileged helpers and absent optional ONNX support stay explicit exclusions from this support-tool closure.

The host browser report binds the captured probe and outer VM driver, exact Firefox/libxul bytes, full root/input objects, selected payload manifest and executed combined initramfs. It requires a successful untruncated outer result and the actual browser request/counter transition. Scope stays component-only. Review requested one further behavior check: require the first actual `/set-cookie` request and bind each request Host header to the reported origin. Otherwise that request could be removed from a saved trace without rejecting an otherwise complete summary. Root owns that fix and its tamper regression. No actual browser pass is inferred from source review.

Root independently checked native attempt 009 and its exact report. It passed actual adapted native services in separate client and worker namespaces. Root found one integer-bound bug in the generic uptime verifier: an integer above 2^53 can round down during conversion to f64. The verifier now checks the integer before conversion. Regression cases reject both raw and summary collisions. The attempt 009 values are small and do not trigger this defect; a new current-code run is still required and is active as native-010.

The current NAND timing patch closes the reviewed spare-write underflow and failed partial-read continuation paths. Raw program/erase now use exact byte ranges; both file-backed load forms return failure on a backend read error. Pending command data remains separate from status reads. Chip reset cancels it; controller reset preserves the chip deadline. These code findings are closed. Actual file-backed storage, spare-only writes and backend failures still need compiled integration evidence. This review does not upgrade the unresolved NAND fixture to an exact S9 part.


The browser host verifier now requires a unique initial `/set-cookie` request, its returned cookie on the first real counter fetch, and the exact declared Host header on all requests. The second profile must request the same origin without that cookie. This closes the trace omission/origin finding. Actual run 005 fixed the earlier rootfs failure and reached Firefox's real Marionette listener. It then hit the browser command deadline before the session produced DOM events. The run remains failed; its saved normal headless diagnostic and listener message are execution evidence, not a browser test pass.

The new NAND storage runner uses a complete private 264MiB data+spare image and compares every byte against independent expected changes. Its vectors cover spare columns, a data/spare crossing, the final page, an out-of-range write, completed writes through process restart and completed erase through a second restart. The child NAND owns the one BlockBackend attachment; controller lookup no longer attaches it twice. Review requested durable false assessment and failure records for errors before host preflight finishes, plus complete result/trace/probe identities in the assessment. Compiled executions remain pending. The exact physical NAND and power-loss gates are not implied by clean process restart.
