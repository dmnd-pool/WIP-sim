# SSH and dashboard access implementation

The complete target and remote dashboard gates remain open.

The Rust coordinator now has `SshTransport` and `SshTunnel`. OpenSSH verifies a
frozen known_hosts file. Password prompts, agent use, unrequested forwarding,
default configuration and host-key updates are disabled. Key and trust paths
use a literal grammar because OpenSSH expands option paths. The server runs a
one-shot Rust relay to the existing owner-only Unix worker socket. Firmware
names never become shell commands. Requests and output have byte and time
bounds. The worker survives client disconnects and retains token identities.

`network-vm/ssh-005` passed actual SSH, wrong host-key rejection, a stalled
handshake, resumed artifact transfer, destination verification, local/SSH
ProcessSpec equality, status, logs, cancellation, result retrieval, cleanup and
shutdown. This test used separate processes in one isolated Linux VM with no
host network attachment. Separate client/worker namespaces are the next step.
Do not describe this test as an external physical host.

`md-sim-access` is a Rust gateway controller. It reuses installed Nginx for HTTP
framing, TLS, streaming and WebSockets. Gateway authentication uses client
certificates. It is separate from firmware cookies and Authorization headers.
The configuration binds an explicit IP, verifies its certificate identity,
requires client CIDRs and an isolated browser-context identity, and fixes one
upstream loopback endpoint. Host, Origin, Location and cookie-domain changes
are limited to the declared origin. The gateway replaces any upstream run-ID
header. Direct-IP browser tests must use the declared isolated context; ports
alone do not isolate cookies.

The endpoint manifest currently trusts the caller's run/hash/upstream binding.
The coordinator must establish that binding from the actual live run before
this component can count as target publication. Separate-client native login,
telemetry, configuration, persistence and browser tests remain required.

The credential command passed with actual OpenSSL and independent OpenSSL
server/client purpose and IP checks:

```sh
cargo build -p md-sim-access --offline --locked
target/debug/md-sim-access credentials work/simulator/access-credentials-003 127.0.0.2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

This is a component test identity, not a miner run. The command requires a new
directory. Private keys stay outside exported reports. Certificates last one
day. `access-credentials-001` preserves the rejected overlong certificate name;
002 passed, and 003 passed after disabling unrecorded OpenSSL configuration.
The public manifest records the tool and certificate hashes. The source
includes a credential-provider sample without secrets.

Tests `tests-active-012.log` passed before the latest CLI additions. The
corresponding strict Clippy check also passed. New CLI and gateway changes need
their affected integration checks. No untested command is a completion gate.

2026-10-19 review loop: credentials now have a 15-second wall deadline,
64-KiB diagnostics cap, private process group, no-new-privileges, parent-death
signal and zero core limit. Exit observation uses waitid(WNOWAIT); cleanup
stops the group before reaping its leader. This prevents signaling a recycled
PID. Nginx also has RLIMIT_CORE=0. Real blocked and noisy subprocess tests and
certificate creation/preflight tests pass. Strict lint found test-module order;
move it after implementation and rerun. SSH subprocess fixes are in progress.

The separate-namespace fixture report is network-vm/namespaces-007/remote-report.json.
It tests real SSH/TLS transport only. The native S9 integration is a separate
run. Review requested stronger report-to-binary binding and explicit role/run
identity checks; do not treat fixture data as native dashboard evidence.
