# Original Boser service probe

Run 036 executes the archived 2022 S9 firmware. Its native Boser service
answers HTTP requests from the separate Linux pool VM. No copied page or
replacement API serves these requests. Later run057 also passes native
Firefox login, live board and pool data over WebSockets, and isolation of
authenticated sessions between two fresh profiles. Run059 passes backup
download and invalid upload rejection. The later installation and damaged-image
tests are described below. Complete update/recovery coverage, SSH and TLS remain
open for this execution path.

The original SD image has SHA-256
`97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef`.
Its FIT image matches the separately extracted update FIT:
`8b2a32020281aa67490c01633adf99c1a2ca6c9897e4c210bb6d0d001c15caca`.
The root image has SHA-256
`2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3`.
The [application member](../../../work/simulator/native-s9/original-boser-app-011/member.json)
is 425,409 bytes. Its SHA-256 is
`1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423`.
The actual HTTP response has those exact bytes.

The [client report](../../../work/simulator/qemu-s9/sd2022-private-pool-036/dashboard-result.json)
retains 11 requests and responses. They check the index, application asset,
an invalid session, an invalid user, login, session extension, advancing
native uptime, schema discovery, logout and session invalidation.
The client uses captured curl for HTTP framing. Rust controls and assesses
the test. Each run has a new VM and cookie file. Ports alone do not provide
cookie isolation.

[Run040](../../../work/simulator/qemu-s9/sd2022-private-pool-040/dashboard-result.json)
adds four native API requests. Boser stores `webTheme: DARK`, and a separate
query returns that value. Boser then restores the original unset value, and
another query returns `null`. The other public settings remain unchanged.
The test uses the exact `StorageIn` contract returned by the running service
and the update operation in the
[archived system application](../../../work/simulator/native-s9/original-boser-system-014/member.json).
This proves write, readback and restoration during one boot. Restart
persistence is a separate test and is not established by this result.

The [second restart cycle](../../../work/simulator/qemu-s9/original-dashboard-cycle-002/assessment.json)
failed after its first successful boot. Native preinit selected a new MAC
address because this reference environment has blank NAND. It then removed
the saved network settings. The exact archived
[MAC selection script](../../../work/simulator/native-s9/original-sd-mac-016/member.json)
and [SD cleanup script](../../../work/simulator/native-s9/original-sd-cleanup-017/member.json)
establish that behavior. The test keeps its static-network assertion.

The cycle driver now runs a separate provisioning boot. The guest writes a
fresh local unicast MAC to `uEnv.txt` on its private SD overlay. The original
bootloader imports that setting on later boots. This is a recorded environment
adaptation. It is not a factory identity. The driver does not edit the native
`.last_mac` or `.last_uenv` records. An
[independent SD inspection](../../../work/simulator/qemu-s9/dashboard-sd-inspection-003/inspection.json)
confirms the saved setting and unchanged bootloader, FIT and FPGA members.

The [third cycle](../../../work/simulator/qemu-s9/original-dashboard-cycle-003/assessment.json)
confirmed the saved MAC but failed during mining. Its execution stopped
advancing before the script finished. It remains a failed test. A separate
[repeat run041](../../../work/simulator/qemu-s9/sd2022-private-pool-041/dashboard-result.json)
passed its native write check and full firmware execution. That repeat does
not establish restart persistence or resolve the intermittent stall.

The [fifth cycle](../../../work/simulator/qemu-s9/original-dashboard-cycle-005/assessment.json)
passed the write and restore phases. Each phase also produced two blocks
accepted by Bitcoin Core. Its final boot completed the firmware script but
failed the dashboard probe. Native Boser opened port80, paused accepts while
starting its workers, then served the first GET after the probe's deadline.
An open TCP socket did not establish HTTP readiness. The probe now permits
up to four initial GET attempts within40seconds. It retains each attempt.
Only connection refusal and transport timeout can retry at this stage.
HTTP errors and later service operations cannot retry. The index hash,
asset hash, login, live-data and settings assertions still apply. The complete
restart cycle was repeated with these assertions.

The [sixth cycle](../../../work/simulator/qemu-s9/original-dashboard-cycle-006/assessment.json)
passes all three linked boots. It writes `DARK`, observes it after restart,
restores the original unset value, and verifies that value after another
restart. Each boot produces two fresh blocks through the executing firmware
and simulated ASIC interface. Independent verification and isolated Bitcoin
Core acceptance pass for all six blocks. Each node retains its block after
restart. The [phase record](../../../work/simulator/qemu-s9/original-dashboard-cycle-006/progress.json)
binds the disk snapshots, native service results, mining audits and node
results. This completes this settings persistence test. It does not complete
the native dashboard or exact physical board gates.

