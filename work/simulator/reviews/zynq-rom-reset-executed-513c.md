# Public Zynq ROM reset attempt513c

The UBSan backend build passed. The new ROM-map test failed before QMP startup. No original-board or ROM-boot gate passed.

The actual build exited0 after182633ms. The existing fabric-reset regression passed. The new ROM mode failed because its global ROM region used a MachineState owner. The exact QEMU `memory_region_register_ram` implementation accepts a DeviceState or null. The first retained diagnostic reports that cast failure. It is not a guest fault or evidence of incompatible firmware.

[The input report](zynq-rom-reset-executed-513c-inputs.json) binds the frozen helper, build, results and raw streams. [The defect record](zynq-rom-owner-failure-513c.json) binds the exact upstream source and correction. [The independent source review](zynq-rom-owner-independent-source-513d-root.json) closes the one-line correction. The next513d run uses null ownership for the global ROM, with all mapping and test assertions unchanged.

The inherited `rom_verified`, `pcap_verified` and `uart_verified` assessment fields are conditional validators. They can be true when that case is not applicable. They do not mean those interfaces ran. This attempt exercises the existing fabric test and attempts the new ROM-map test only.

The public ROM remains a reference input. Its author identifies XC7Z010-CLG400A. No evidence binds its exact bytes to the selected S9 silicon revision. The complete reset path, physical timing and mining gates remain open.
