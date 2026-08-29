# W2 direct source-spine implementation packet

**Stage / wave:** P03–P04 / W2  
**Status:** `BLOCKED` until the accepted W1 process/control receipt and exact direct handoffs.  
**Goal:** acquire, identify, register, retain, reopen, materialize and unitize admitted local sources
without Qdrant or optional workers.

## Package order

```text
accepted W1 control/config/port handoffs
        ↓
search-source-admission
        ↓
search-source-identity      search-source-registry
        ↓                         ↓
search-safe-reader
        ↓
search-revision-store
        ↓
search-materializer
        ↓
search-unitizer
```

Identity and registry work may overlap only after admission/shared contracts are accepted. The reader
never invents policy/identity. The revision store never reads arbitrary paths. Preparation starts only
from an exact immutable revision receipt.

## One-agent package packets

| Package | Primary packet | Write scope |
|---|---|---|
| `search-source-admission` | `crates/search-source/search-source-admission/FUNCTIONS.md` | package only |
| `search-source-identity` | `crates/search-source/search-source-identity/FUNCTIONS.md` | package only |
| `search-source-registry` | `crates/search-source/search-source-registry/FUNCTIONS.md` | package only |
| `search-safe-reader` | `crates/search-source/search-safe-reader/FUNCTIONS.md` | package only |
| `search-revision-store` | `crates/search-source/search-revision-store/FUNCTIONS.md` | package only |
| `search-materializer` | `crates/search-prep/search-materializer/FUNCTIONS.md` | package only |
| `search-unitizer` | `crates/search-prep/search-unitizer/FUNCTIONS.md` | package only |

Exact function paths are registered in `swarm/function-packets.toml`. The package agent reads only its
assignment, function/config packets, accepted direct handoffs and named fixtures.

## Source pipeline

```text
bounded SourceObservation
→ AdmissionDecision + immutable AdmissionReceipt
→ SourceIdentity + PathBinding/lineage decision
→ admitted root/source/membership/portfolio/source-view state
→ final-handle-contained StableReadReceipt
→ immutable residency-aware SourceRevision
→ CanonicalRepresentation + CoordinateMap + LossMap + assurance
→ deterministic UnitManifest
```

Each arrow is an explicit package handoff. A later package cannot reinterpret an earlier package's
identity, policy, bytes, maps or assurance.

## Ownership boundaries

- admission is pure policy meaning and performs no I/O;
- source identity is physical/logical identity and path/lineage history, never corpus/access policy;
- registry owns roots, admitted sources, memberships, portfolios/views and owner cutover, not bytes;
- safe reader owns final-handle containment and stable no-execute acquisition, not durable storage;
- revision store owns immutable CAS/residency/readback/leases, not arbitrary path acquisition;
- materializer owns baseline text/code representation, maps/loss/assurance, not unit/index/ranking;
- unitizer owns deterministic occurrences/manifests, not lexical/model/Qdrant behavior.

## Core invariants

- deny by default for credential/private-key/system/cache/build/vendor/generated/binary/sensitive classes
  under the exact accepted policy;
- unknown load-bearing policy/observation fields never silently allow;
- paths are locators, not source identity;
- rename/hardlink/path-reuse and case/Unicode behavior are explicit identity transitions;
- nested repositories, worktrees and submodules remain explicit boundaries;
- one source namespace has one active mutable owner generation; old owner is fenced before new activation;
- root/membership/reference scope is explicit—no nearest-repo, implicit HEAD or disk-wide fallback;
- final opened handle/object remains inside the exact admitted root after resolution;
- no hooks, filters, prompts, credentials, shell, toolchains or network execute during reads;
- unstable source retries only within finite budget then returns explicit failure;
- revision bytes are immutable and reopened by exact digest/residency, never current path;
- materialization maps every offset-changing transform and lowers assurance on loss;
- unit ID binds exact revision/representation/profile/occurrence and claims no cross-revision semantics;
- all lists/bytes/depth/attempts are bounded; cancellation never produces false complete output;
- source bodies and unrestricted paths stay out of technical receipts/ordinary telemetry;
- DIRECT source/read/preparation behavior does not depend on Qdrant availability.

## Mutation and recovery

Registry and revision-store mutations use stable operation identities, expected generation guards and
exact readback recovery. Timeout/cancellation after possible commit is unknown until the owning package
resolves it.

Pure admission/identity/materialization/unitization decisions are retry-safe for equal exact inputs. A
changed policy/source/revision/profile under the same operation identity is rejected.

Source-owner cutover follows:

```text
PREPARED
→ OLD_OWNER_FENCED
→ TRANSFER_OR_IMPORT_VERIFIED
→ NEW_OWNER_ACTIVATED_WITH_NEW_GENERATION
→ COMPLETED
```

Ordinary export/copy cannot satisfy this state machine.

## Required W2 tests and evidence

### Admission

- canonical policy/observation/receipt goldens;
- sensitive/system/generated/vendor/binary default deny fixtures;
- unknown-field and forbidden override fail-closed;
- restrictive/permissive policy change obligations;
- no-I/O/dependency and redaction guard.

### Identity and registry

- rename, hardlink, path reuse, case/Unicode, unavailable/reused physical-ID matrices;
- nested repository/submodule/worktree/fork/mirror/ambiguous lineage fixtures;
- explicit portfolio/view and foreign membership non-disclosure;
- membership requires exact current allow receipt;
- owner cutover fence-before-activation and crash/unknown-outcome recovery.

### Safe read and revision

- final-handle versus textual-prefix, symlink/junction/reparse/device/stream/path replacement fixtures;
- file changes before/during/after read become `SOURCE_UNSTABLE`;
- exact local Git object with hooks/filters/prompts/network disabled;
- size/retry/deadline/cancel and batch accounting;
- CAS temp/write/fsync/rename/control-publish/reopen crash matrix;
- residency-domain mismatch, exact readback and protected-root/lease fixtures.

### Preparation

- encoding/BOM/invalid-sequence/newline/Unicode coordinate/loss goldens;
- lossy transform lowers assurance and cannot claim exact mapping;
- deterministic unit boundaries/IDs/manifests and complete range accounting;
- malformed/oversize/cancellation/no-fake-complete properties;
- changed profiles require re-preparation/reunitization/reprojection;
- no optional provider, ranking, model, Qdrant or vendor dependency.

## Hard stops

- W1 or any direct dependency handoff absent/moving;
- source policy/identity/owner/residency/profile field missing;
- reader executes repository-controlled code or escapes an admitted root;
- current path or Qdrant payload substitutes for exact revision bytes;
- identity/CAS key includes corpus/access policy;
- bytes/text/vectors or large point lists enter redb registry state;
- materializer/unitizer silently truncates or overclaims coordinates/completeness;
- baseline W2 selects an optional document/OCR/archive provider;
- one package duplicates another's owner or opens its concrete store;
- package agent edits shared registry, other package or root dependency files.

## Handoff

Every package publishes immutable commit/API/profile/receipt digests sufficient for the next direct
consumer. Integration accepts W2 only after one DIRECT end-to-end path demonstrates:

```text
admission
→ identity/registry
→ stable no-execute read
→ immutable revision admission/reopen
→ deterministic materialization/maps/assurance
→ deterministic unit manifest
```

under crash, cancellation, redaction and no-Qdrant tests. Structural packet validity is not G1 evidence.
