# Original CVC callback-order failure516

The actual isolated comparison failed. At5ps, Icarus returned sampled0 and the
original compiled CVC returned sampled1. Both used the same C and HDL bytes.
The expected result remains sampled0. The CVC process exited1. The outer run
also exited1 after3,816ms. Its output was complete.

All four compilation steps passed without diagnostics. Six actual fixture,
plugin and program artifacts are retained. Their copied bytes do not preserve
executable mode. They are evidence, not a directly executable replay package.

The [input receipt](cvc-rw-executed-516.json) links the exact helper, specification,
raw result, report and archive. The [independent audit](cvc-rw-independent-execution-516.json)
confirms the failure. No selected FPGA, CPU, firmware or mining gate passes from
this test.

Next: test a separate, recorded change to the existing CVC callback queues.
Keep this failed output and the original two-NBA fixture unchanged.
