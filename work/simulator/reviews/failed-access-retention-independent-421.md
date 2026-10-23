# Independent failed-access retention check 421

The selected access test failed. Bubblewrap rejected `--uid` without a user
namespace before the dashboard service started. The client report and overall
assessment both remain false. Browser execution was not requested.

The failure did not discard the run evidence. The independent
[byte and receipt check](failed-access-retention-independent-421.json) confirms:

- Both retained native result copies contain 105,141,709 exact bytes with
  SHA-256 `c5efb13b5978f36c6a8793e43a9073aa0485ba6cef7104b180ecd896e47e247a`.
- The pool bundle contains 107,513 bytes and the failed access bundle contains
  448,850 bytes. Their identities match the joined receipt.
- The actual recorded runtime stdin acknowledgment and the guest's joined
  receipt match the native, pool, and access identities. Host and worker native
  receipts agree. Cleanup names the same run and exact native result.
- All 24 extracted access files and ten pool session files match both their
  declared hashes and the exact bytes inside the retained bundles. Pool stdout
  and stderr match too. The original Bubblewrap error is retained unchanged.

This is a failed-access retention result. It is not a dashboard success, a
mining correctness audit, or Bitcoin Core acceptance evidence. The outer QEMU
wrapper exited with code zero; that does not override the failed selected test.
The full simulator completion gates remain open.
