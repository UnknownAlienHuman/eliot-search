# ELIOT Search — architecture for the local baseline

**Decision date:** 2026-10-18. **State:** refactoring target; the implementation has not cut over.
This decision follows the maintainer's request to challenge the architecture, compatibility and security
machinery that displaced working search. It replaces the baseline obligations listed below in
[Architecture 8.4](ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md) and older package/issue instructions.
Evidence: the [source audit](../audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) and
[donor index](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md).

## 1. Product and trust boundary

Build a standalone analysis and retrieval tool for one OS user, explicit local source roots and local
CLI/agent clients. Help agents find implementations, inspect exact evidence, orient themselves in
repositories and compare sources without reading entire corpora. Large repositories and portfolios
remain the mission under ADRs [0005](../adr/0005-standalone-search-product-and-controller-boundary.md)
and [0006](../adr/0006-agent-analysis-framework-product-scope.md). A model is not required for search.

The baseline trusts the owning OS account. An agent's requested corpus is an explicit search scope;
a binding UUID does not create another OS security principal. Clients restricted below that account's
authority need a separately implemented restricted-client profile. The baseline does not promise
protection from arbitrary malicious code running as its owner.

Retain explicit source selection, owner-only application storage and local IPC, source containment,
known credential/private-key exclusions, bounded reads, no execution of indexed code/macros/resources,
private Qdrant credentials and exact source readback. Never silently ingest the user's home directory
or widen a requested corpus.

## 2. Five responsibilities, one application

These are module responsibilities, not five new crates or agent assignments.

| Responsibility | Smallest useful owner |
|---|---|
| Sources | Registered roots, file identity/path, immutable byte snapshots, inventory and coordinate maps. |
| Preparation | Existing materializer, unitizer and lexical profile; deterministic input/output with explicit gaps. |
| Index publication | One Qdrant adapter and one control transaction publishing the active route/epoch. |
| Query | Resolve scope once, pin a snapshot, retrieve bounded candidates, reopen exact bytes, return locations/snippets. |
| Runtime | Capture invocation/config once, hold the root lock, own IPC/Qdrant child lifetimes and cancellation. |

Qdrant remains the only indexed retrieval backend. redb holds small control records, never a searchable
corpus. Files hold immutable source/preparation objects. DIRECT remains an explicitly named exact scan
path when an index is unavailable or an exact predicate is requested.

Keep vendor/OS code at actual effect boundaries; call pure transformations directly. Traits are useful
at storage/transport/source I/O seams. Package-name capabilities, generic receipts and port conformance
catalogues are not prerequisites for every function. Merge packages after deleting duplicate mechanisms
when no independent dependency/runtime boundary remains. C00–C30 is not an obligation to instantiate
thirty separate owners.

## 3. Finish this path first

1. Register a local root; capture a stable file through the existing contained reader. Retain admitted
   bytes and enough metadata to reopen the exact revision.
2. Run existing preparation/lexical code, produce point IDs/manifests and upsert bounded batches through
   the real Qdrant client. Publish the verified route/epoch in one control transaction.
3. Accept bounded text and explicit scope through the real CLI/provider entrypoint. Resolve scope, pin
   the route/epoch, retrieve, validate against manifests and retained source, and return relative location,
   exact range, snippet, revision and freshness/gaps.
4. Demonstrate edit, incremental update, restart, cancellation and index loss through those entrypoints.
   Rebuild projections after index loss, and the collection generation after a schema/profile change.
   Ordinary file edits must not require rebuilding the whole corpus.

Do not implement another recipe registry, runtime, client, S11 algorithm or preparation engine before
using what exists. `BlockingRealQueryPlane` already owns a reusable Tokio runtime per plane instance. The #213 evidence and
orientation schemas exist in `search-contracts::evidence` and provider `evidence_schema`. The gap is
real caller/storage/query composition, not those mechanisms' existence.

Preserve the existing #598 manager and unpublished work; reuse its useful held-source/revision output.
This decision does not reset its branch or authorize another source writer. Additional key, transfer
and lifecycle mechanisms in the old programme no longer gate the local baseline.

