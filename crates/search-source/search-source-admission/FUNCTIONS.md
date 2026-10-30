<a id="search-source-admission--source-and-deletion-map"></a>
# Source admission — selection checks, not an approval system

This owner decides whether the selected source may be read and retained. It owns neither filesystem access nor
storage. [Local scope](../../../docs/architecture/LOCAL_SCOPE.md) has no users/roles, grants or approval service.
These are implementation instructions; the legacy caller has not been replaced by this document.

<a id="target-data"></a>
## Smallest useful decision

Use the selected file/root, current exclusion settings and actual observations already available to the source
owner. Pass only facts used by a real check; no universal location/content/security classification schema.
Stage 2 needs one explicit file, not a root inventory, policy registry or batch evaluator.

| Condition | Action |
|---|---|
| Outside selection, configured exclusion, known private-key/credential/system exclusion, or application-owned state | Refuse/skip before body I/O with the actual reason. Explicit file selection does not override an exclusion. |
| Unsupported file kind or format | Report unsupported at the boundary that knows it; do not fabricate empty content. |
| Unknown required fact, I/O failure, unstable read, cancellation or exhausted work budget | Stop or report incomplete work; do not mark the source deleted or permanently unsupported. |
| Eligible metadata | Permit one bounded stable read; metadata size does not replace an actual read limit. |
| Actual bytes satisfy the selected checks | Pass the same owned body to retention, preserving the acquisition observation. |

A former `ReviewRequired` outcome is a refusal with a reason, not a new approval queue/token/workflow. Changing
selection or an exclusion is an explicit settings operation; unresolved facts never become an automatic allow.
Compile configured patterns once when used, with the existing maintained matcher. Do not duplicate the ignore
parser or matching context owned by [reconciliation](../search-source-reconcile/FUNCTIONS.md).

<a id="direct-ingestion-sequence"></a>
## One file to retention

```text
capture selection/settings and current source guard
→ check known exclusions, kind and declared size
→ stable acquisition under the operation's actual deadline/cancellation/byte cap
→ check actual length and any byte-dependent exclusion
→ shared immutable writer receives the same owned body
→ short guarded source-head update
```

[Acquisition](../search-safe-reader/FUNCTIONS.md#filesystem-byte-handoff) owns selected-root containment, supported
link handling, before/after observations and the single opened-file read. Admission does not build an ancestor-
handle graph or choose the manifest/object publication primitive. Use execution-only state; no query view exists yet.

The post-read decision may narrow permission, never reverse a pre-read refusal. Supported zero bytes are a real
input, not a missing source. Decoding, encoding support, maps and units belong to
[preparation](../../search-prep/search-materializer/FUNCTIONS.md); do not decode/parse the whole file twice to
produce an admission classification. A failed read does not become a successful zero-length revision.

[Object I/O](../search-revision-store/FUNCTIONS.md) owns actual content identity and publication/readback.
[Source rows](../search-source-registry/FUNCTIONS.md#current-state-and-conditional-application) own the non-reset
guard and conditional installation. Neither receipts nor equality of content alone authorizes a stale update.

## Cancellation and effects

A refusal before acquisition performs no body read or object write. Cancellation stops further work, but once
object publication or a metadata commit may have happened, cancellation is not proof of rollback. Resolve the
outcome at that effect owner. Do not retry a stale body as a new source observation.

A published object whose head update failed may remain unreferenced. Only
[stopped-root maintenance](../../search-runtime/search-retention/FUNCTIONS.md#stopped-root-local-object-collection)
decides final-object reachability; admission does not delete it or invent an online collector.

## Persistence

Store only current selection/settings, a useful outcome/reason and the source binding actually needed by the
consumer. Reuse the existing guarded source row; no separate admission database, original-input archive,
matched-rule transcript, fingerprint or completed-operation history.

Keep refusals diagnosable. The local CLI may identify the selected source through its
[safe display boundary](../../../bins/eliot-search/FUNCTIONS.md#untrusted-display-text); do not dump file contents,
credential values or full rule/config payloads. One bounded error mapping is enough.

<a id="current-connected-call"></a>
<a id="useful-current-functions"></a>
## Source seams

Inspect `native_source.rs::acquire_source_v2`, `policy_v2`, `evaluation_v2`, `OriginalAdmissionV2` and their real
registry/object consumers. The [earlier source inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/2aa65104d903200806decec83c4713389608ba15/crates/search-source/search-source-admission/FUNCTIONS.md#current-connected-call)
records the body-handoff defect and receipt/archive chain. It is an inspection starting point, not a target API
or a fresh caller census. Follow actual dispatch before changing code.

## Delete with callers

Move useful exclusions and byte/kind checks beside acquisition. Remove original observation reconstruction,
receipt/provenance packages, v1/v2/batch compatibility, v4 registry restoration and their exclusive fixtures with
the converted consumer. Do not manufacture a public permit type or an `admission-v3` facade. Delete this crate
when only small source-local helpers remain. Preserve user files/keys and unsupported old roots.

## Verification

After code run the [minimal production closure](../../../docs/implementation/WORKING_GUIDE.md#minimal-production-checks)
for the actual source/reader/object/daemon consumers. Later connected cases cover exclusion before I/O, explicit
selection not bypassing exclusion, zero/oversized/changing files, actual read limits, same-body retention, stale
head refusal, cancellation on either side of publication, truthful gaps and bounded safe diagnostics. No new
review/receipt matrix or broad format suite is a prerequisite.
