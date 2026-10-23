# Stratum V1 response review

The response fix is implemented and tested with the archived firmware.
This record does not establish complete protocol conformance.

The archived Braiins firmware warns when the local pool returns both
`result: false` and a non-null error for a rejected share. Run078 retains
this warning. The old pool emits that combination for unauthorized,
malformed and rejected submissions. It also assigns code23 to every
independent verifier rejection. The old tests and wire replay check
that same format. Their agreement does not establish protocol compliance.

The [official JSON-RPC 1.0 response contract](https://www.jsonrpc.org/specification_v1)
requires a null result when an error is present. This agrees with the
firmware warning. A normal false result with a null error is a different
case. For example, the archived server returns false when authorization is
denied. Do not replace all false results without checking the method.

The [isolated Slush server inspection](../../work/simulator/slush-mining-source-001/references.json)
binds commit `b2a24d7424784cada95010232cdb79cfed481da6` and archive SHA-256
`7de97fcbc2f3da0e5a65a7ad673c05a6bbe2f9cda9148726882f6164e12074f5`.
The source was read, not executed. `mining/service.py` raises
`SubmitException` for an unauthorized or unsubscribed submission.
`lib/template_registry.py` raises it for missing jobs, duplicate shares,
bad sizes, times outside the allowed range and work above the target.
The base exception and response transport belong to the separate Python
`stratum` dependency. Its exact source still needs to be resolved before
using it as evidence for numeric error codes.

The [isolated extension specification](../../work/simulator/slush-extensions-source-001/references.json)
binds commit `04679d0a85e7fb0f64b9995d7192c966aa03466d` and archive SHA-256
`44325c134f86f94b84102a43f38d4a313ccb52084e76d4d2d08022b954aeaab4`.
It is evidence for extension negotiation. It does not by itself establish
the baseline error-code mapping.

The [isolated SRI inspection](../../work/simulator/sri-stratum-source-002/references.json)
binds commit `31f80744907c6e21f4ae491ca6a0d86797bce8b0` and archive SHA-256
`42d1742ca392944fe1ca1462bb65bbbb2544877dd0e80e33c74a7294aa448233`.
The source was read, not executed. `sv1/src/methods/client_to_server.rs`
has SHA-256 `9b66a2ebe30d5a18306468e28067ae03fd74aa58e2eb4f4ee2130630ae71d081`
and35,519 bytes. Its `SubmitError` maps other errors to20, absent jobs to21,
duplicates to22, low difficulty to23, unauthorized workers to24 and missing
subscriptions to25. Its `Submit::respond` returns null for an error.
`sv1/src/json_rpc.rs` has SHA-256
`4594f87c11d814b8cc41736eaf5006f1854acf01edc4e1e1cdf641eff5c97ab6`
and10,010 bytes. It serializes the error as `[code, message, data]`.

New pool-input schema5 selects `json-rpc1-stratum-v1` responses. Schemas2–4
select `legacy-false-code23`. The report records the selected policy.
Both wire auditors require the policy to match the frozen input and use a
separate response table. They do not accept both forms for one identity.
Schema5 keeps schema4's distinct seed for each connection. False authorization
results still have a null error. A submitted worker name must match the
authorized worker.

Socket tests check actual responses for accepted, malformed, unauthorized,
unsubscribed, duplicate, stale, unknown-job, out-of-range and low-difficulty
submissions. The legacy contract has its own execution test. Mutation tests
reject the other contract, wrong codes, missing fields and invalid error data.
The new auditor also replays the retained run080 evidence under schema4.
[Fresh run083](../../work/simulator/qemu-s9/stratum-response-083/pool-result.json)
uses frozen pool-input schema5 and source102 helpers. The complete firmware
run exits0 in237,877ms without truncated traces. The actual wire trace has
nine low-difficulty errors with code23 and null results. The
[independent audit](../../work/simulator/qemu-s9/stratum-response-083/regtest-session-0.json)
verifies23,635 computed hashes,21 serial results and submissions, two fresh
jobs, a clean-job transition and a difficulty change. Both blocks pass Core
acceptance and restart persistence:
[first block](../../work/simulator/bitcoin-core/stratum-response083-0/block-acceptance.json)
and [second block](../../work/simulator/bitcoin-core/stratum-response083-1/block-acceptance.json).
This firmware run does not exercise every rejection code. The socket tests
cover those negative cases separately. Exact hardware and full release gates
remain open.