## 4. Source truth without a source-ownership platform

Use one local data-root namespace. A source occurrence records root/file identity and path; a revision
identifies immutable bytes by a real digest and length. A compact inventory/manifest binds revisions to
a published generation/epoch. Never read current file B and label it revision A.

Retain snapshots needed by published results and live request/continuation pins. Missing historical
bytes produce unavailable/stale results, not reconstructed quotes from Qdrant. Long-term history,
durable exported handles and legal holds are separate features with explicit retention behavior.
Original files and retained revisions that may be the only copy are user data; projections are rebuildable.

For new local storage, owner-only ACLs, immutable objects, digest verification and atomic publication are
the baseline. Revision encryption is an explicit storage feature for a real confidentiality need, not a
prerequisite for indexing already readable local files. Existing encrypted stores must not be silently
downgraded or lose keys. If selected, use one actual writer/reader and the existing OS secret/AEAD
libraries; do not maintain parallel DPAPI, unused DEK-selection and unconsumed AEAD pipelines.

Remove the six independent scope/access/confidentiality/key/retention/erasure domains and bilateral
provider ownership transfer from the default local contract. A restricted storage/transfer feature
introduces only domains it enforces. Do not trick old validators with six synthetic equal UUIDs or a
fixed permitted observation: remove the obsolete requirement and change its producer/consumer together.

## 5. One root owner and a short recovery path

Hold one OS file lock on a stable file for the root owner's lifetime. Use redb transactions for
installation/format, active route/epoch and pending publication. Retain actual root/file identity checks
against wrong-root/path substitution. Remove the second sealed-owner lock with the last old writer;
it is not needed when only one implementation can open that format.

Do not bind ordinary root admission to every daemon executable hash or a two-slot owner succession
chain. Those serve old-layout/upgrade continuity, not authentication against the same OS account.
Use a format version and transactional state. For Qdrant, own the selected child, verify the installed
artifact at installation/upgrade, and check actual endpoint/process and server/schema capabilities at
connection. Do not attach to an arbitrary process because its port responds.

Publication needs one commit boundary: staged Qdrant effects/manifests become visible only when the
control transaction advances the route/epoch. A crash before it leaves staging to reconcile; a crash
after it reopens committed state. Retain existing interval/pin semantics for incremental publication
until their replacement proves equivalent behavior. Alias movement alone is insufficient. Durable
operation identity belongs to effects needing retry/unknown-outcome recovery, not every pure function/read.

For incompatible pre-release formats, create a new root/collection generation and rebuild from available
sources. General staged migration/rollback is not the default. Leave the old root untouched. Preserve
sole retained copies, keys, configuration and required history through a specific offline export/import;
otherwise report reset of that data as unsupported. Reject unknown formats clearly. Never reinterpret
old IDs, auto-delete old roots or claim that a staging receipt completed serving cutover.

## 6. Retrieval and exhaustive verification

Ordinary ranked retrieval returns a bounded candidate page. It makes no complete-negative claim and
requires no exact corpus count, proof ticket or durable query job on each request. Empty top-k means
no candidates returned by that query/profile, not proof of absence.

Use one resolved scope and pinned route/epoch. Eligibility governs retrieval and its IDF population.
When count/scroll/statistics or exact verification are requested, use that same scope. Removing mandatory
`count_exact` from top-k must not remove filtering or source validation. An explicit exact operation
freezes the relevant inventory, records gaps and scans its denominator before making an exhaustive claim. Change `IndexedLegOutput`/`LegCompletion` with the
executor: removing the count call must not leave `CompleteCandidateScope` or a fabricated denominator
on a normal ranked page.

Return bounded relative locations/snippets. Scores remain ranking hints. Add orientation, comparison,
lineage and document coordinates through the same query/result path as consumers become real. Every
v1 recipe and optional result field need not be implemented before returning the first indexed result.
Reuse delivered evidence contracts where they fit; simplify unused mandatory fields instead of creating
another parallel contract family.

