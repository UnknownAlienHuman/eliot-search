# Sources — one retained-body path

S1 connects one explicitly selected local file through a stable opened-handle read, immutable plain storage, and a guarded current source row. The CLI reports the source identity, ObjectRef, native file key, revisions, and verified retained readback only after the row commit. Exact-source gates and native coverage are recorded in the [work register](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

The connected operation is:

explicit file selection → read-only source-row capture → one bounded same-handle read → plain ObjectRef publication and byte readback → one source-row CAS in control/control.redb → factual CLI result

The normal operator route uses an already-initialized root and an operator-owned local Qdrant endpoint:

    eliot-search index "C:\example\repo" --data-root "C:\example\root" --qdrant-url http://127.0.0.1:6334
    eliot-search index --data-root "C:\example\root" --qdrant-url http://127.0.0.1:6334
    eliot-search search --query "identifier" --data-root "C:\example\root" --qdrant-url http://127.0.0.1:6334

index PATH admits and refreshes only that File/Tree. A new process's bare index reloads all admissions and
refreshes their unique union; explicit File wins over Tree ignores. It uses typed retain/preparation/publication
helpers under one guard/deadline, without UUID assembly. Tree walks exclude .git/data-root paths; links and
incomplete observations refuse refresh. No admissions means INDEX_SELECTION_REQUIRED; old retain-file rows
are not promoted. The lower-level commands remain available:

The actual retained-file commands, using an already-current root, are:

    eliot-search retain-file --file "C:\example\input.txt" --data-root "C:\example\root"
    eliot-searchd --retain-file --source-file "C:\example\input.txt" --data-root "C:\example\root"

These are placeholder paths. The CLI uses the existing owned daemon child; it does not open the source itself.

The source read is capped at 8 MiB. A real empty file is retained as a zero-length object. Missing files, unsupported native identity, changed handles, corrupt objects, unsupported control schemas, and stale CAS outcomes are errors.

Index exclusion resolves data-root/candidate filesystem paths for containment. This does not canonicalize or
replace stored native locator identity/SourceId. Catalogue writes/preflight and publication reads enforce the
existing count/byte bounds, including locator bytes; bounded legacy reconciliation is not a live query repair.
The Unix OsString import correction alone is not Unix runtime qualification.

## Identity and current state

- Native retention generates SourceNamespaceId from the operating-system random source on the first successful source commit and independently generates SourceId. Served-baseline import preserves the archived portable identities.
- The locator index maps native path bytes to SourceId; the path never becomes the ID.
- NativeFileKey records the opened file's platform identity. It is neither SourceId nor content identity.
- ObjectRef uses format version, source kind, exact byte length, and the maintained BLAKE3 digest.
- The current row and registration revisions guard stale completion atomically. Replacing a head never deletes its old immutable object.
- The original acquired buffer supplies the object writer through a byte slice. No mutable source path is reopened for retention.

Import preserves served Source/preparation revisions, refs/settings/outcomes, including a valid older input binding.
Imported NativeFileKey is historical evidence, not proof of a live destination file; no File/Tree admission is
created. The [import consumer](../search-runtime/search-retention/FUNCTIONS.md#explicit-restoreimport-into-a-fresh-destination)
uses retained objects and never re-prepares merely to copy the baseline.

A stale source-row or registration guard preserves the newer logical current row and its immutable object. The rejected object may remain unreferenced. A stale writable transaction need not preserve physical control.redb bytes. Read-only capture and pre-write observation/refusal are separate boundaries.

read_object uses the addressed retained bytes even after the live source changes. Missing or corrupt objects fail; the reader does not fall back to the live path or recreate the object. S1 native evidence includes a directory-junction refusal; the regular-file symlink case was NOT_RUN because Windows returned privilege error 1314. S1 registration-only stale-CAS coverage uses an external fixture transition, not an implemented unregister/re-add command.

## Retained-only preparation

P1 adds one separate command through the existing owned child:

    eliot-search prepare-source --source-id UUID --data-root "C:\example\root"
    eliot-search prepare-source --source-id UUID --unit 1 --data-root "C:\example\root"

Use the SourceId returned by retain-file. The daemon resolves the current source and selected settings in CONTROL, reads the verified retained Source ObjectRef, calls prepare_utf8, stores the complete Prepared artifact and decodes its retained bytes before guarded current-outcome installation. It emits one complete selected stored unit with actual text/ranges. The original pathname may have changed or disappeared; it is never reopened. Retain-file does not automatically prepare.

PreparationSettings affects output identity; fail-only execution ceilings do not. Settings selection is idempotent and prepare-source preserves an existing selection. Empty UTF-8 produces a complete zero-unit artifact; binary/invalid UTF-8 and selected ceiling refusals are distinct bound Gaps. Missing/corrupt retained objects, stale completion, cancellation and deadline remain errors. A missing/corrupt current Ready artifact is never rebuilt implicitly.

[Unitizer](../search-prep/search-unitizer/FUNCTIONS.md) owns the complete artifact and borrowed decoder; [CONTROL](../search-control-redb/FUNCTIONS.md#current-preparation) owns settings/outcome revisions and atomic expected-outcome CAS.

## Prepared corpus publication

    eliot-search publish-prepared --qdrant-url http://127.0.0.1:6334 --data-root "C:\example\root"
    eliot-search search --query "identifier" --qdrant-url http://127.0.0.1:6334 --data-root "C:\example\root"

The operator starts the selected local Qdrant 1.20.0 service. Q1 captures all retained sources and matching
preparations, publishes actual saved unit text, verifies the backend and checked canary, and installs current
plus a complete validated evidence snapshot. A source/settings/outcome mutation advances the shared non-reset
wanted guard atomically. Reference pages contain no text; publication hydrates each source's saved Prepared
object across adjacent pages per pass and returns generation state/counts, without user-query options.
The separate read-only search command verifies returned evidence and full units against Source/Prepared objects
from the same published snapshot; absent original paths or newer preparation do not replace that binding.
See [publication](../search-index-qdrant/search-publication/FUNCTIONS.md#current-q1-consumer) and
[evidence](../search-query/search-candidate-validator/FUNCTIONS.md).

R1 explicit index/refresh now uses these owners. Identical native key/ObjectRef preserves the logical head;
a changed key updates it even for identical bytes. Only confirmed in-scope absence removes current membership
for new generations; admission/kind, retained objects and pinned evidence remain for later re-add. An ignored
but present file is not removal. Outside rows remain; inaccessible/partial inventory cannot confirm absence.
Unrelated legacy unprepared rows still fail full publication rather than being prepared or omitted implicitly.
Admission/reload and foreground watch are implemented. Watch accepts no PATH and refreshes these saved
selections in one owned Index child at a time, with cleanup before the next interval. R1 acceptance and
unclaimed recovery/convergence/lifetime boundaries belong to the exact scopes in the
[work register](../../docs/audit/ANALYSIS_REGISTRY.md#evidence-status); do not infer universal qualification.

## Ownership

| Responsibility | Owner |
|---|---|
| Explicit no-execute file acquisition and native identity | [Safe reader](search-safe-reader/FUNCTIONS.md) |
| Plain immutable bytes and verified object readback | [Object store](search-revision-store/FUNCTIONS.md) |
| Current source/ObjectRef/settings values | [Source values](search-source-registry/FUNCTIONS.md) |
| Namespace, locator index, source/preparation CAS in control/control.redb | [Control](../search-control-redb/FUNCTIONS.md) |
| Exact UTF-8 and complete artifact/unit ranges | [Materializer](../search-prep/search-materializer/FUNCTIONS.md), [Unitizer](../search-prep/search-unitizer/FUNCTIONS.md) |
| Root exclusion and shared no-clobber publication | [Root](../search-runtime/search-runtime-owner/FUNCTIONS.md) |
| Actual daemon and CLI caller | [Daemon](../../bins/eliot-searchd/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md) |
