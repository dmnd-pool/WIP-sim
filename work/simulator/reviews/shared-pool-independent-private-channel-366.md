# Independent shared runner review366

Reviewer: private_channel agent. This is a read-only review of the shared-pool
runner path and serial spawn callback. It does not establish actual shared
firmware execution, pool readiness, valid mining, or dashboard completion.

Reviewed `examples/support/s9_shared_pool.rs`, the shared-pool branches in
`examples/s9_boot_bundle.rs`, and `src/serial_console.rs` in the firmware crate.
The runner takes descriptor3 before other runner work. It marks that endpoint
CLOEXEC. Only the selected QEMU child receives it. The parent copy is closed
immediately after spawn and before serial console reads. The existing private
NIC adapter is selected. Neither embedded pool starter runs in shared mode.
The manifest and pool input are captured in the process specification. Actual
support component execution and the final support image still need parent-run
identity and provenance evidence.

The observation interval starts before the owned QEMU spawn. Its duration is
the declared pool window plus five seconds. The code makes no pool-readiness
or mining-success assertion. The enclosing process retains its declared wall
budget. The serial callback executes after the owned child guard exists and
before console observation. Spawn failure does not invoke it.

Two review improvements are fixed. The parent-PID mismatch now returns
ECANCELED, because `getppid` does not set a failure errno. The descriptor test
now starts an actual unselected child while the source endpoint remains live.
That child confirms both descriptor3 and the source descriptor are absent.
The selected child then exchanges real bytes and the peer observes EOF.

The [compiled test log](../checks-2026-10-21-001/shared-pool366-review-fixed-tests.log)
passes three outer tests. Two ignored harness entries are explicitly executed
children. No unresolved correctness finding remains in this scope. Parent
integration must still run the actual shared native firmware path.
The [source and test identities](shared-pool-independent-private-channel-366-inputs.json)
identify this review checkpoint.
