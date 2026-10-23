# Independent selected SPL inspector review, cycle 454

The selected static inspector passes this review. No unresolved correctness or scope finding remains. The [exact input and result record](spl-inspector-independent-454.json) pins the source, binary, logs and actual isolated result.

The Rust inspector ran in the existing preparation isolation. It exited zero in 31 ms without truncated output or stderr. Its stdout is exactly the retained contract report. All six distinct captured objects match their expected identities. The report's inspector and source identities match the actual binary and final source files. The limits remain explicit: 15 seconds wall time, 10 CPU seconds, 256 MiB address space and 1 MiB scratch storage.

Independent decoding of the original bytes confirms all 654 emitted records across 15 tables and three revision branches. Each pointer, source offset, parent offset, header, argument, operation, terminator and encoded hash matches. Record counts are `[73,26,14,82,22]`, `[73,26,14,84,22]` and `[73,26,14,83,22]`. The first and fourth table roles retain neutral names.

The pinned ARM instructions support the extracted semantics. A masked poll succeeds when any selected bit is set. Its read budget is 100,000,001 and its exhausted return value is 2. The delay calculation preserves the low 32-bit multiplication before the 64-bit product and right shift by 39. Argument 1 gives 333,333 ticks. These facts are software requests, not measured physical delays. The report does not select a physical silicon revision or claim execution of the timeout path.

All eight focused tests passed, including the exact retained-input test. Formatting, strict Clippy and the release build passed. The initial relative-path test failure and the minimum-Rust-version Clippy failure remain saved. The final alignment check uses the equivalent `start & 3 == 0`; the final tests cover that source.

This adds static inspection of unchanged archived bytes. It adds no CPU emulator or duplicate clock model. It does not execute ARM instructions or grant firmware boot, mining, original-board, FPGA-equivalence or physical-calibration acceptance. Original-code delayed-ready and timeout tests remain separate work.