The new browser helper uses the protocol from the exact
[native vendor asset](../../../work/simulator/native-s9/original-boser-vendors-019/member.json):
`graphql-transport-ws`. Browser startup has its own support-tool allowance.
It occurs before miner startup. The miner keeps its CPU and wall limits.
[Run048](../../../work/simulator/qemu-s9/original-browser-048/browser-result.json)
passes the browser component test. It uses a fresh Firefox profile, renders
the native dashboard, verifies the archived application bytes, extends the
authenticated session and observes advancing HTTP uptime. Its real WebSocket
connection delivers two different native sample timestamps. The connection
initialization includes the language header used by the archived application.
Run047 omitted that header and failed before the acknowledgement. Its failure
remains recorded.

Run048 completes in 372,388 milliseconds, including browser startup. It has no
trace truncation. The persistent pool remains connected after the first two
accepted jobs. Its independent verifier checks every later submission. The
[hardware audit](../../../work/simulator/qemu-s9/original-browser-048/regtest-audit-001.json)
checks 102,121 computed hashes, 117 complete ASIC replies and 94 submissions. The
pool accepts 46 submissions and rejects 48 below its target. Two fresh-job block
candidates pass isolated Core acceptance and restart checks:
[candidate0](../../../work/simulator/bitcoin-core/original-s9-048-candidate-0/block-acceptance.json)
and [candidate1](../../../work/simulator/bitcoin-core/original-s9-048-candidate-1/block-acceptance.json).
The audit retains 44 additional hardware-correlated block candidates. Those
additional candidates have not each been submitted to Core.

The browser and miner wall clocks differ in run048. The native page therefore
shows an incorrect age. Its production hash-rate estimate also uses the
original difficulty scale. Do not use that displayed rate as functional
hash-rate evidence. The explicit simulator cap for this run is 1000 H/s.
The default remains 10000 H/s. Original firmware bytes, board topology and
ordinary interface clocks are unchanged. The fixed 4-bit ASIC filter remains
a separate recorded adaptation.

This command produced the expanded run057. Use a new output directory to repeat it:

```sh
target/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 /tmp/md-sim-original-boser-057 \
  spl sd2022-v1 backend=/tmp/md-sim-qemu-backend-077-ubsan budget=360 \
  presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
  local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
  adapt-fixed-filter-bits=4 hash-cap-hps=1000 pool-difficulty=-28,-27 \
  regtest-genesis save-sd dashboard-probe \
  browser-payload=work/simulator/network-vm/native-driver-024/payload.json \
  adapt-clock-epoch=1792591709 \
  console=work/simulator/qemu-s9/console-private-pool-014.sh
```

The [input specification](../../../work/simulator/qemu-s9/original-browser-057/process-spec.json)
pins the executed tools and images. The
[execution checksums](../../../work/simulator/qemu-s9/original-browser-057/execution-SHA256SUMS)
identify the full traces and result retained in the run directory. These are
two isolated local VMs. No external physical host or published dashboard was
tested. This remains an adapted reference environment, not an exact physical
S9 target or a complete dashboard pass.

[Run049](../../../work/simulator/qemu-s9/original-browser-049/browser-result.json)
first tested `adapt-clock-epoch=1792591709`. It passes the same
browser checks and the explicit clock check. The support VM's real-time clock
starts at that UTC epoch. After console activation, the original guest `date`
command sets wall time to the epoch plus supervisor elapsed time. The guest
reads back that value before Boser and bosminer start. Browser requests then
check native timestamps against their measured send and receive times, with
a five-second allowance for whole-second clocks and serial delivery.
This adaptation leaves monotonic clocks and device timers unchanged. It does
not test a physical real-time clock or a network time service. The
[adaptation record](../../../work/simulator/qemu-s9/original-browser-049/adaptation.json)
and [input specification](../../../work/simulator/qemu-s9/original-browser-049/process-spec.json)
retain the policy and epoch. Without this option, the runner preserves the
firmware's original wall-clock behavior.

Run049's [independent audit](../../../work/simulator/qemu-s9/original-browser-049/regtest-audit-001.json)
checks 102,301 computed hashes, 95 complete ASIC replies and 74 submissions.
Both fresh-job blocks pass isolated Core acceptance and restart checks:
[candidate0](../../../work/simulator/bitcoin-core/original-s9-049-candidate-0/block-acceptance.json)
and [candidate1](../../../work/simulator/bitcoin-core/original-s9-049-candidate-1/block-acceptance.json).

[Run057](../../../work/simulator/qemu-s9/original-browser-057/browser-result.json)
extends the browser test to two fresh profiles at the same origin. The first
profile logs in. The second still receives the login page, has no cookies,
and receives `AuthError` from the native session check. The first profile's
session remains valid. Two WebSocket samples show boards 6, 7 and 8, each with
63 chips at 100 MHz, the explicit temperature fixtures, four fan channels and
the active private pool. Sample timestamps advance from 1792591975 to 1792591976.
The test checks these fields against the selected reference model. It does
not claim physical calibration or use displayed rates as a hashing oracle.

The full run exits zero in 357,097 ms without trace truncation. The support VM
has four virtual CPUs and 2 GiB RAM; each browser has a limit of two CPUs,
768 MiB memory and 256 tasks. This fixes the heavy browser CPU throttling
seen in failed runs 052–054. Both browser groups are empty after cleanup.
Support startup and address space have separate bounds. The miner retains
its 330-second CPU, 360-second wall and 3 GiB address-space limits.

