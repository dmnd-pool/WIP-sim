# Independent input and deadline review 410

Reviewer: `exact_target_gap`. Author: `channel_review`.

No unresolved correctness finding remains in these two helper modules.
This is a component review. It does not establish native dashboard access,
an external host test, an exact physical miner target, or full completion.

`carrier_inputs.rs` has SHA-256
`5e6a7c64c3bffe730d92de859a97e5b674371fbb6c864d3d574bbf982b571c5b`.
I checked the canonical object parser, each actual Get request and reply,
the exact offset and byte count, the complete hash, and the explicit EOF check.
The helper opens its private destination directory once. It creates a private
staging file through that descriptor and publishes without replacing a path.
The 4 MiB cap applies before transfer. Failure removes the owned staging file.
The actual worker socket-pair test, changed-reply tests, and strict lint pass.

`worker_timeout.rs` has SHA-256
`83b14b1ec0bdd95534097c06992cf7965265a009295d7481ab89f77cee9bffb3`.
I checked bounded serialization, nonblocking Unix connect, full-backlog errors,
partial reads and writes, the existing response decoder, and the final deadline
check. A request has one absolute deadline. Partial replies cannot extend it.
The implementation does not change the old worker request functions.

The host socket-pair deadline regression passes. The private VM 406 also ran
the three explicit listener tests. It checked complete success and error
envelopes, a slow partial reply, and an actual full Unix listener backlog.
Its guest log records three passes and actual scenario exit code zero.
The VM had only loopback. Its log SHA-256 is
`14f70589505c75ab44fc3f934239d80befc1c1603fbe9260408d83a05c4c0d08`.

Checked logs, relative to `work/simulator/`:

| Log | SHA-256 |
| --- | --- |
| `channel403-carrier-input-tests.log` | `0c46311fc9b804face2e681dac7493e0ddeb3fde2cdd55c5f3fe8637688ff327` |
| `channel403-carrier-input-clippy.log` | `cef0f48bfa6e06b2babb833562410181c0dda78741c5882ab721ecec918dd0c8` |
| `channel405-worker-timeout-tests.log` | `8182c57efefb396f2123f1c6f5a751bf669d50512bd7b5b09111ea57de19c045` |
| `channel405-worker-timeout-clippy.log` | `f1672422df13d91109910756de6d940852439526bc09fa5b1ced1bc626f44233` |

The access adapter uses this request API with a quarter of the captured lease
duration for each authorization request. Metadata transfer uses a five-second
request budget before exposure. The caller must still authenticate its socket,
bound the total operation, and preserve the selected run identity.
