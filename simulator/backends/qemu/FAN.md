# AM1 fan development contract

This component implements fan registers and motor motion. It is not a
calibrated physical fan or proof of the selected FPGA image.

The archived Braiins source has SHA-256
`dc5f58721f596d2fc36b50d5049461264986ac339c4b4ff4d383749b98f3a363`.
Its `open/hw/zynq-io-am1-s9/fpga-io/map.svd` has SHA-256
`fddc9e903f82b0d4f4ccf64363819a327142d242bf55d270b9cc56a154f89333`.
It specifies four read-only RPS registers at offsets0,4,8,12 and an
eight-bit PWM register at16, with zero reset values. RPS means revolutions
per second. The driver `open/bosminer/bosminer-am1-s9/src/fan.rs`,
SHA-256 `67b6e02eab74cebb085a355955d00466838a6ce991a01f4404abd1aa97300752`,
converts RPS to RPM by multiplying by60. It writes PWM in0..100.

The DCENT_OS archive has SHA-256
`5501e53fc05648ce4a03da2b47ba799c0fe1e0c6dd4e358db5a18504d419f25c`.
Its `DCENT_OS_Antminer/dcentrald/dcentrald-hal/src/fan.rs` has SHA-256
`79c8819e086c18b92807a7ba8cf53c0d8bea05a1e2e41f68b2da82b7aaf474e7`.
The isolated extraction is `work/simulator/dcent-fan-source-001/`.
This source specifies a second PWM at20 and two physical AM1 fan positions.
Only the second position has a tachometer connection, at offset4.
The source states PWM0..100. An older
[architecture page](https://github.com/DCentralTech/DCENT_OS/blob/main/DCENT_OS_Antminer/docs/DCENTRALD_ARCHITECTURE.md)
instead states0..127. The implementation follows the pinned source. It does
not treat that page as calibration or exact board authority.

The model has no connected motors by default. Its explicit reference-motors
fixture supplies two motors, PWM channels0/1, with the declared tach wiring.
Each has minimum900RPM, maximum6000RPM, two pulses per revolution and a
linear acceleration or deceleration limit of12000RPM per second. These are
uncalibrated development parameters. They do not identify an archived fan.
Power is initially on. Duty interpolates linearly between minimum and maximum.
Power loss causes coast-down to zero. A locked rotor stops immediately.

Tach readings count integrated motion over one-second windows. The sampling
period, rounding, analogue response, second PWM register width and reset
behavior are explicit model assumptions. The source does not establish them.
Integer integration retains fractional pulse area. Irregular backend updates
must give the same result as one continuous interval. No read operation
increments a counter or assigns a successful RPM value.

The bounded Rust advance operation accepts at most60seconds per call. The
backend uses a separate elapsed-time operation. This operation divides a late
callback into bounded calls and retains all elapsed time and sampling phase.
It does not change the guest clock or the motor parameters. A host scheduling
pause can delay a callback beyond60seconds. Such a pause must not stop the
fan model. The backend still rejects a clock that moves backwards.

The [delayed-callback test on backend067](../../../work/simulator/qemu-s9/fan-pause-delayed-001/observation.json)
reproduces the old failure. It stops the owned emulator process for65seconds.
The callback then receives65000129199nanoseconds and fails the old60second
bound. The [new regression](../../../work/simulator/qemu-s9/fan-pause-delayed-003/assessment.json)
passes on backend069. Actual fan readback is100RPS before and after the
65000millisecond host pause. The test executes project-owned CPU code;
it is not archived firmware or a physical fan calibration test.

```sh
target/release/examples/s9_fan_pause_probe \
  /tmp/md-sim-qemu-backend-069-ubsan \
  work/simulator/qemu-s9/fan-pause-delayed-003 65
```

Use a new output directory. Run002 retains a test-assessment failure:
the fan was correct, but the test rejected the two expected warnings from
disconnected Ethernet ports. The current test permits only those exact
warning lines. Other QEMU diagnostics and all failed fan accesses still fail.
This reproduced defect does not prove the cause of original firmware run035,
which lacks the clock details needed for that conclusion.

Controller reset clears command and sample registers. External motor motion,
power and stall state survive it. Unsupported offsets fail; PWM byte values
above100 stop the backend. Values are not silently clamped. Higher byte lanes
do not modify the eight-bit PWM. Writes to read-only tach registers do nothing.
There is no interrupt. MMIO is atomic. The model does not generate a physical
25kHz output waveform or model clock-domain crossings. It is unmigratable.

An optional `input-plan` device property schedules external power and rotor-lock
inputs. The Rust model accepts at most32 ordered records. Each record has an
absolute `at_ns`, a connected motor index, and optional `powered` and `stalled`
Boolean values. At least one value must be set. Times must be positive and
within one hour. Equal-time records use increasing motor index. A controller
reset retains the input timeline and does not replay past faults. The preceding
motion interval and a tachometer sample at the same time finish before an input
change. A delayed callback still applies each change at its exact virtual time.

The boot helper accepts `fan-plan=JSON_FILE` with `fan-fixture`. It saves the
original plan bytes and hash, the parsed records, and the backend capability.
The C binding only passes inputs and records applied events. Rust advances
motor motion and tachometer counts. A planned stall does not set an RPM reply.
Component tests cover analytic motion, split time steps, reset, validation and
late callbacks. Native-firmware fault tests for this new input path are pending.
The [compiled timed-input test](../../../work/simulator/qemu-s9/backend188-fan-plan-qtest-188/assessment.json)
passes all21 protocol responses on the UBSAN build. It checks the exact four
applied input events and the analytic tachometer windows across stall, release,
power loss and power restoration. The existing connected and empty fan tests
also pass all25 responses each on that build.

The current MMIO trace includes the QEMU virtual time used to advance the motor
state before each access. This permits checks of a firmware command and later
tachometer read against the same time base as sensor inputs. Older traces
without this field do not establish a timed fan-response result.

The [native105°C run](../../../work/simulator/qemu-s9/temperature-hot-184/cooling-audit-184.json)
passes the timed cooling audit with backend183. Both PWM commands rise from1%
to100% about0.994s after the first complete hot sensor read. The connected
tachometer later reports100RPS, or6000RPM. After healthy65°C reads return,
both commands fall to88% about1.546s later. Later tachometer reads show reduced
speed. The audit checks the exact model binding, fixture parameters, executed
fixture option, sensor queries, command order and sustained full duty during
the hot interval. It reports measured intervals without a physical deadline
or calibrated thermal-plant claim.

The [connected](../../../work/simulator/qemu-s9/backend183-fan-qtest-184/assessment.json)
and [disconnected](../../../work/simulator/qemu-s9/backend183-fan-empty-qtest-184/assessment.json)
compiled controller checks also pass on the UBSAN build. Each requires all25
protocol responses, including timed motion, stall, power loss and reset.

The separate `md-sim-s9-fan-development` machine maps this contract at
`0x42800000`. It does not change the previous machine profiles. Use the explicit
`fan-fixture` diagnostic option to attach the reference motors. A successful
fan check under that option does not establish exact-target cooling or thermal
protection conformance.

Original firmware run073 exposed a different interface gap. After a native
miner stop, the archived safety monitor writes100 to offsets20 and24. The
two-PWM reference contract implements offsets16 and20. Offset24 stops the
backend with an explicit unsupported-access error. The
[extracted monitor](../../../work/simulator/native-s9/original-bosminer-monitor-031/member.json)
has SHA-256 `5dd1adc64ed54033c2b9ed6c1998de31c744d133fbb4bfc77f11bef30b3f2a1f`.
Its loop starts at1 and ends at2. This is evidence of the firmware access,
not proof of the selected2022 FPGA map or a physical firmware defect.

The optional diagnostic flag `adapt-monitor-fan-source=EXTRACTED_MEMBER`
maps that loop to0 through1 for the declared reference model. Rust checks
the exact original member, saves a one-line patch and adapted output, and
records both hashes. The guest checks the source and output hashes before
it starts the native miner. Run074 retains the
[adaptation manifest](../../../work/simulator/qemu-s9/sd2022-private-pool-074/monitor-fan-adaptation.json).
The monitor still runs its safety checks and protective fan writes. This
is a patched native-script path. It does not pass the original-script,
2022 FPGA or physical thermal-protection gates. The complete update and
restart test remains in progress.