The [057 audit](../../../work/simulator/qemu-s9/original-browser-057/regtest-audit-001.json)
checks 42,154 computed hashes, 54 ASIC replies and 53 firmware submissions.
The pool accepts 23 and rejects 30 below its target. Both fresh jobs produce
Core-accepted blocks with height changes from 0 to 1 and restart persistence:
[candidate0](../../../work/simulator/bitcoin-core/original-s9-057-candidate-0-retry-001/block-acceptance.json)
and [candidate1](../../../work/simulator/bitcoin-core/original-s9-057-candidate-1-retry-001/block-acceptance.json).
The first node-test commands used an audit outside the source run directory.
Their replay checks failed before node execution. The corrected commands use
the identical audit bytes beside the original traces. Both failures remain
recorded. The audit retains 21 additional block candidates without individual
node acceptance claims. Exact physical targets and full-matrix gates remain open.

The [LuCI test059](../../../work/simulator/qemu-s9/original-luci-059/dashboard-result.json)
uses the same command without `browser-payload`, and adds `dashboard-luci`.
The native service advertises its flash operations route. The test follows
that route through the authenticated Boser service. It retains all 22 HTTP
requests and responses. The native backup is 7,626 compressed bytes and has
36 entries. Independent Rust gzip and tar readers check it without extracting
or executing a member. It contains the active private network configuration.
The firmware rejects a wrong form token with HTTP403 and a GET to the backup
operation with HTTP405. It also rejects a small invalid image sent through
the real multipart upload handler. Native uptime still advances afterward.

The authenticated proxy returns the literal form token `0000`. Standard LuCI
creates a longer token. Failed run058 exposed the test's incorrect assumption
about that length. The test now preserves the actual token and still requires
rejection of a different token. This does not establish token randomness or
security strength. No firmware authentication code was changed.

The full059 run exits zero in 237,322 ms without trace truncation. Its
[audit](../../../work/simulator/qemu-s9/original-luci-059/regtest-audit-001.json)
checks 28,165 computed hashes, 29 ASIC replies and 29 firmware submissions.
The pool accepts 21 and rejects 8 below its target. Both fresh-job candidates
pass Core acceptance, height changes and restart persistence:
[candidate0](../../../work/simulator/bitcoin-core/original-s9-059-candidate-0/block-acceptance.json)
and [candidate1](../../../work/simulator/bitcoin-core/original-s9-059-candidate-1/block-acceptance.json).
These results do not establish a valid update, backup restoration or recovery.

The [signed SD update inspection](../../../work/simulator/native-s9/signed-sd-update-002/inspection.json)
reads the exact archive with SHA-256
`469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e`.
It has 19,179,888 bytes. The isolated reader checks seven archive entries
and hashes all six regular members. Its SPL, U-Boot, FPGA and FIT objects
match the baseline bundle. The exported `COMMAND` and `CONTROL` files are
reference data. The inspector does not execute them. The native firmware
must still check the image signature and format before installation.

The signed-upload test uses a separate, bounded client operation. Failed
runs 061–063 and 067 received the complete image but reached the original
60-second test limit while native validation was still running. Contract
version 2 gives this large-file operation 120 seconds. Ordinary requests
retain 10 seconds and connections retain 2 seconds. The firmware timing
configuration is unchanged. Run069 used a stale client and did not test the
new budget. Build `s9_pool_guest` together with `s9_boot_bundle` when changing
this suite. Version 2 rejects the old client contract. Run070 then exposed
two more test bounds: the whole suite left only 89 seconds for the upload,
and the combined packet and ASIC trace exceeded 64 MiB. The selected upload
suite now has a 300-second pool window and a 128 MiB aggregate trace bound.
Its per-file limit is 64 MiB. It requires at least a 330-second host wall
budget; the miner CPU bound remains 330 seconds. These are test resource
limits. These failed runs remain failures.

Run075 passes the version3 maintenance sequence. The actual native service
stops mining, validates the complete signed image, cancels installation and
starts mining again. Its [execution result](../../../work/simulator/qemu-s9/sd2022-private-pool-075/result.json)
has exit0 and no truncated trace. The stopped miner refuses connections on
port4028 while the native HTTP uptime advances. Signed validation returns
HTTP200 in39,570ms. The restart opens a new mining connection with a different
frozen job seed. Both connections pass the independent hardware and header
audit: [session0](../../../work/simulator/qemu-s9/sd2022-private-pool-075/regtest-session-0.json)
and [session3](../../../work/simulator/qemu-s9/sd2022-private-pool-075/regtest-session-3.json).
Their shared run has10,290 computed hashes and10 serial results. Do not add
these shared counts twice. Each connection supplies two fresh jobs, a clean
transition and a difficulty change. All four block candidates pass isolated
Bitcoin Core acceptance and restart persistence:
[0/0](../../../work/simulator/bitcoin-core/adapted-s9-075-session-0-candidate-0/block-acceptance.json),
[0/1](../../../work/simulator/bitcoin-core/adapted-s9-075-session-0-candidate-1/block-acceptance.json),
[3/0](../../../work/simulator/bitcoin-core/adapted-s9-075-session-3-candidate-0/block-acceptance.json),
and [3/1](../../../work/simulator/bitcoin-core/adapted-s9-075-session-3-candidate-1/block-acceptance.json).