Continuations may hold a bounded in-memory result window and snapshot pin with expiry. Restart may
invalidate them. Normal paging needs no durable handle ledger or provider export receipt. Actual source
removal/restriction invalidates affected results/handles before acknowledgement. Garbage collection only
removes unreferenced objects owned by this data root.

## 7. Invocation, transport and resource limits

Capture `args_os()` and effective config once, select one command and pass an immutable invocation.
Reuse existing parser/Serde mechanisms. For child stdio, inherited handles identify the connection.
For persistent Windows pipes, enforce the owning OS account and local-only endpoint. Use one framing/
session implementation. Pairing, delegated grants, replay proofs and capability negotiation belong to
restricted-client features when needed, not an imagined boundary against arbitrary same-user code.

Use one absolute deadline converted once at admission, one cancellation signal and concrete limits for
queue depth, file/byte reads, candidate batches and output. RPCs/pages receive remaining time or an
absolute `timeout_at`, never a renewed duration. Bound allocation before reading/parsing; throttle
background work when it would starve interactive queries. Counting every OS call, canonical byte and
generic port preflight is not the resource model. Add counters for actual scarce/unbounded operations.

Errors identify the operation, affected relative path/config field and actionable cause within the
user's scope. Do not log credentials or source/query bodies by default. Content-free receipt codes
alone are inadequate operator diagnostics.

## 8. Requirements retired from the local baseline

| Earlier obligation | Current disposition |
|---|---|
| 8.4 S6.3 six-domain CAS; S7.2.1 namespace transfer | One local namespace and snapshots; restricted residency/provider transfer optional. |
| Mandatory key candidate/current/rotation/usage chain before ingestion | Encryption optional for new local storage; preserve existing encrypted data and use one consumed implementation. |
| General legacy journal migration/cutover/rollback staging | New-format rebuild; specific non-destructive export for actual retained data. |
| Owner executable continuity and two-slot succession | One OS lock, root identity and transactional format/state; retire old writer before removing its co-lock. |
| Pairing/delegated grants/proofs for every same-user client | OS-owned connection and explicit scope; restricted clients add actual delegation. |
| Exact denominator/proof machinery on every ranked query | Bounded non-exhaustive retrieval; exhaustive verification explicit. |
| S26 durable handles and S28.2 paired accelerated restore as prerequisites | Ephemeral pins/cursors and rebuild-first recovery; durable export/backup optional. |
| Executed test receipts in `RealDataPlane::connect` via `QualifiedGate` | Keep actual endpoint/owner binding, health/version/build, API authentication and separate schema verification. Move live test reports to evaluation/release; do not fake a gate. |
| Wave-1…7 feature ladder and optional lifecycle/proofs in `full-baseline` | One supported default application; optional features follow real dependencies. |
| Frozen pre-release wire bytes/canonical encoding for every internal record | Change real ends/version together; reuse codecs or library serialization. No silent stored-data reinterpretation. |
| Per-site digest ledger, issue snapshots, committed maps, source-shape tests and custom Rust import/module taint analyser | Remove as merge requirements; use Cargo dependency data, Rust visibility, API review and behavioral checks. Do not build a second compiler. |
| Capability quotas, second-commit scope freeze and prescribed review forms | Coherent implementation/deletion in a reviewable change; review behavior and remaining risks. |

These are product decisions, not exceptions needing waivers or another registry. Older documents may
explain retained code but cannot restore these baseline obligations. Update the touched owner contract
when implementing its change. Preserve a concrete external consumer or retained-data requirement when
one exists; an old ticket or historical fixture alone is insufficient.

## 9. Evidence of a useful implementation

Through the real entrypoint: index a source, retrieve a relevant hit, reopen exact bytes, then exercise
update, restart, missing index, cancellation and scope exclusion. Check changed production code and
consumers with pinned compiler/Clippy; test behavior after integration. Measure live Qdrant, Windows
behavior and performance before claiming them.

Qualification reports are not startup credentials. File-name/token/line-count assertions, committed
maps, source hashes and issue states do not demonstrate search. Keep tests/tools that catch concrete
product regressions. No fixed LOC promise substitutes for deleting a mechanism and its consumers.
