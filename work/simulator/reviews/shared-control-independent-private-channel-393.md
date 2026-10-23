# Shared control reader follow-up393

Review after freeze390 found that root still used ordinary file opens for
three control reads below the broker-owned directory. A substituted FIFO
could block before the length check. A symlink could redirect the read.

The current source closes this finding. All three reads now use the same
opened descriptor with `O_NOFOLLOW` and `O_NONBLOCK`. They require a regular
file owned by UID 1000, one link, no group or other write access, and a bounded
extent. The read also enforces its byte limit. Real-file tests reject the
wrong owner, changed mode, multiple links, symlinks, oversized content, and a
FIFO without a writer.

The new progress records are diagnostic only. They inspect actual child
status, bounded complete audit events, and artifact sizes. Their readers also
reject nonregular files without blocking. They do not grant a completion gate.

[Focused tests](../channel393-shared-tests.log) pass 82 tests, with two invoked
child helpers listed as ignored entries. [Strict Clippy](../channel393-shared-clippy.log)
passes. No correctness finding remains in this follow-up source scope.
Frozen390 and the actual run391 remain unchanged. They do not prove this fix.