Run075 uses backend078, the corrected [PIC heartbeat contract](PIC16.md),
and the explicit [native monitor adaptation](FAN.md). It is an adapted
reference environment. It does not establish the original physical board
gate. The native signed image was cancelled; no update was installed.

Browser run076 remains failed. Its packet capture shows an idle connection
that the native server closed with an empty HTTP408 response before Firefox
sent the session request. Stream266 has server408 at251.874259s, serverFIN
at251.880131s and the first client POST at252.104630s. The first profile's
login and live subscription passed, but this does not pass the complete run.
The revised browser client retains the raw response and permits one retry
only for an empty HTTP408 with `Connection: close` on the unauthenticated
session check. Both attempts share one30-second deadline. This follows
[RFC9110 section15.5.9](https://www.rfc-editor.org/rfc/rfc9110.html#name-408-request-timeout).
The retry must still receive the native `AuthError`; HTTP408 is never a pass.
The revised client requires a new actual browser run.

The [native installation run077](../../../work/simulator/qemu-s9/native-update-cycle-001/install-077/result.json)
passes in307,942ms with exit0 and no truncated trace. It writes the native
dashboard theme `DARK`, stops the miner, uploads the exact signed image,
checks the native checksum and posts `step=2&keep=1`. The archived updater
reports writing SPL, U-Boot, FPGA and FIT. Its kernel then reports a restart
at188.724172s. The supervisor retains897 serial bytes, observes EOF and reaps
the actual QEMU exit0. It does not kill QEMU on this successful path.

The [snapshot](../../../work/simulator/qemu-s9/native-update-cycle-001/install-077/sd-snapshot-0.json)
uses storage schema3. It records this native restart separately from an
issued `sync` command. The latter remains false. Its private overlay has
20,512,768 raw bytes and18,950,912 compressed bytes. The
[independent inspection](../../../work/simulator/qemu-s9/native-update-cycle-001/install-077-inspection/inspection.json)
checks all four boot files against the signed image and baseline. The saved
boot environment and MAC are unchanged. The monitor override is absent.
The native updater left `upper/sysupgrade.tgz` for the next boot to restore.
Its SHA-256 is `c56e6973f0801725c099172adea3571639d66d364ca50eb01a9c779008629f1b`.
The archive alone does not establish settings persistence.

The [pre-install mining audit](../../../work/simulator/qemu-s9/native-update-cycle-001/install-077/regtest-session-0.json)
checks4,682 computed hashes, six ASIC replies, four submitted results and two
fresh jobs with a clean transition and difficulty change. Both candidates
pass Core acceptance and restart persistence:
[first](../../../work/simulator/qemu-s9/native-update-cycle-001/core-install077-0/block-acceptance.json)
and [second](../../../work/simulator/qemu-s9/native-update-cycle-001/core-install077-1/block-acceptance.json).
Run078 boots this disk. It verifies the original monitor and all four boot
file hashes, then reads the exact saved mining and static network settings.
The theme is null, so the required DARK persistence check fails. The native
default backup omits `/etc/web_storage.json`. No post-update browser, mining
or Core pass is claimed. Its large snapshot export also exceeds the old
64MiB trace bound. Run078 remains failed and its raw logs are retained.
Saved-disk runs now reserve the existing128MiB maximum trace allowance.
Partial snapshot bytes cannot pass retention or resume checks.

The archived `/etc/sysupgrade.conf` documents extra files to preserve.
The [explicit configuration](../../../work/simulator/qemu-s9/native-update-cycle-002/keep-policy-adaptation.json)
adds only `/etc/web_storage.json` to that native policy. Install contract2
also checks the actual downloaded backup for the requested public settings
before it stops mining or commits the image. The API spells the tested theme
`DARK`; the native file spells it `Dark`. The check uses an explicit mapping
and rejects missing, changed and unknown values.

[Configured run080](../../../work/simulator/qemu-s9/native-update-cycle-002/install-080/dashboard-result.json)
passes this check, signed validation, installation and native restart. Its
backup includes the150-byte configured keep-file policy and108-byte theme
file. The updater reaches the kernel restart at209.542933s, then QEMU exits0.
The whole run completes in307,929ms without trace truncation. The
[independent disk inspection](../../../work/simulator/qemu-s9/native-update-cycle-002/install-080-inspection/inspection.json)
checks the boot files, retained boot environment, removed monitor override
and native settings archive. The
[mining audit](../../../work/simulator/qemu-s9/native-update-cycle-002/install-080/regtest-session-0.json)
checks4,803 computed hashes, four serial results and two fresh jobs.

[Post-update run081](../../../work/simulator/qemu-s9/native-update-cycle-002/post-update-browser-081/dashboard-result.json)
verifies the original monitor, four boot files, saved mining configuration,
network and keep-file policy. The native API reads the saved DARK theme and
restores its original null value. The native browser passes login, assets,
live telemetry, WebSockets and separate-profile authentication tests. Its
second profile receives the expected AuthError without an HTTP408 retry.
The run exits0 in357,342ms with no truncated trace. Its mining audit verifies
52,652 computed hashes,56 serial results and two fresh jobs. Both blocks pass
Core acceptance and restart checks in `native-update-cycle-002/core-post081-0`
and `core-post081-1` under `work/simulator/qemu-s9/`.

[Run082](../../../work/simulator/qemu-s9/native-update-cycle-002/restored-settings-082/dashboard-result.json)
boots the run081 disk and reads the original null settings. It exits0 in
237,505ms with no truncated trace. The separate run081 disk inspection fails:
7zip reports an ext4 header warning and the filesystem has its RECOVER flag.
The raw listing is retained. No filesystem-consistency pass is claimed for081.
The original physical board and complete update/recovery gates stay open.

This configured installation command was tested with source099 and install
contract2. Use a new output directory to repeat it. The selected input snapshot is the hash-checked native MAC provisioning
result from the earlier settings cycle. Its native boot environment is an
explicit adaptation for the blank reference NAND.

```sh
target/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/native-update-cycle-002/install-080 spl sd2022-v1 \
  backend=work/simulator/qemu-s9/backend-078-ubsan budget=360 \
  presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
  local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
  adapt-fixed-filter-bits=4 hash-cap-hps=1000 pool-difficulty=-28,-27 \
  regtest-genesis save-sd dashboard-probe dashboard-luci dashboard-update-install \
  dashboard-state=work/simulator/qemu-s9/native-update-cycle-001/write-state.json \
  sd-state=work/simulator/qemu-s9/original-dashboard-cycle-006/provision/sd-snapshot-0.json \
  adapt-clock-epoch=1792591709 \
  adapt-monitor-fan-source=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  console=work/simulator/qemu-s9/native-update-cycle-002/console-initial.sh
```

The explicit `sd-native-reboot` option requests `/sbin/reboot` through the
guest shell after service checks. It requires kernel restart, serial EOF and
actual QEMU exit0 before saving schema3 storage. It does not use `-f` or kill
the emulator on success. [Run084](../../../work/simulator/qemu-s9/native-sd-reboot-084/result.json)
passes in237,560ms without trace truncation. The original monitor remains in
use. Its [independent disk inspection](../../../work/simulator/qemu-s9/native-sd-reboot-084-inspection/inspection.json)
passes with the earlier file parser. The restored theme is null, the four
boot-file hashes match, and no monitor override remains. Mining verification
checks33,001 computed hashes,26 serial results and two fresh jobs. Both blocks
pass Core acceptance and restart persistence. This does not relabel the081
disk failure or establish crash recovery. The stronger inspector rejects the
raw084 snapshot because its ext4 journal still requires recovery. Explicit
committed-journal replay on a disposable inspection copy then passes the full
read-only filesystem check and strict file inventory. This is recovered
consistency, not evidence of a clean unmount. The saved snapshot used by the
firmware is not repaired. The cycle003 provisioning disk still fails after
journal replay because its free-space counts disagree.

This command was tested with source103. Use a new output directory to repeat it.

```sh
target/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/native-sd-reboot-084 spl sd2022-v1 \
  backend=work/simulator/qemu-s9/backend-078-ubsan budget=360 \
  presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
  local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
  adapt-fixed-filter-bits=4 hash-cap-hps=1000 pool-difficulty=-28,-27 \
  regtest-genesis save-sd sd-native-reboot dashboard-probe \
  dashboard-state=work/simulator/qemu-s9/native-update-cycle-001/verify-state.json \
  sd-state=work/simulator/qemu-s9/stratum-response-083/sd-snapshot-0.json \
  adapt-clock-epoch=1792591709 \
  console=work/simulator/qemu-s9/native-update-cycle-002/console-readback.sh
target/release/examples/s9_sd_snapshot_inspect \
  work/simulator/qemu-s9/native-sd-reboot-084/sd-snapshot-0.json \
  work/simulator/qemu-s9/native-sd-reboot-084-journal-002 replay-journal
```

Run040 also completes the original firmware mining path. Its
[independent audit](../../../work/simulator/qemu-s9/sd2022-private-pool-040/regtest-audit-001.json)
checks240352computed hashes,262ASIC replies and two fresh accepted submissions.
Both resulting blocks pass isolated Bitcoin Core acceptance and restart
checks: [candidate0](../../../work/simulator/bitcoin-core/original-s9-040-candidate-0/block-acceptance.json)
and [candidate1](../../../work/simulator/bitcoin-core/original-s9-040-candidate-1/block-acceptance.json).

The archived root account has an empty password. Run 034 incorrectly
expected a failed root login with a supplied password. That test remains
failed. The native service returned `VoidResult`, which the archived
application uses for success. The corrected test checks an unknown user
and then the actual empty-password login. It does not claim that this
default account rejects arbitrary passwords.

The tested command is below. Use a new output directory to repeat it.

```sh
target/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/sd2022-private-pool-036 spl sd2022-v1 \
  backend=/tmp/md-sim-qemu-backend-067-ubsan budget=360 \
  presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
  local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
  adapt-fixed-filter-bits=4 pool-difficulty=-28,-27 regtest-genesis save-sd \
  dashboard-probe console=work/simulator/qemu-s9/console-private-pool-014.sh
```

The [process specification](../../../work/simulator/qemu-s9/sd2022-private-pool-036/process-spec.json)
pins the inputs and limits. The two local VMs use a private Ethernet
connection. `http://10.0.2.15` is the client VM's selected origin. It is not
a host listener or a published endpoint. The
[packet capture](../../../work/simulator/qemu-s9/sd2022-private-pool-036/pool.pcap)
contains actual traffic. The completion record in the execution output
binds its checksum.

Run 035 passed these HTTP checks but stopped later on a fan-model error.
Backend 067 adds clock and write diagnostics; it does not change model
behavior. Run 036 did not reproduce that error. The intermittent defect
remains open. Neither run establishes an exact physical S9 target or a
complete dashboard gate.

Backend069 fixes a separately reproduced late fan callback. See the
[fan regression](FAN.md). Run039 and run040 complete with this backend.
The earlier035log does not prove that its failure had the same cause.

## Damaged signed update

[Run087](../../../work/simulator/qemu-s9/native-update-rejection-087/dashboard-result.json)
passes a separate negative upload test. The original signed archive remains
unchanged. [Isolated preparation](../../../work/simulator/native-s9/tampered-signed-update-001/manifest.json)
changes one bit at byte9168902 inside its FIT payload. A complete byte comparison
checks that all other bytes, including the signature trailer, remain unchanged.
The derived19,179,888-byte image has SHA-256
`98a5748d67fd98e4be0e54fb752f82620163f16b848cbdd0c6ee1ddf89b6ef02`.
It is a negative input, not another executable firmware target.

The native LuCI handler receives the complete upload and returns its
unsupported-format page in11,608ms. It provides no install confirmation.
The native mining service restarts and processes two new jobs. The generic
HTTP error does not identify which validation check rejected the file.
The run exits0 in307,995ms without trace truncation.

The [before](../../../work/simulator/qemu-s9/native-update-rejection-087/regtest-session-0.json)
and [after](../../../work/simulator/qemu-s9/native-update-rejection-087/regtest-session-2.json)
audits verify actual ASIC responses and firmware submissions. Each session
has two fresh jobs, a clean transition and a difficulty change. Their shared
execution trace contains17,630 computed hashes and11 serial results. Do not
add these shared counts twice. Core accepts all four blocks and retains them
after restart:
[0/0](../../../work/simulator/bitcoin-core/native-update-rejection087-s0-0/block-acceptance.json),
[0/1](../../../work/simulator/bitcoin-core/native-update-rejection087-s0-1/block-acceptance.json),
[2/0](../../../work/simulator/bitcoin-core/native-update-rejection087-s2-0/block-acceptance.json),
[2/1](../../../work/simulator/bitcoin-core/native-update-rejection087-s2-1/block-acceptance.json).

These commands built the separate optimized tools and ran087. The separate
build directory preserved another suite's frozen binaries. Use new output
paths for another run. The selected derived input is the retained preparation
above. It is checked by its full hash before upload.

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/rejection-target-109 cargo build --release -p md-sim-firmware --example s9_boot_bundle --example s9_pool_guest --example s9_update_fault --example isolated_network_vm --offline --locked
work/simulator/rejection-target-109/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/native-update-rejection-087 spl sd2022-v1 \
  backend=work/simulator/qemu-s9/backend-078-ubsan budget=360 \
  presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
  local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
  adapt-fixed-filter-bits=4 hash-cap-hps=1000 pool-difficulty=-28,-27 \
  regtest-genesis dashboard-probe dashboard-luci \
  dashboard-update-corruption=work/simulator/native-s9/tampered-signed-update-001/fault.tar \
  adapt-clock-epoch=1792614360 \
  adapt-monitor-fan-source=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  console=work/simulator/qemu-s9/console-private-pool-014.sh
```

[Run086](../../../work/simulator/qemu-s9/native-update-rejection-086/result.json)
failed before miner startup. Its larger debug support image did not unpack
within the256MiB support VM. The optimized run retains the same memory budget.
The failed run is not a firmware rejection result. Neither run closes the
original physical board, full recovery or current Boser remote access gates.

## Automatic signed update cycle

[Cycle004](../../../work/simulator/qemu-s9/automated-update-cycle-004/assessment.json)
passes all four linked boots with source108. Its
[frozen input manifest](../../../work/simulator/qemu-s9/automated-update-cycle-004/driver-input.json)
binds eight helper builds, both emulators, the original images, console
scripts, selected browser payload, policies and fresh run values. The driver
checks those identities before each phase and again at completion.

The native firmware provisions the private MAC, writes DARK, installs the
signed image, reads the saved theme, restores its original null value, and
verifies null after another restart. The real Firefox suite passes in the
post-install boot. It checks native login, archived assets, live data,
WebSockets and two separate browser profiles.

| Boot | Computed hashes | ASIC replies | Firmware submissions | Core blocks |
| --- | ---: | ---: | ---: | ---: |
| Provision | 30,446 | 22 | 18 | 2 |
| Write and install | 7,801 | 4 | 3 | 2 |
| Read back and restore | 41,077 | 29 | 28 | 2 |
| Verify restoration | 29,572 | 27 | 23 | 2 |

Each boot has two fresh jobs, a clean transition and a difficulty change.
The independent verifier reconstructs each submitted header. All eight node
checks show acceptance from height0 to1 and the same tip after restart.
The assessment links each audit, disk check, native service result and Core
result. All four firmware processes exit0 without trace truncation.

Provisioning and the two normal boots use the recorded
`sd-read-only-checkpoint`: the guest runs `sync`, remounts both SD filesystems
read-only, checks their actual mount flags, and then requests a native reboot.
Their strict read-only disk inspections pass without replay or mutation.
This is an explicit maintenance operation. It does not prove unassisted
shutdown or power-loss recovery. The installer uses its own restart. Its
inspection permits committed-journal replay on a separate copy; in this run,
that operation makes no byte change. The full filesystem check then passes.
The earlier failed cycles and dirty snapshots remain failed evidence.

This command ran the complete cycle. Use a new output path to repeat it.

```sh
target/release/examples/s9_dashboard_cycle \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/backend-078-ubsan \
  /tmp/md-sim-qemu-pool-003-ubsan \
  work/simulator/qemu-s9/automated-update-cycle-004 \
  signed-update=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  browser-payload=work/simulator/network-vm/native-driver-024/payload.json
```

The original physical board, complete update/recovery matrix and this board
path's SSH/TLS publication remain incomplete. This cycle uses two local VMs.

## Prepared local and worker execution

`s9_execute_prepared` uses the existing coordinator executors and worker
protocol. It transfers the complete prepared process, including original
firmware, device backend, support VM, pool seed, clock, limits and adaptation
records. It uses the same result reader as `s9_boot_bundle`. A process success
does not grant a physical board, mining or complete dashboard pass.

[Local088](../../../work/simulator/qemu-s9/worker-reference-local-088/transport-assessment.json)
and [worker089](../../../work/simulator/qemu-s9/worker-reference-ipc-089/transport-assessment.json)
execute the same frozen process. Its ProcessSpec hash is
`e5d6f6458ec2e94deeacd942e80b0faed16e3f341524824063759acf07ae1984`.
The complete locked-run hash, including the report build and metadata, is
`f2bc75c5313bc5f350caeb81eb5055c01260f18b96031b92c8e7471ae4573f0f`.
The seed was generated during preparation. Repeating that preparation output
is the same work identity, not another random-seed test.

Both executions pass native upload rejection and resume mining in sessions0
and3. Each session has two jobs, a clean transition and a difficulty change.
The local trace contains15,560 hashes and6 ASIC replies; the worker trace has
16,102 hashes and9 replies. Both have separate independent audits and four
Core acceptance and restart checks. The candidate hashes match because the
input work matches. The eight node checks are separate; no result was copied
between transports.

The worker's [transfer record](../../../work/simulator/qemu-s9/worker-reference-ipc-089/artifact-transfer.json)
checks52 distinct objects at the destination. The returned result contains
109,112,953bytes and has SHA-256
`fed17563be2e9b159fd339fb395383d7519adf2f066d7611b2518ad3670a42cb`.
[Cleanup](../../../work/simulator/qemu-s9/worker-reference-ipc-089/worker-cleanup.json)
acknowledges that exact result. It removes the completed run and keeps the
published input cache and token receipt. It preserves the archive.

These commands built and exercised source110. Use new output paths to repeat
the executions. The prepared input is the retained
[process specification](../../../work/simulator/qemu-s9/worker-prepared-110/process-spec.json)
and its adjacent input store. It was made with the damaged-image command above,
`prepare-only`, epoch1792614887 and the source110 build directory.

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/worker-target-110 cargo build --release -p md-sim-firmware --example s9_boot_bundle --example s9_execute_prepared --example s9_pool_guest --example isolated_network_vm --offline --locked
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/worker-target-110 cargo build --release -p md-sim-coordinator --bin md-sim-worker-ipc --offline --locked
work/simulator/worker-target-110/release/examples/s9_execute_prepared \
  work/simulator/qemu-s9/worker-prepared-110 \
  work/simulator/qemu-s9/worker-reference-local-088 local
work/simulator/worker-target-110/release/examples/s9_execute_prepared \
  work/simulator/qemu-s9/worker-prepared-110 \
  work/simulator/qemu-s9/worker-reference-ipc-089 \
  worker-ipc=work/simulator/worker-target-110/release/md-sim-worker-ipc
```

This is a separate worker process on the same host, using inherited connected
IPC. Each miner and pool VM runs in the closed process environment. The client
and worker are not separate physical hosts. The helper accepts `worker-ssh=CONFIG`
through the existing SSH transport. Later SSH board attempts reached archived
SPL, U-Boot, and Linux, but have not completed mining. The
[progress record](../../../work/simulator/continuation-2026-10-21.md) retains
their failures and the current test. These results do not test its published dashboard.
Source111 adds bounded live output files to the common local executor. Its
isolated-process test observes both streams before cancellation. The source110
local replay above did not yet have those live files.

Source112 [local090](../../../work/simulator/qemu-s9/worker-reference-local-090/transport-assessment.json)
and [worker091](../../../work/simulator/qemu-s9/worker-reference-ipc-091/transport-assessment.json)
repeat the frozen input with strict isolation-tool identity checks. Both pass.
Local090's live stdout and stderr files match the final guest and backend logs.
Both mining sessions have independent audits. All eight Core checks pass.
These use the same prepared seed as088/089.

The [source113 cancellation test](../../../work/simulator/qemu-s9/worker-reference-cancel-092/transport-assessment.json)
sends SIGTERM to the owned coordinator after the selected worker's native miner
stop event. The coordinator requests worker cancellation. It retrieves the
complete cancelled result and acknowledges cleanup. The retained
[signal record](../../../work/simulator/qemu-s9/worker-reference-cancel-092/cancellation-request.json)
identifies signal15. This is a cancellation test, not a completed mining run.
SIGINT uses the same path. It must not be used to claim recovery after power loss.

[Worker094](../../../work/simulator/qemu-s9/worker-reference-ipc-094/transport-assessment.json)
uses a fresh prepared seed and source114. It also checks the normal path with
the signal watcher active and no cancellation requested. Prepared inputs can
be read-only; their permissions and bytes stay unchanged. This command passed:

```sh
work/simulator/worker-target-114/release/examples/s9_execute_prepared \
  work/simulator/qemu-s9/worker-prepared-114 \
  work/simulator/qemu-s9/worker-reference-ipc-094 \
  worker-ipc=work/simulator/worker-target-114/release/md-sim-worker-ipc
```

It completed in308.321seconds with intact traces. The independent
[session0 audit](../../../work/simulator/qemu-s9/worker-reference-ipc-094/regtest-session-0.json)
and [session3 audit](../../../work/simulator/qemu-s9/worker-reference-ipc-094/regtest-session-3.json)
each verify two fresh jobs, a clean transition and a difficulty change.
The shared trace contains13,159 computed hashes and five verified ASIC replies.
All four Core checks pass: [0/0](../../../work/simulator/bitcoin-core/worker-reference-ipc094-s0-0/block-acceptance.json),
[0/1](../../../work/simulator/bitcoin-core/worker-reference-ipc094-s0-1/block-acceptance.json),
[3/0](../../../work/simulator/bitcoin-core/worker-reference-ipc094-s3-0/block-acceptance.json),
[3/1](../../../work/simulator/bitcoin-core/worker-reference-ipc094-s3-1/block-acceptance.json).

The Rust SSH transport now supports a persistent authenticated channel with
bounded request frames. It checks the pinned host key and local tool identities.
Each request has a deadline. A failed request closes the channel and is not
automatically repeated. The worker relay supports `--stream`; existing one-shot
clients still work. The separate [SSH115 transport suite](../../../work/simulator/network-vm/ssh-stream-115/remote-report.json)
passes in distinct client and worker namespaces inside a disconnected local VM.
It checks actual transfer, reconnect, execution, result retrieval, stop and cleanup.
Its HTTP/TLS service is a protocol fixture. That result does not test this board's
native dashboard or execute this board firmware over SSH.

The [SSH123 component suite](../../../work/simulator/network-vm/ssh-live-123/remote-report.json)
also executes the common coordinator's live-log path. It observes both actual
shell streams before cancellation. It checks the saved bytes against the full
verified process result and checks the cleanup acknowledgement. The worker
reports saved stream lengths, so unchanged lengths do not cause empty log
requests. Each polling step reads at most one bounded chunk from each stream.
The coordinator saves `worker-stdout.log` and `worker-stderr.log`. These files
are provisional until they match the complete result. The isolated worker test
also changes a saved prefix and verifies rejection without remote cleanup.

These component commands passed. Use a new output directory to repeat a run.
The captured scenario selects the source123 helper and worker builds.

```sh
work/simulator/ssh-target-123/release/examples/isolated_network_vm \
  work/simulator/network-vm/ssh-live-123 \
  work/simulator/network-vm/ssh-stream-driver-123/scenario.json
work/simulator/ssh-target-123/release/examples/remote_namespace_check report \
  work/simulator/network-vm/ssh-live-123
```

This is actual SSH over a private virtual network between separate client and
worker environments in one local VM. Its HTTP and TLS services remain protocol
fixtures. The test grants no firmware, native dashboard, or exact hardware gate.
