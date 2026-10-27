# Fresh prepared inputs 498

The new preparation helper passes its component checks and a real copy of
prepared input 494. This check does not execute firmware or grant a mining result.
The fresh CLI run remains a separate test.

The helper copies the verified input store to a private staging directory.
It uses the existing pool preparation function to read 32 new random bytes
and the current host time. Only the pool seed and time change. It preserves
the pool schema, connection limit, difficulty, duration and regtest anchor.
It changes only the pool object in the process specification.

The receipt joins both process and pool identities, the complete parent run,
and the helper binary. The execution coordinator checks the complete parent
contract against the unchanged child inputs. It does not trust an arbitrary
parent policy. The receipt and its parent files enter the locked input set.
A repeated prepared input remains a replay, not a new random-work test.

Receipt reads use a bounded regular file descriptor. They reject symlinks,
hard links and named pipes. The helper publishes its output with an atomic
no-replace rename. Errors remove only its private temporary directory.
It rejects a second derivation from an already derived input. The supported
pool schemas are 7 and 8. Preparation and coordination require the same pinned
isolation tool identities.

The final affected test run passed 148 tests with zero failures. Six existing
integration tests were ignored. The new helper passed 14 tests. Strict scoped
Clippy and the three affected release builds passed. The first compile found
a test-fixture syntax error. The first Clippy run found a duplicate module.
Both were fixed, and their failed logs remain saved.

The source review found two unsafe metadata reopen paths. Both now use the
strict file reader and compare the expected identity again before retention.
The regression tests also reject changed policies, deadlines, parent claims,
unbound receipts, derivatives, and destination overwrite.

This command was executed successfully:

```bash
work/simulator/fresh-tools-498/s9_fresh_prepared \
  work/simulator/qemu-s9/native-browser-prepared-494 \
  work/simulator/qemu-s9/fresh-prepared-498
```

The output now exists. Use a new path for another preparation.
An independent byte comparison checked 51 original files and 46 child input
mounts. The process changed only its pool mount. The pool changed only seed
and time. All adaptation, console and support metadata stayed unchanged.

See the [input and source identities](fresh-preparation-498-inputs.json) and
the [executed preparation audit](fresh-preparation-executed-498.json).
The next steps are the helper freeze 499, CLI manifest promotion, full current
source checks, and the complete fresh CLI 500 test. The original-board and
exact-target gates remain incomplete.
