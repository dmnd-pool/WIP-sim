# Independent browser resource selection review 430

Reviewer: `channel_review`. Author: the parent.

No source finding remains in the separation of the new browser support budget.
This review does not establish that the budget is sufficient or calibrated.
The explicit browser run must still execute its full tests.

The outer ProcessSpec captures `MD_SIM_CARRIER_BROWSER_SUPPORT=1` only when the
original caller supplies the browser payload. The child rejects other values
and rejects browser mode outside shared firmware execution. This selection is
therefore part of the outer process identity. The actual QEMU argument vector
and browser flag are retained; assessment also compares the flag to the
original caller's selection.

Only browser mode selects 4 GiB guest RAM, two support TCG CPUs, and ramfs. Its
outer limits are 6 GiB address space, 1,200 seconds wall time, and 2,000 CPU
seconds. The new confinement check retains the existing namespace, identity,
capability, network, filesystem, and file-limit checks.

Basic firmware mode keeps 1,536 MiB guest RAM and outer limits of 4 GiB,
900 seconds wall time, and 850 CPU seconds. The separate native job's captured
process specification, mining cap, bus and boot timing, and observation window
are not rewritten by this selector. This is a support-browser resource policy,
not a claim that either mode passes firmware or mining acceptance.
