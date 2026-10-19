# search-source-provenance — legacy archive scheduled for deletion

This package reconstructs complete pre-release admission policy/observation/decision/receipt inputs for old
registry/source-record formats. It is not current source authority and is not needed to retain a local file.

## Verified source and callers

Production source is about **57.1 KiB** before the 9.7 KiB package test file:

```text
admission.rs       13,001 B
codec.rs            3,986 B
fragments.rs       12,613 B
lib.rs              1,183 B
policy.rs          20,088 B
storage_proof.rs    6,276 B
```

`OriginalAdmissionV2` is used by:

- the current native source acquisition path;
- source-record provenance/request/v4 batch codecs;
- daemon registry-control original-input/v4 restore paths;
- package and compatibility tests.

Those consumers exist to reproduce old input archives and operation history. The direct current source path
requires none of them.

## Current replacement

```text
bounded current admission policy
→ small before-read decision
→ exact stable body read
→ small after-read final classification
→ immutable ObjectRef
→ typed current SourceRecord / RevisionRef
```

Store only the policy revision, final outcome/reason and classification if current diagnostics need them.
Equal pure inputs may be reevaluated. Restart reads current root/source/head rows; it never reconstructs
complete original policy arrays or admission receipts.

## Delete after #270 cutover

- `OriginalAdmissionV2`, `RetainedPolicyV2` and `OriginalInputsLayoutV2`;
- archive fragment codecs, canonical digest/preimage and storage proof;
- `ARCHIVE_V2.md` profile and byte compatibility;
- source-record v4 provenance/request/batch codecs;
- daemon `registry_control/original_inputs*` and v4 restore paths;
- package fixtures and workspace/dependency/status/ledger entries.

Move no forwarding facade. Unsupported old roots are refused before writes and left intact. Preserve #598's
active private work; consume only its accepted retained-source result and do not create a competing writer.

See the [current admission map](../search-source-admission/FUNCTIONS.md),
[source registry map](../search-source-registry/FUNCTIONS.md) and issue
[#270](https://github.com/UnknownAlienHuman/eliot-search/issues/270).
