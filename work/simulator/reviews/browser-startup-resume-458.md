# Selected browser startup comparison 458

The user resumed milestone work on 26 October 2026. The repository was clean
at commit `b276dd7a`. Milestone 1.2 remains a separate passed adapted checkpoint.

Before execution, all 21 files in the frozen456 input manifest and all 20
ProcessSpec input mappings were read and matched their SHA-256 hashes and
sizes. The first read-only check used the wrong CAS directory; the corrected
path is `browser-startup-456-state/store/objects`. No input was changed.

This exact prepared command completed:

```sh
work/simulator/browser-startup-456-inputs/md-sim --state-dir work/simulator/browser-startup-456-state process execute work/simulator/browser-startup-456-inputs/process-458b.json --out work/simulator/network-vm/browser-startup-458b
```

The only declared difference from457b is the QEMU translation cache:256MiB
instead of64MiB. Both use two virtual CPUs,4GiB guest memory,6GiB outer address
space, the same421,270,225-byte image, backend328 and exact helper inputs.
Browser startup300seconds and NewSession270seconds remain unchanged. The
fixture checks actual two-profile startup and cleanup. It grants no firmware
or native dashboard result.

The prior457b baseline ran on23October, so host scheduling conditions are not
identical. Any timing comparison must state that limit. Failed453 native
browser startup and gateway shutdown acknowledgment remain failures.

Read-only source inspection found a separate scheduling issue:
`carrier_dashboard_client::exercise` waits for `worker-run.json` before it
starts both browser profiles. Thus support preparation consumes native
firmware lifetime. The browser helper already emits a readiness marker before
it waits for a selected publication. A bounded preparation barrier before
worker dispatch can separate these costs without changing native or browser
budgets. No such source change has been made at this checkpoint.

## Result

Comparison458b passed. The outer process exited0 without truncation in159,583ms.
Actual two-profile startup took49,563ms and48,876ms. Both profiles had empty
cookies and complete about:blank pages. Both owned cgroups were empty after
shutdown. The exact root and child tests each ran one test and passed.
[Assessment](../network-vm/browser-startup-458b/assessment.json) and
[decoded report](../network-vm/browser-startup-458b/startup-component.json) retain
the actual process, session, cleanup and input identities. No VM remains.

The result supports using256MiB for this browser support topology. It does not
prove that cache size was the sole cause of453 failure. Native browser and
gateway stop acknowledgment still require a fresh complete selected-run test.
