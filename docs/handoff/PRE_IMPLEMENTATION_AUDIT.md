# Pre-implementation crate and swarm audit

**Audited base:** `75bc08e016f4c3d624406eb18a497ecf5c0148c4`  
**Architecture:** ELIOT Search 8.4  
**Architecture section SHA-256:** `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`  
**Result:** scaffold corrected; business implementation remains absent.

## Verdict

The first swarm scaffold was structurally sound: it converted the broad source/index/query families
into capability packages, established one-writer ownership, bounded read sets, package-local logical
surfaces and a dependency-safe wave plan. It was not yet implementation-safe because several
load-bearing state owners were missing or conflated and some core packages depended directly on
concrete adapters.

The corrected scaffold has **39 library packages plus 4 binaries**. Every package remains below the
10,000-line review threshold, and no new product authority or storage technology is introduced.

## Findings closed

| ID | Severity | Finding | Closure |
|---|---:|---|---|
| F-01 | Blocker | `expand_handle@1`, ephemeral/durable handle state, expansion authorization and revocation had no package owner. | Added `search-handles`; result projection only requests handles and lifecycle/security code emits invalidations. |
| F-02 | Blocker | C17 owned pin watermarks but no package owned exact retired-point deletion. P07 could not prove “no pinned reclamation.” | Added `search-index-reclaimer`; it consumes pin watermarks and exact committed retired manifests only. |
| F-03 | Major | `search-qdrant-bridge` combined Windows process supervision, secret handling, artifact identity and vendor data-plane calls. | Split `search-qdrant-supervisor` from the data-plane bridge. |
| F-04 | Major | OS-bound secret storage was implicit and reusable provider-binding/Qdrant credential requirements had no independent security owner. | Added `search-os-secrets` with opaque `SecretRef` semantics and plaintext-side-channel tests. |
| F-05 | Major | Source admission evaluation appeared in both `search-safe-reader` and `search-source-registry`. | Added pure `search-source-admission`; reader performs no policy decision and registry persists decision receipts. |
| F-06 | Blocker | Query/lifecycle packages declared direct concrete Qdrant/redb/revision-store dependencies despite the root port rule. | Removed concrete adapter edges from publication, execution, exact, candidate validation and retention; `eliot-searchd` composes ports and adapters. |
| F-07 | Major | Ordinary index reclamation, security purge and CAS retention could be implemented as one deletion path. | Ownership is explicit: index reclaimer = ordinary retired points; retention = CAS lifecycle/purge/restore; live purge fences remain monotonic security state. |

## Package boundaries added

### `search-os-secrets`

Owns opaque creation, resolution, rotation and deletion of OS-user/incarnation-bound secrets. It never
returns plaintext through logs, config, command lines, telemetry or public serialization.

### `search-source-admission`

Evaluates `SourceAdmissionPolicy` against metadata/path/format/sensitivity observations and emits a
versioned decision receipt. It reads no bytes and registers no source.

### `search-qdrant-supervisor`

Owns the exact qualified executable, hash/PID identity, loopback binding, process ACL/Job Object,
bounded restart/quarantine and secret-reference injection. It does not expose Qdrant data operations.

### `search-index-reclaimer`

Consumes committed retired-point manifests and route/epoch watermarks. It deletes exact IDs only after
the watermark permits; broad correctness-path filters are forbidden.

### `search-handles`

Owns ephemeral handle tables, durable source-handle records, TTL/quota/binding state, expansion
authorization and invalidation. A handle never grants access by itself and can never durably target
unsaved bytes.

## Dependency corrections

The following packages now consume vendor-neutral ports rather than concrete adapters:

- `search-publication` — `ControlJournalPort` and `SearchIndexPort`;
- `search-retrieval-executor` — direct/index/provider leg ports;
- `search-candidate-validator` — `SourceRevisionStorePort` and live-security state;
- `search-exact` — inventory, revision/readback and cancellation ports;
- `search-retention` — control, object-store, index-admin and handle-invalidation ports.

Only `eliot-searchd` owns the complete composition graph.

## Deferred split triggers

The following packages remain intentionally unified, but their agents must request a split before
implementation crosses the stated trigger:

| Package | Keep unified because | Mandatory split trigger |
|---|---|---|
| `search-publication` | commit and crash recovery are one linearizable state machine | implementation exceeds 9,500 soft target or recovery needs a distinct process/dependency |
| `search-retrieval-executor` | one bounded scheduler/fusion owner | implementation reaches 9,500 lines or optional providers require concrete dependencies |
| `search-exact` | one proof denominator and completeness owner | regex/structural engine requires replaceable vendor dependency or package approaches 8,500 lines |
| `search-retention` | one monotonic CAS/purge/restore policy owner | backup provider or CAS implementation becomes independently replaceable |
| `search-safe-reader` | one baseline no-execute acquisition contract | filesystem/Git backends require incompatible unsafe/native dependencies or package approaches 6,500 lines |

## Implementation status

```text
architecture coherence: reviewed
crate ownership: corrected
one-agent/one-crate instructions: present
machine-readable package graph: updated
Rust business implementation: absent
runtime tests: not executed
Qdrant qualification: not executed
Windows process/security proof: not executed
performance evidence: absent
product acceptance: not accepted
active implementation authorization: P00 only
```
