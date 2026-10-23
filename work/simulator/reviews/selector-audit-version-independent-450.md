# Independent historical audit version review, cycle 450

The new mining auditor correctly rejects reuse of the old run 441 pool join. This is an incompatible verifier identity, not a new mining failure or pass.

The [independent exact comparison](selector-audit-version-independent-450.json) found three differing join fields. The recorded verifier changes from `ddf6362c881515f4c6a5cf0e639c50bc938b4fb483e7667aaa31a246899fe48a` to `0e5ae443fa21a6dbb2758750263e6cd28d53117c0f08b807c9ad2b65a8a8a3bb`. The new join also includes image verifier `3bd7e5de2834f546b1c1ed29d5cca8e204007d799c7aa526092a5f4741222258`. Those identities match the exact frozen source files. The third difference is the later outer result. The existing comparison already sets that field to null because the original join was retained before the outer process ended. After this documented normalization, only the two verifier identity changes remain. All other join fields are exactly equal.

The failed command stopped at the unchanged strict join comparison. It did not create `regtest-audit450.json`. The new rates command independently verified the current image-selection path and rate inputs, then wrote its own report; it does not grant new mining/Core acceptance. The historical 443 audit and Core reports still match their recorded hashes. They remain evidence for frozen 440. No historical evidence or assertion was changed.

Fresh run 451 must capture and audit with the same frozen 450 verifier identities. This review ran no build, firmware, VM or auditor. It only read source, report and log bytes.
