# Independent native access and retention review 441

The selected adapted S9 HTTP and TLS path passed this review. The exact native,
pool and access evidence was retained before worker cleanup. This is not an
exact-board or milestone 2 completion result. Browser testing was not requested.

The selected run is
`run-1c963695c1adf0903873ff423ff2f3c4be65537d03241ecd445b4fcef88c3268`.
Its full locked-run hash is
`09e19cc31d326fbee7f9d88c8cbaeca636f390d0fc03106850b011fbeea4f5a9`.
The [input review](native-access-inputs-independent-441.json) recomputed that hash
and checked the captured helpers against frozen 440. The native command and all
native resource limits equal the prior 430 inputs: 10 H/s, 360-second wall limit,
330-second CPU limit and 3 GiB address-space limit. The pool remains 300 seconds,
with a five-second observation margin. No browser resource profile was selected.

## Actual access checks

The independent [byte and response audit](native-access-retention-independent-441.json)
checked 15 raw HTTP exchanges through the real SSH tunnel and 15 raw TLS exchanges
at `https://192.0.2.1:9443/`. This was the selected endpoint inside the support VM.
It is not a host-browser or external physical-host URL.

The publication, provider authorization, complete lock, policy, original run,
worker process, carrier endpoint and tunnel destination agree. All 15 TLS response
headers name that same run. The actual client and server have different network,
mount, PID, UTS and IPC namespaces. The client has only loopback and its private
veth interface. It has no direct route to the miner network or external network.
The native worker runs outside that VM on the same execution host.

The raw responses establish these checks, without trusting only `passed` fields:

- A client without its certificate was rejected with HTTP 400 and curl exit 22.
- Guest authentication rejected an absent session and invalid credentials. It
  accepted login and session extension. Logout succeeded, then the expired
  session was rejected. HTTP and TLS created distinct native session values in
  separate fresh cookie stores.
- The original application asset had 425,409 bytes and SHA-256
  `1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423`
  through both transports.
- Native uptime advanced from 70 to 74 seconds over HTTP, then from 88 to 91
  seconds over TLS, with the same boot start, host name and firmware version.
- Both tests changed the theme from null to DARK, read it back, and restored the
  original value. Every retained response body, header and request matches its
  report bytes and hash.
- The selected worker remained active after access checks. Gateway and tunnel
  cleanup completed. The server stop receipt names the owned nginx PID from the
  exact publication; the client stop receipt names the selected run. The carrier
  later observed the same worker stop and closed its listeners.

The readiness fix was exercised by the actual native run. It recorded four
`ECONNREFUSED` results at 2,503–2,683 ms, then a successful connection at 2,737 ms.
The loop finished at 2,744 ms within the existing 30-second budget. The native
HTTP/TLS response tests followed. This supports the fix; it does not recover the
missing operating-system error from failed run 431.

## Exact retained results

The review rehashed both native result copies, all 260 extracted access files,
all ten pool session files, and the pool stdout/stderr bytes. It compared them
with the exact retained bundles. Decoding the outer and native ProcessResult
logs gives the retained raw logs byte for byte. The actual runtime stdin receipt,
guest final receipt, host receipt, worker receipt and cleanup record agree.

| Retained object | SHA-256 | Bytes |
| --- | --- | ---: |
| Native result | `c3faec35da051fb3e14f86eecdae4507f68ec62bc4e6046fc9904de16f872b7d` | 105932004 |
| Pool bundle | `aa66b980a4ace342f683918caeaa00bf29bcdf1c29870981c44bf226d86f53eb` | 93953 |
| Access bundle | `38260f925109ec16f08718921283e34fb235f85017afde601ea28c59a06a55ef` | 15921402 |

The native process exited zero in 305,154 ms. The outer process exited zero in
767,202 ms. Both traces were untruncated, and the selected final assessment passed.
The logs contain actual archived SPL, U-Boot, Linux and Boser service execution.
The adaptation manifest still marks the original-board gate incomplete.

## Mining and Core evidence 443

The separate [independent mining/Core link audit](native-mining-core-links-independent-443.json)
checked the frozen auditor build and source hashes, exact 441 native result,
external pool bundle, selected session and every copied source input. It also
reconstructed all seven submitted headers from the actual Stratum messages and
recomputed SHA-256d with a separate implementation. It checked the coinbase,
merkle root, version mask, nonce, job, pool target and two full candidate blocks.

Two fresh jobs, a clean-job transition and difficulty changes from 2^-31 to
2^-30 were observed. The frozen hardware audit records 1,687 computations and
seven matched hardware/firmware submissions. The independent wire replay confirms
five accepted shares and two LowDifficulty results.

The two exact blocks were imported by isolated original Bitcoin Core processes.
The raw Core logs show each fresh chain move from height 0 to height 1, then load
the same height-1 hash after a clean restart without a loadblock argument:

- `756bad500232f33432d1fb307b29420adf3c0177f6ba1b2443c3b3869b467a63`
- `21c7823c03e5097c15eb5d705793371216fc3d4dc109ea3fc0679965d000d916`

The exact Core binary, imported block bytes, source run links and raw result logs
were checked. The node did not generate these blocks. This proves isolated
import/restart acceptance; it does not prove RPC operation or host-crash durability.
Only LowDifficulty rejection was observed in selected pool session 0. Duplicate,
stale, invalid and out-of-range rejection are not established by this session.
Component negative tests remain separate.

Browser behavior, WebSockets, restart persistence on this execution identity,
client-loss survival, external physical-host deployment, exact-board calibration
and full-target coverage remain open. The complete simulator is not finished.
The [access input manifest](native-access-retention-independent-441-inputs.json)
and [mining/Core input manifest](native-mining-core-links-independent-443-inputs.json)
pin the evidence. This reviewer changed no source or run result.
