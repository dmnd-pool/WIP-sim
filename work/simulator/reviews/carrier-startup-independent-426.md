# Independent carrier startup review 426

Reviewer: `channel_review`. Author: `private_channel`.

The UID fix and actual startup regression pass this review. No unresolved source
finding remains. The source and execution identities are pinned in the
[input check](carrier-startup-independent-426-inputs.json).

Run 421 failed before service startup because Bubblewrap requires a user
namespace for its `--uid` option. The fix keeps the existing real user namespace.
Bubblewrap creates the mount and process isolation as root and retains only
SETUID, SETGID, and SETPCAP for the trusted Rust entry. That entry drops all
bounding capabilities, supplementary groups, and all real, effective, and saved
UID and GID slots to 1000. It explicitly clears capability sets, enables
no-new-privileges, and disables dumpability before reading launch metadata or
connecting to the broker. All five capability sets must read back as zero.

The first actual regression, 424, found a second error. Root with these reduced
capabilities could not enter the UID1000-owned mode0700 access directory.
The fixed launcher starts in `/`; the Rust entry enters `/work/access` after
dropping its identity. Directory permissions remain private. Failure 424 and
its original stderr remain retained.

The actual private VM regression 426 ran the production Supervisor and captured
production child. One test passed with zero ignored tests in the VM. Eleven
broker connections measured UID and GID 1000 with SO_PEERCRED. The child records
all six real/effective/saved IDs as 1000, no groups, five zero capability sets,
NoNewPrivs1, the original user mapping, and `/work/access` as its current
directory. The test checks distinct mount, PID, IPC, and UTS namespaces, the same
network and user namespaces, excluded private files, and exclusion of a real
non-CLOEXEC parent descriptor. It reaches the real listener readiness record and
stops it with no error, closed listeners, and removed Unix socket.

The broker replies in this regression are explicitly scripted. No HTTP
connections were accepted. This tests the real launcher, credentials, isolation,
and service lifecycle. It is not a firmware, mining, HTTP, TLS, or external-host
acceptance test.

The production helper has SHA-256
`44b8bb8ded0aca613e218f5396015f02f71436e058a1396f0e03f57b5dd3c0ea`.
Its exact captured root entry and reported guest identity agree. Production
source still matches the captured source hash. Final workspace formatting
wrapped one test assertion; the saved captured test source and current source
have identical whitespace-separated tokens. Both identities are retained.
The 90 host component tests pass; three VM helper entries are ignored there.
Production and test-target strict lint and formatting pass in logs 427.

The separate [421 retention review](failed-access-retention-independent-421.md)
confirms that the failed native access run kept its exact native, pool, and
access evidence before acknowledgment and cleanup. Its access result remains
failed. A fresh full firmware run is still required after the startup fix.
