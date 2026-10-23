# Independent milestone status review 431

Milestone 2 is **incomplete**. Its recorded acceptance is one complete **exact
target and its remote workflow**. Completing the adapted S9 SSH/TLS path is an
intermediate result within that milestone. It does not change the acceptance
criteria. No exact physical target has passed all gates.

At this review, basic access run 431 is active and has no assessment. Run 421
failed before HTTP; its result remains failed. Startup test 426 fixes and tests
the real service launcher, with scripted provider replies. It is not a native
HTTP/TLS pass. The separate browser run has not yet passed this new path.

| Gate | Current exact-target status and remaining action |
| --- | --- |
| G0 Identity and preparation | Open. Resolve the selected FPGA source/build association or validate a complete reconstruction. Resolve controller PCB, RAM/storage, ordered hashboards, ASIC, PSU revision and cooling identities. |
| G1 Execution | Open. Validate the original reset and boot/storage path on that resolved model. Archived SPL, U-Boot, kernel and miner execution in the adapted reference environment does not close this gate. |
| G2 Enumeration | Open. Validate topology and component models, including absent and wrong-identity devices. Three reference chains of 63 chips are not exact-board evidence. |
| G3 Functional work | Open for the exact target. Adapted run 391 passes two fresh jobs, independent verification and two Core acceptance/restart checks. Repeat the required arbitrary/fixed work, rejection, transition and Core tests for the final exact execution identity. |
| G4 Management and UI | Open. Complete selected SSH tunnel and authenticated TLS tests, then the explicit browser run. Verify login, assets, live data, configuration change/readback, restoration and restart persistence on the same final identity. Local adapted browser passes remain separate. |
| G5 Fault response | Open. Run interface-level protocol, reset, queue, sensor, power, thermal and shutdown faults against the exact contracts. Component tests do not establish complete target conformance. |
| G6 Update and recovery | Open. Validate the exact baseline and all applicable update/recovery edges, including wrong target and interrupted writes. Earlier adapted signed-update and persistence passes cannot be inherited by changed identities. |
| G7 Timing | Open. Obtain revision-identified physical traces with clock uncertainty; compare model timing and error bounds. Keep low-rate work timing adaptations separate from ordinary buses, boot and protection timing. |
| G8 Physical behavior | Open. Obtain measured revision-specific power and thermal data. Supplier specifications and assumed fixtures do not establish calibration. |
| G9 Isolation and reproducibility | Open for the full exact workflow. Keep the passed transport, isolation and retention components; complete selected-target remote access, reconnect/loss/recovery, state/persistence and cleanup cases. Freeze and independently review the final tested state. |

The immediate exactness dependency is substantive. The retained public FPGA
candidate differs from the selected image in 1,964 of 5,144 checked frame ranges.
That is not a source match or proof of incompatibility. The matching build
history or reconstruction databases and the physical reference inputs remain
missing. Model work that depends on them remains implementation work as well.

No bounded **milestone completion estimate** is defensible from current data.
The measured figures support only execution-phase estimates:

- Shared adapted run 391: 748,209 ms, or 12.47 minutes, for the outer execution.
- Failed access run 421: 829,296 ms, or 13.82 minutes, for the outer execution.
- Current basic run 431 has a 900-second outer wall budget. Preparation,
  independent audits, Core checks and fixes are additional work.
- The optional browser profile has a 1,200-second outer wall budget. This is an
  explicit resource ceiling, not a measured completion estimate.
- The earlier four-boot adapted milestone 284 took 33.72 minutes in total. Its
  different scope and frozen inputs do not predict completion of milestone 2.

It is reasonable to describe the next basic outer test as roughly a quarter
hour, based on the two measured attempts. It is not reasonable to describe
milestone 2 as one test or a fixed number of hours away. A percentage-complete
claim is also unsupported.

The milestone and README completion claims remain appropriately incomplete.
Historical v11 paragraphs in `progress.md` describe older limitations; the
linked continuation and recent reports govern current component status. The
frozen matrix still has 79,599 rows: 71,437 unknown, 1,377 blocked and 6,785 not
applicable, with zero executed or passing exact targets. Full-matrix
`--require-complete`, complete clean-install packaging and release-wide
conformance are later full-task gates; they are not closed by milestone 2.

This was a read-only review. No build, VM, source edit, catalogue change or
result reclassification was performed. The adjacent manifest pins its evidence.
