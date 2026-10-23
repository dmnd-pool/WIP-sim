# Serial device state review

This is an author review. The independent agents remain unavailable because
of the account limit. This does not close the independent review gate.

The serial component saves its current integer time, bit period, capacity,
input and output levels, partial receive and transmit frames, pending bytes,
and both event deadlines. State version1 is explicit. Deserialization rejects
unknown fields. Restore checks all bounds and state relationships before it
constructs an active device. No restore operation emits a byte or pin edge.

The replay test covers two bit periods and four different byte patterns. It
restores through JSON at each quarter bit, across full duplex traffic and
three queued output bytes. The original and restored events must agree, and
received bytes and their completion times must match independent expectations.
Existing tests compare all256 received bytes and transmitted bit waveforms.
Break, reset, changed baud, rejected versions and corrupt state remain covered.

Review found that the old enqueue check reserved time only for the first bit.
A later time overflow could strand a partial frame. The new check reserves
the complete queued transmission before adding bytes. Receive start also
checks the time of the final sample. Tests use times near u64::MAX and require
rejection without queue changes. Restore rejects saved frames whose later
bits cannot fit in the time range, even when their first deadline fits.

The scoped256 test passes all six cases. Strict workspace lint, formatting,
and all561 tests across99 binaries pass. The compiled UBSan backend regression
passes859 responses. The signed update cycle256 passes its provision boot and
two Core block tests, then fails in the native update probe. The probe omitted
hostname from its first GraphQL query, although the updated telemetry validator
requires it. This is a test-client defect. The failed cycle remains failed.
Existing retained firmware executions keep their earlier binaries and hashes.

The state is local to the serial device. A full chain snapshot also needs
chip registers, packet decoders, work and result queues, power and reset pins,
shared hash budgets and the connected controller. A full board snapshot must
include CPU and network state. Neither capability is claimed by this change.
