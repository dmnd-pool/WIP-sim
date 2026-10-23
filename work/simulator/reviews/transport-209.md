# Author review: process accounting and interrupted artifact transfer

This is an author review. It does not close the independent review gate.
All three agents remain blocked by the service quota until 25 October.
The accompanying input manifest pins the reviewed files and actual reports.

Source208 uses wait4 for the owned emulator PID. It retains the exit status
and resource record after reaping. Later checks use that retained status.
Cleanup does not kill a reaped process. The existing cumulative child totals
remain separately labelled. Tests cover exit7, SIGKILL, repeated status reads,
pending serial bytes, native restart observation and a separate exiting child.
The new CPU profile is explicit. It keeps4800 seconds of wall time and permits
9590 CPU seconds. The previous profile keeps4790 CPU seconds. This is a
diagnostic resource choice. The cause of local200's SIGKILL is not established.
The local208 firmware run is still active. Its outcome is not assumed here.

Source209 adds a test relay that forwards actual worker artifact requests.
It permits only Begin, Chunk and Finish, one declared object, bounded frames,
eight requests and the private VM's fixed worker socket. It withholds the
selected successful reply and exits. The client receives an SSH error.
The real worker response remains in the relay's stderr evidence. No worker
write, saved offset or hash is supplied by an imitation service.

The actual test uses two fresh270336-byte objects. For a lost chunk reply,
the saved offset is262144. For a lost publication reply, it is270336.
Neither failed mutation triggers an automatic reconnect or blind replay.
A new connection queries the real destination state. The production transfer
function resumes from that offset and verifies the complete destination hash.
The test retrieves each object and compares all bytes with its fresh input.
Worker build, policy and tool identities agree before and after recovery.

The host reporter requires both cases and checks the original error receipt,
worker response, object identity, saved offset, final verification and retrieved
identity. Regression mutations remove or change each of those requirements.
The actual SSH209 test and matching host report pass in separate client and
worker namespaces in one disconnected local VM. The existing selected live-run
reconnect, cancellation, result cleanup and authenticated TLS tests also pass.
This establishes component transport behavior. It is not an external physical
host, firmware mining or native dashboard result.

All535 Rust tests in96 binaries pass. Formatting, strict Clippy and release
builds pass. Source209 matches all379 captured source files. No C was added.
The two initial compile errors were fixed before these checks and execution.
The README now contains the actual209 commands. Source210 records that
documentation change. A build and test with an empty Cargo home and new output
directory is active. Its result is not assumed here.

The interrupted NAND207 cycle also completed. It retains the damaged first
disk as an observation, then passes two strict native recovery inspections.
All three firmware runs pass independent mining and rate checks. Six blocks
pass separate Core acceptance and restart tests. The narrower software
interruption scope and the earlier failed206 cycle remain explicit.

Required open work includes exact hardware models and target resolution,
the original-board gate, native local and remote dashboard completion, every
applicable exact target, calibration and the strict complete matrix.

The clean210 check has now completed. All535 tests in96 binaries pass.
All58 release executables match the frozen209 copies byte for byte. The source
manifest pins3611 source and vendor inputs and passes its verify command.
See reports/clean-build-210.json and reports/source-210.json. The declared
prerequisite is the installed Rust/Cargo1.94.1 toolchain. This is not a claim
that the complete firmware package installs from a clean machine.
