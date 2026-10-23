# Shared worker preparation and retention review384

This is an independent source review by the private-channel agent. It covers
the parent agent's shared support VM preparation, host evidence import, and
private descriptor setup. It is not a firmware execution, mining, dashboard,
SSH, TLS, or external physical host result.

The reviewed host importer keeps the native worker result bytes. It verifies
the native process, run, token, and lock before it imports the separate pool
bundle. It checks the pool envelope, bounded raw logs, session files, and
support identities. The host sends its joined receipt only after the native
and pool evidence is saved and synchronized. Cleanup must acknowledge that
receipt. A native-only receipt does not authorize cleanup in this path.

The descriptor setup reserves source descriptors before remapping them. The
worker receives control on descriptor 3 and the support carrier on descriptor
4. The support VM receives its private Ethernet endpoint on descriptor 3.
The command clears inheritance on other descriptors and closes the parent's
reserved copies after launch. The default mode keeps its separate path.

Three findings were sent to the parent and are fixed in the reviewed source:

- The support proof now parses the actual prepared root manifest and outer
  process specification. It requires exactly one matching mount for the
  support runner, pool verifier, shared manifest, pool input, emulator,
  kernel, initramfs, and provider configuration.
- The final host receipt now checks the exact provider configuration,
  selected native process, support identity, and carrier policy. It rejects
  errors and timeout, requires traffic in both directions, and saves the
  provider receipt. The final assessment requires this check to pass.
- The child parent-death guard now returns `ECANCELED` on a changed parent
  PID. It no longer reports an unrelated prior system error.

No correctness finding remains in this source-review scope. The standalone
external pool audit is reviewed separately. It must also check the final
outer result and the provider receipt before its strict-success result can
serve as complete joined evidence. Actual shared firmware execution remains
the parent's next integration gate.

The [source manifest](worker-shared-independent-private-channel-384-inputs.json)
records the reviewed files. Checkpoint numbers identify evidence groups, not
target conformance levels.
