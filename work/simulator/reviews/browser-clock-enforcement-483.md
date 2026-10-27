# Selected browser clock enforcement

Status: source reviewed; affected tests, strict Clippy and release build passed.
Actual execution is pending. The [source and check identities](browser-clock-enforcement-483.json)
record the exact checked files.
This does not change the failed results from runs 469 or 479.

The live browser and retained-result verifier now derive the expected clock
policy from the selected locked process. `MD_SIM_S9_HOST_REALTIME_CLOCK=1`
requires the execution-host policy. The original fixed-epoch variable requires
its exact epoch. Conflicting variables and invalid values fail. A separate
policy file cannot select the policy for a selected execution.

For each declared policy, both origins must retain the matching policy and
two to sixteen HTTP uptime observations. Native `since + durationS` must fall
within the browser request and response interval, with the existing five-second
tolerance. Missing times, arithmetic overflow and excess skew fail. The same
function runs during live observation and retained verification.

Regression tests cover missing or changed policy, both interval boundaries,
one-millisecond excess skew on either side, reversed intervals, missing fields,
overflow, an insufficient sample count and conflicting selected policies.
The legacy local reference path still reads its bounded fixed-epoch input.
The three affected example suites passed 186 tests, with zero failures and
11 declared ignored helper entries. Strict Clippy and release builds passed
for the browser, worker-channel, remote-namespace, isolated-VM, QEMU-builder
and S9 boot helpers. The independent source review found no blocking issue.
Actual native clock
readback and comparison remain necessary before any execution pass.
