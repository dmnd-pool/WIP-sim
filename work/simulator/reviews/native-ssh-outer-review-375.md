# Native SSH recovery evidence review375

The outer result import and assessment pass this review. This reviewer did not
write `worker_channel_ssh.rs` or the outer probe integration. This reviewer did
write the namespace helper and broker. The observations below are an independent
check of the parent's outer import and saved evidence, not an independent
review of the authored namespace code. A separate code reviewer is still needed.

The exact retrieved result is 7,535 bytes with SHA-256
`be8d2a42cb705fdec9d6b69729dcdb246c7cdf480945f18041e7cd4955da5dfa`.
A fresh read confirms that identity against the worker cleanup acknowledgment
and both saved run records. The selected run is
`run-1e17148f730c32fcedaf398d75bc803f0a1a90a88ac7784f18b33c472b7f4f81`.
Cleanup reports `removed:true`. The result is cancelled, as required by this
test: recovery stops the still-running support process. The result contains
23 sequential observations with the actual hash of this run's fresh payload.
The trace is not truncated.

The log has 391 bytes before client loss, 2,399 at the fresh SSH connection,
and 3,119 after a separate 500ms interval. Both post-loss Status records name
the selected run and report it active. The retained complete trace includes
those bytes. The guest also compares the earlier byte prefix with the later
native log before it requests recovery and stop.

The first PID namespace contains an actual `ssh` process. After termination,
the owned Bubblewrap child becomes a zombie and is reaped with wait status9.
The process scan then finds no old client processes. Checked namespace file
descriptors hold the old objects across replacement. The replacement mnt, pid,
uts and ipc inode numbers differ. Server and client use distinct network and
process environments. The client attempts reuse the same private veth network;
they do not claim two different network namespaces. The SSHD log contains two
public-key acceptances and two connection closures. Both role reports pass.

The persistent broker forwards 56 requests and acknowledges real worker
shutdown. This run does not inject an unread reply or a wrong-UID peer. Those
cases have separate actual-worker evidence in [broker361](worker-broker-361.md).
This separation prevents a zero-count event from being counted as exercised.

The outer assessment now checks exact result identity, fresh payload and output
sequence, both post-loss Status records, cleanup run ID, `removed:true`, and
the acknowledged exact result. It imports available artifacts before checking
the guest pass flag. Thus a later observation failure can retain a complete
result while keeping the test failed. No new concrete defect was found in
this outer import path.

Reviewed outer import source SHA-256:
`d637ef64401607fd8bd17fb6f6a54e346ec3d2deca60ddb00044b5964446227f`.
The captured namespace helper is
`79f9fb3a033a01a6fdfc77305d175e40abbe344af9420e0fef14b72ad198c248`.
The actual native worker is
`fbae558c7872cbe372fb224fbedd8a26e6be3e3caf30e50bc0138a5c293dfa89`.

See the [host assessment](../network-vm/native-ssh-channel-375/assessment.json),
[independent byte and evidence check](../network-vm/native-ssh-channel-375/outer-artifact-review.json),
[exact worker result](../network-vm/native-ssh-channel-375/execution/worker-result.json),
[cleanup receipt](../network-vm/native-ssh-channel-375/execution/worker-cleanup.json),
and [captured SSH inputs](../network-vm/native-ssh-channel-375/ssh-inputs.json).

This is one native worker outside a disconnected support VM. Actual OpenSSH
connects separate client and server namespaces inside that VM. It proves
selected support-process survival after client/SSH loss, fresh recovery,
result retrieval, stop and cleanup. The result was retained inside the VM
before cleanup and exported later. It does not prove an outer durable receipt,
whole-VM survival, an external physical host, firmware execution, mining,
dashboard access, or exact physical-target acceptance.
