# Independent core0 formatter review 472

The source and actual execution checks pass within the stated function scope.
No unresolved correctness finding remains in this change. The
[byte and callback review](t17-core0-independent-472.json) records the checks.

The reviewer parsed both ARM ELF load tables independently. The original
1,024-byte formatter and all four literal ranges match at their original
virtual addresses. The oracle mappings are readable and executable, without
write access. The complete original miner hash matches its archived identity.

The reviewer parsed all 5,192 output cases and 77,574 callback records. All
256 possible data bytes occur. All 5,152 actual header calls and 15,474 field
calls agree with the original field contract. These totals include both log
destinations. The model-comparison counts in the execution report select only
the primary destination. The independent counts therefore differ as expected.
The review also observes 120 null close calls after controlled open failures.

The execution stdout and executable match their retained tar members.
Compilation used the undefined-behavior sanitizer with traps. Compiler and
execution stderr are empty. The actual process exited zero without truncation.
Source review checked the ARM argument layout, exact source ranges, callback
bounds, reset of the test inputs and strict raw readback context checks.
Two incorrect callback counts in parser tests were fixed before execution.
The original callback expectations were not changed to fit the result.

This validates an original formatter and a raw-field view. It does not prove
ASIC mailbox completion, reset values, clock effects or physical core mapping.
The live model still rejects the unsupported core write. The original archive
and earlier failed firmware run remain unchanged.
