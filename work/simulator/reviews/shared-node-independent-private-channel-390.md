# Shared Core probe independent review390

The private-channel agent reviewed the explicit `shared` mode in
`node_prerequisite_probe.rs`. No correctness finding remains in this delta.
This review does not establish actual shared firmware mining or Core block
acceptance. Those runs must produce their own evidence.

The command requires the shared evidence mode to match the audit's external
pool provenance. It reruns the selected session through the shared auditor
before it imports a candidate. The fresh replay must match the saved audit
and selected block. The default, offline, and source-fixture modes remain
separate. Core's existing isolated three-phase import and restart checks are
unchanged.

The output keeps the shared bundle, joined receipt, support proof, provider
receipt, pool logs, and manifest with exact hashes. It also binds the large
outer process result at its original path. That source path remains required
for a complete replay; the copied output does not claim to be self-contained.

Review found that ordinary file copy could exceed the declared size if the
source changed after its hash check. The fixed helper opens the source once
without following symlinks, requires a regular file with the exact extent,
creates a new destination, and copies at most the expected size plus one
byte. It then requires the exact copied size and hash. Both shared and native
evidence copies use this helper.

The [ten focused tests](../checks-2026-10-21-001/shared-node390-tests.log) pass.
They include explicit mode selection, changed provenance, changed outer
identity, source growth, symlink rejection, and preservation of an existing
destination. [Strict focused Clippy](../checks-2026-10-21-001/shared-node390-clippy.log)
also passes. The [input manifest](shared-node-independent-private-channel-390-inputs.json)
records the exact reviewed source and logs.
