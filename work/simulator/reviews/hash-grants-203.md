# Hash grant review

This is an author review. Independent review is blocked by the agent service
quota. It does not close the independent review or release gate.

The shared BM1387 budget now returns the actual monotonic timestamp used by
the rate limiter. The model emits one numbered grant with each computed
SHA-256d result. Device resets retain the run-owned budget. Other device
events have zero grant fields. The C ABI has40-byte events, with grant at
offset24 and time at32.

The auditor checks the declared cap against the executed machine property.
It independently checks the complete trace, every hash against its firmware
packet and filter, unique consecutive grants across all chains, first and
later intervals, and the total process wall bound. It sorts grants because
per-chain trace delivery may differ from admission order. It rejects missing,
repeated, reversed or too-close grants. No hash counter is advanced without
a computed result. Clock sampling occurs before the computation, not when
the C wrapper drains its trace.

Legacy hash audits retain their limited scope. The separate rates command
fails when timestamps are absent or the trace is truncated. Native restart
suites now require that command before their independent Core tests.

Evidence: native-low-rate10-grants-201, helpers/model201/backend201,
rate-audit202 and regtest-audit202-session0. All1331grants obey cap10, with
minimum114487347ns. Twelve submissions are independently verified. Two Core
imports move height0 to1 and retain that tip on restart. The old194 trace
fails the new rates command because it has no timestamps.

Limits: timestamps come from the actual model's monotonic clock. They prove
its software admission limit, not physical hashrate or calibrated hardware.
Pool prefix correlation leaves47unmatched computations explicit in201; none
is counted as a verified submission without its independent full header.
Exact hardware, original-board, complete matrix and independent review gates
remain open. The new203restart integration test is pending.
