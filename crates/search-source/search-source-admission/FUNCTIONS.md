# search-source-admission — source map

The [implementation guide](../../../docs/implementation/README.md) owns the connected ingestion path.
This map distinguishes useful policy code from compatibility/proof machinery.

## Implemented functions and callers

| Source/API | Actual behavior / caller reality |
|---|---|
| `policy_v2::compile_policy_v2`, path matching | Compiles the bounded reviewed GlobSet policy and matches validated relative locator bytes. Pure CPU work. |
| `observation_v2::AdmissionObservationV2` | Retains bounded locator/metadata observations and computes their existing identity. It proves no containment or read. |
| `evaluation_v2::evaluate_pre_read_v2` | Produces the pre-read verdict. Native source acquisition calls it before opening/reading the body. |
| `evaluation_v2::evaluate_v2` | Produces the full decision from a complete observation. Current non-test callers route through source-provenance/receipt reconstruction. |
| `batch_v2::evaluate_batch_v2` | Complete bounded multiset evaluator. Search found package/docs callers, not normal daemon ingestion. |
| v1 policy/observation/decision/receipt/batch codecs | Preserve old byte profiles and refusal semantics. They are compatibility inventory, not the target current format. |
| `search-source-admission-receipt` | Separate receipt issuer/verifier leaf. A receipt is not native read, retained bytes or current source state. |
| `search-source-provenance` | Stores/reconstructs complete original inputs for historical registry/archive formats. It is not needed to evaluate a current source candidate. |

## Connected operation

The daemon inventory/source owner supplies one bounded candidate observation. Admission performs no I/O:

1. Validate the root-relative locator and metadata that exist before body acquisition.
2. Evaluate mandatory exclusions and path/class rules.
3. Return a closed pre-read outcome.
4. For `AllowRead`, the source owner performs one stable final-handle read.
5. Add only facts that genuinely require bytes: actual size/digest, encoding/binary/content class and
   selected detector/profile results.
6. Reevaluate/finalize; a later fact may narrow but never widen a pre-read refusal.
7. Pass the owned body and final bounded classification directly to revision retention/current source
   commit.

The inventory loop may evaluate candidates one at a time or in a small bounded batch for efficiency. It
does not need a durable canonical batch object, sorted multiset identity or receipt per candidate.

## Keep

- one immutable compiled policy per operation/profile;
- portable validated root-relative locator bytes;
- mandatory deny precedence and explicit review/unsupported/incomplete outcomes;
- finite rule/pattern/input/work limits and caller deadline/cancellation checkpoints;
- content-free errors and diagnostics;
- exact policy/profile revision used by current source state where useful.

## Remove after caller migration

- v1 byte compatibility and its exclusive fixtures from the current product;
- separate receipt issuance/verification for ordinary local ingestion;
- full original-input provenance archive and reconstitution pipeline;
- canonical decision/batch identities and permanent batch result types;
- duplicate configuration/bootstrap/digest paths used only by retired formats;
- package size/fixture allowances and historical AUTHORED_NOT_RUN status prose.

Do not replace removed records with another admission journal. A changed policy causes future candidates or
an explicit reconcile to be evaluated under the new policy; actual current-source removal/update is owned
by ingestion/reconcile.

## Behavior to verify through ingestion

- denied source produces zero body-read calls;
- an absent or malformed locator never becomes allow;
- known secret/private/system exclusions dominate permissive patterns;
- final-handle replacement/race is reported by the reader, not repaired by policy receipts;
- post-read binary/encoding/size facts narrow correctly;
- cancellation/deadline yields no accepted body;
- same accepted source body reaches retention without a pathname reread;
- ordinary errors/logs disclose no path, policy text or source body.
