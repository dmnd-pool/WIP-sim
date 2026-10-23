# Selected browser startup diagnosis456

The complete browser gate remains failed. Native run453 passed the HTTP and TLS API suites, but its first Firefox NewSession did not finish within the existing300-second startup budget. Its secondary TLS shutdown acknowledgement also failed. Neither failure is waived.

The reconstructed453 failure evidence is in [browser-start-failure-453](browser-start-failure-453/provenance.json). The independent comparison is [browser-startup-comparison-independent-453.json](browser-startup-comparison-independent-453.json).

All33 captured Firefox files and all107 actual recommended preferences match the passing322 run. The selected profile adds only the declared client-certificate selection preference to the common base preferences. The SWGL framebuffer warning occurs in both passing284/322 logs and failed453, so it does not establish this failure's cause. Passing322 profile startup took51,392ms and49,069ms. Failed453 took about91seconds to issue NewSession after Marionette became enabled, then exceeded the total startup deadline. The sampled Firefox main thread was running. The failure snapshot reported2,187,164KiB available guest memory; this is not evidence of an out-of-memory kill.

Actual322 support arguments select4vCPUs and a256MiB TCG translation cache. Actual453 selects2vCPUs and64MiB. Firefox payload identities match, but helper identity, certificate preparation and the enclosing transport topology differ. Translation-cache pressure is a test hypothesis, not an established cause.

The explicit `--selected-startup` mode uses the production selected-client credential import, UID1001 privilege drop, two fresh profiles, Firefox arguments and cgroups. It checks both real NewSession responses, empty cookies, complete about:blank documents and empty owned browser cgroups after close. It sets firmware and dashboard gates to false. A VM-only test preserves the production client role command's mounts and namespaces, replacing only its final entry with the exact ignored test child. There is no miner, dashboard or SSH service in this component fixture.

The captured inputs and both specifications are pinned in [manifest456](../browser-startup-456-inputs/manifest.json). The same421,270,225-byte image and pinned328 backend are used for serial2vCPU comparisons. Guest memory stays4GiB; the outer address limit stays6GiB. Firefox startup300seconds, NewSession270seconds, CPU300seconds, per-leaf resident memory768MiB and task256 limits stay unchanged. Native305/360-second limits are not part of this fixture and are not changed.

Focused456 checks passed:101 namespace tests and10 browser tests; strict Clippy; release browser-helper build; scoped formatting. Eight namespace entries are explicitly ignored in ordinary checks, including the new two VM entries. Their existence is not execution evidence.

The initial457 launcher failed before boot because its generic argv contained `-sandbox`, which backend328 does not support. That exact20ms failure remains at [457 result](../network-vm/browser-startup-457/result.json). Revised457b/458b remove only that unavailable option and match the production backend launcher. Runtime namespaces, read-only inputs and resource limits remain enabled. Baseline457b is active at this checkpoint. No cache policy fix or browser pass is claimed.


The457b baseline completed before the user-requested stop. Its actual two selected-client Firefox sessions passed startup, empty-cookie/about:blank, certificate import and cleanup checks. Startup took202,007ms and202,503ms. The outer process exited0 without trace truncation after467,881ms. The explicit root test and its role child each ran exactly one test and passed. See [assessment457b](../network-vm/browser-startup-457b/assessment.json) and [full component report](../network-vm/browser-startup-457b/startup-component.json).

The two startup intervals total404,510ms. This isolated baseline is longer than the native305-second observation window. It does not reproduce the concurrent453 timeout exactly and does not establish which support setting causes the delay. No256MiB-cache variant was launched. No production cache or deadline changed. No command or VM is active. Cleanup455 remains staged, unbuilt and unexecuted.

After the user resumes, the prepared comparison command is:

```sh
work/simulator/browser-startup-456-inputs/md-sim --state-dir work/simulator/browser-startup-456-state process execute work/simulator/browser-startup-456-inputs/process-458b.json --out work/simulator/network-vm/browser-startup-458b
```

This command was not run. Its result needs actual one-test execution, both startup responses and complete cleanup, then independent review. It cannot grant native browser or full simulator acceptance.
