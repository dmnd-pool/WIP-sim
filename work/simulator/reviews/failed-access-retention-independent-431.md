# Independent failed-access retention check 431

Run 431 remains failed. The native HTTP checks passed, then the selected TLS
listener readiness check failed. No TLS request ran. Browser testing was not
requested. A startup race is a hypothesis; the single failed connection did not
retain its operating-system error.

The failure preserved the exact run evidence. The independent
[byte and receipt check](failed-access-retention-independent-431.json) confirms:

- Both native result copies contain 105,718,948 bytes with SHA-256
  `e3aac175299649271bd985c54fd173541864d134c0c5f6bed46bf94776403cb1`.
- The pool bundle contains 98,537 bytes with SHA-256
  `11be68bd9a2838456bab25fda1ccb945553d8bc3d9155b5ee9e9751bb490f52b`.
  The failed access bundle contains 5,516,831 bytes with SHA-256
  `d8d56b446cbc8a9b368dce25237d9570dc716ed86f6f1773243dc88040e47b9b`.
- The actual recorded runtime stdin acknowledgment, guest final joined receipt,
  host receipt, worker receipt and cleanup identity match the selected token,
  run, lock and exact artifacts. Cleanup confirms removal and retained token
  receipt. The guest final record equals the record retained in the assessment.
- All 181 extracted access files and ten pool session files match both the exact
  bundle bytes and their declared hashes. Pool stdout and stderr match too.
  The failed access report and its observer copy agree.

The existing `complete_result_exported` summary field is false because its
expression also requires the overall observation to succeed. This review checked
complete result retention from the actual bytes and receipts, independently of
that combined summary field. It does not change the failed assessment.

The outer QEMU process exited with code zero in 834,098 ms with untruncated
traces. That is not a selected test pass. This review grants no TLS, browser,
mining/Core, exact-board, remote-loss or full simulator completion claim.
The [input manifest](failed-access-retention-independent-431-inputs.json) pins
all retained evidence checked here. No source or result was changed.
