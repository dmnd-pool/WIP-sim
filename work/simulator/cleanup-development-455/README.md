# Staged T17 duplicate cleanup extension

This directory contains source work only. The active utility under `crates/`
is unchanged. No build, test or deletion has been performed for this extension.
Do not run it while browser 453 is active. The parent must coordinate later
integration and checks. `base.json` pins the original utility copied here.

The staged entry point is `reclaim_qemu_inputs.rs`. Its new module and tests
are under `reclaim_qemu_inputs/`. On later integration, preserve that relative
layout under `crates/md-sim-firmware/examples/`.

The new scope is limited to direct stores named:

`work/simulator/bitaxe/t17-{core,readback}-oracle-NNN/inputs`

The number must have exactly three digits. Alias paths, symlink boundaries,
nested stores and other names are rejected. Targets must use the protected
`t17-core-oracle-274/inputs` canonical store. That store cannot be a target.
This does not add browser, access or other cleanup paths.

The validator reads bounded regular files through the existing same-open,
no-follow, nonblocking reader. The ProcessSpec must select the declared
packing or readback entry point. The ProcessResult must be terminal and
untruncated. Its stdout and stderr must equal the retained `evidence.tar`
and `stderr.log`. The tar parser never extracts files. It checks the exact
member names, type, count and expanded bounds, rejects duplicates and hidden
trailing data, and compares each decoded member with its adjacent retained
file. GNU sparse executable members are supported for readback 446.

The assessment must match the declared mode and source input identities.
Small source objects in the captured store and adjacent C/link/ABI files must
match those identities. Output fields, when present, must match exact tar
members. Readback requires its output and compiler-argument field contract.
A complete failed result can remain eligible; its failure is not changed.

Historical packing 272 lacks `oracle_executable` and `output_words` fields.
Only that checkpoint can use the legacy omission. That form is explicitly
recorded. Its executable and output identities come
from the exact tar bytes bound to ProcessResult.stdout. No missing fields
are added to the old assessment. Cleanup validation does not rerun the oracle
or establish a firmware acceptance result.

Canonical and target Store locks remain held during planning and deletion.
All oracle evidence is checked again after locking and before journaling.
The existing hash, size, inode and no-hardlink deletion path is reused. The
durable journal records exact canonical copy sources and the complete checked
oracle evidence. Only duplicate CAS objects can be removed. Adjacent archives,
source files, executable, output, logs, manifests, result and evidence tar are
retained. Original downloaded and extracted firmware stays unchanged.

Staged tests cover both modes, sparse tar expansion, legacy packing, complete
failure retention, exact path scope, substituted evidence and input modes,
links/FIFOs/oversize files, tar members/trailers, active locks and journal byte
substitution. A separate explicit real CLI test uses scratch stores. It must
refuse an active owner, changed evidence and a protected canonical target;
then remove one exact duplicate, preserve the unique objects and all evidence,
and restore the duplicate through a normal copy with one link.

[The declaration-only intersection](declaration-potential.json) lists 43 common
objects/541,161,169 bytes for 272, 42/541,159,543 bytes for 446 and
43/541,161,169 bytes for 447 against protected 274. The total is 1,623,481,881
bytes (about 1.51 GiB). Only the small ProcessSpec files were read and hashed.
Candidate object contents have not been rehashed. These are potential bytes,
not verified duplicates or a reclamation result.

After the browser run ends and integration is approved, run the affected
ordinary tests, build the standalone utility, explicitly run the real CLI
test with `MD_SIM_RECLAIM_EXECUTABLE`, and run strict Clippy/formatting.
Recheck the actual finished272/446/447 evidence read-only. Independent review
must close before any exact duplicate cleanup. Record fresh journals and
before/after hashes. The planned size is not a verified reclamation total.
