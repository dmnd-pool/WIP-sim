# Owned SSH service guard 414

The coordinator now exposes `OwnedSshChild::spawn_service`. It installs the
existing private process group, parent-death signal, no-new-privileges rule, and
resource limits. The raw child constructor remains private. CPU is bounded to
1 through 3,600 seconds. The per-file output limit is 1 byte through 64 MiB.
Existing transport and tunnel callers retain their previous 64 MiB limit.

`exited` observes the leader without reaping it. `finish` stops the owned process
group before reaping the leader. This prevents a recycled process-group ID from
being signalled and stops descendants that outlive the SSH leader. Repeated
cleanup returns the recorded status. `close_stdin` closes the guard's pipe.
Callers must still enforce a wall deadline.

The focused coordinator tests in `channel414-ssh-guard-tests.log` pass all nine
tests. The new real-process regression checks stdin EOF, inherited hard and soft
CPU and file limits, actual output-limit termination, repeated cleanup, and
invalid-limit refusal before spawn. The existing actual inherited-pipe child
regression also passes. Strict coordinator library and test lint passes in
`channel414-ssh-guard-clippy.log`.

The access author independently inspected the API and requested two fixes. The
public constructor now rejects unlimited resource values. A changed parent PID
returns `ECANCELED`; it no longer relies on an unset `errno`. Both fixes precede
the final tests. The access integration owns its additional lingering-descendant
regression and call-site report. This component change is not a firmware or
dashboard execution result.

The adjacent manifest pins the three changed source files and both final logs.
