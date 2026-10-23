# Store quota implementation checkpoint

The content store now enforces aggregate logical bytes and distinct object slots.
Incomplete transfers count toward both limits. Empty transfers consume a slot.
`Store::open` keeps its existing object-size argument and adds default limits of
16 GiB and 4,096 object slots. `Store::open_with_limits` accepts an explicit
`StoreLimits` policy. The worker records its policy separately in each run lock.

The store holds an exclusive file lock for its lifetime. A mutex serializes
mutations through shared handles. Every mutation checks actual disk state.
This includes interrupted writes, restart, retry, abort and publication.
An interrupted publication can leave two local names for one inode. The store
charges that data once and checks that both names identify the same object.
External hard links, symbolic links, unexpected names and changed policies fail
closed. Published objects are retained and never overwritten by a transfer.

The initial policy uses a bounded reserved `limits.pending.json` file.
Publication uses a non-overwriting hard link and directory synchronization.
Restart can finish a matching complete pending policy, rewrite an incomplete
uncommitted serialization, or remove the second name after publication.
A valid policy with different limits, a separate pending file beside a committed
policy, or corruption of a committed policy remains an error.

Checks run from the repository root:

- `cargo fmt -p md-sim-runtime`: passed.
- `cargo test -p md-sim-runtime --test store_quota --locked --offline`: six tests passed.
- `cargo clippy -p md-sim-runtime --all-targets --locked --offline -- -D warnings`: passed.

The tests cover concurrent writes, exclusive handles, partial transfers, repeated
begin and retry, object slots, abort, restart, interrupted publication, policy
initialization loss, conflicting policies and preservation of unrelated files.
The catalogue agent reviewed the initial implementation and the incremental
policy-recovery fix. It reran all six tests and found no remaining scoped defect.
The independent runtime review then ran all 20 tests successfully. It also
checked aggregate active-run reservations, parent-loss cleanup and interrupted
run recovery. Two recovery findings were fixed and verified. See `review.md`.

Limits count logical file bytes. They do not claim filesystem block quotas or
aggregate control groups. Directory metadata is bounded by the object count.
These are runtime component checks. They do not prove firmware execution,
remote transport, native dashboards or complete simulator coverage.

The shared runtime changed. Previous v10 evidence is preserved as historical
evidence. Current canonical binaries and affected source execution suites must
be rebuilt and run again before a final source-state claim.

The store and runtime review are closed for this implemented scope. The runtime
source is ready for a canonical rebuild and affected firmware execution checks.
