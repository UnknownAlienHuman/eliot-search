# Handle and continuation core audit — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`  
**Audited source:** `7a4471d6505e21f818beca934259b4222710d1dd`  
**Scope:** `search-handles`, `search-continuation`, contract lifecycle records and the legacy daemon handle/continuation catalogs  
**Product qualification performed:** none

## Result

The project already contains canonical handle and continuation packages, but they are **not yet safe integration targets as-is**. The actual daemon still serves legacy plaintext-token catalogs, while the canonical packages retain package-level authority, lifecycle and boundedness defects that must be fixed before cutover.

The accepted sequence is now:

```text
#282 HandleStore canonical/private/bounded model
→ #283 store-owned exact-range verified expansion
→ #274 owner-issued live checkpoints
→ #275 product handle cutover

#284 owner-bound continuation creation/restore
→ #285 atomic prepared delivery + recoverable cleanup + resumable lifecycle
→ #274 owner-issued live checkpoints
→ #276 product continuation/pin cutover
```

PR #118 is closed as packet-only. Its useful observations remain historical; it is not a code branch.

---

## Findings

### F69 — HIGH — Two incompatible canonical handle models

`search-contracts::lifecycle` and `search-handles` define parallel retained/unsaved target and record shapes. The package version uses weaker generic digests for fields that already have typed contract identities and exposes mutable authority-bearing records publicly.

**Risk:** drift, lossy conversion and a third compatibility model during daemon cutover.

**Owner:** #282.

### F70 — HIGH — Handle limits are incomplete and one limit is inert

`HandlePolicy` has active class quotas and `max_expansion_bytes`, but:

- no total retained-record ceiling;
- no maximum ephemeral/durable TTL;
- no finite terminal-history/compaction policy;
- `max_expansion_bytes` is not enforced by canonical expansion.

Expired/invalidated records remain in both indexes indefinitely while no longer counting against active class quota.

**Owner:** #282/#283.

### F71 — CRITICAL — A stale cloned handle record can bypass store invalidation

The canonical API accepts a caller-provided public cloneable `HandleRecord` in free `revalidate`/`expand` functions. Expansion does not first resolve the current record/revision from `HandleStore`.

A caller retaining a clone from before invalidation can continue to present it even after the store record moved or was removed.

**Owner:** #282/#283.

### F72 — CRITICAL — Handle authorization and permits are caller-constructible

`HandleValidationContext`, `authorization_digest` and `HandleExpansionPermit` are ordinary public data. Matching fields do not prove current owner/access/purge/view/residency authority.

**Owner:** #283, integrated through #274.

### F73 — CRITICAL — Expansion does not verify returned bytes

`HandleReadback` echoes expected content/excerpt digests next to `bytes`. Expansion compares the echoed digests but does not hash the returned bytes. It also accepts any nonempty output with `len <= maximum_bytes`, rather than exact selected-range length.

A buggy or hostile port can return wrong or truncated bytes with copied expected digests.

**Owner:** #283.

### F74 — CRITICAL — Continuation payloads are not bound to their referenced objects

A `ContinuationRecord` names candidate-window, issued-set, pin, durable-job and checkpoint references, but ordinary creation does not verify that its in-memory payload is exactly the object named by those references or that all objects bind the same scope/query/plan/ranking/result identity.

**Owner:** #284.

### F75 — CRITICAL — Continuation scope and live state are caller-shaped authority

`ContinuationSecurityScope::new` accepts an arbitrary nonempty membership set. `LiveContinuationState` consists of ordinary Booleans such as `grant_active`, `purge_clear`, `owner_generation_current` and `epoch_pin_valid`.

An omitted influenced membership or fabricated all-true state can appear valid without owner proof.

**Owner:** #284, integrated through #272/#274.

### F76 — CRITICAL — Revalidate, output and state commit are separate

Current flow permits:

```text
revalidate_emission(&self)
→ write bytes
→ commit_emission(&mut self)
```

The permit is cloneable and the store is not exclusively held across output. Another mutation can make the later commit stale after bytes escaped, leaving candidates unissued and replayable.

**Owner:** #285.

### F77 — CRITICAL — Ordinary terminal cleanup obligations can be lost

Completion, final emission, expiry, generic invalidation and live-limit application return cleanup effects but do not retain one pending cleanup state in the record. `compact_terminal` can remove any terminal record whose special security-cleanup field is absent.

Epoch-pin release or durable-checkpoint deletion can therefore be forgotten before external acknowledgement/readback.

**Owner:** #285.

### F78 — HIGH — Restrictive lifecycle operations can make no progress above one batch

Generic invalidation and live-limit transitions can collect more than `max_lifecycle_batch`, return `ResourceExhausted` without mutation and reproduce the same failure forever on retry. Some paths collect/sort the full selected set before the ceiling.

**Owner:** #285.

### F79 — HIGH — Token material is compared, not independently recomputed

Handle and continuation creation receive plaintext token/ID/digest material from an external crypto/provider seam. The package largely checks equality/collisions among caller-provided fields instead of recomputing the dedicated-domain token digest through the accepted canonical crypto owner.

**Owner:** #282/#284 after #237.

### F80 — CRITICAL — Product serving still uses legacy catalogs

The daemon `CommandState` still holds `ContinuationCatalog` and `ResultHandleCatalog`. The legacy handle map is keyed by plaintext token and does not store the original authorized evidence range; expansion accepts arbitrary ranges within the source revision and uses a clean Boolean barrier. Legacy continuations retain `StoredMatch` windows.

Canonical `HandleStore`/`ContinuationStore` are retained under the standalone process owner but are not the actual query-serving stores.

**Owner:** #275/#276 after package-core fixes.

---

## Useful legacy mechanics to preserve

The legacy daemon implementations contain mechanics worth porting, not authority models:

- staged handle batches under an exclusive catalog borrow;
- `PreparedExpansion` retaining the handle borrow until output;
- prepared continuation pages that defer mutation until complete output;
- fail-stop behavior after partial output;
- deadline checks before write/flush;
- batch token collision reservation.

Port these mechanics into the canonical package APIs. Do not preserve plaintext maps, broad source fences, arbitrary revision ranges, clean barriers or local session tags as product authority.

---

## Required serialization

```text
#237
  ├─ #282 → #283
  └─ #284 → #285   (also waits for #272/#274 and durable owner APIs)

#267 + #272 + #274 + #282 + #283
→ #275

#235 + #261/#263 + #272 + #274 + #275 + #284 + #285
→ #276

#274 + #275 + #276
→ #249 / #279 / #280 / #281 / #264
```

Agents must not patch package-core defects inside daemon integration worktrees.

---

## GitHub reconciliation performed

- created #282 and #283 for canonical handle package work;
- created #284 and #285 for canonical continuation package work;
- closed PR #118 as `[SUPERSEDED][PACKET-ONLY]`;
- updated #275/#276 with package-core prerequisites;
- corrected #249 and #264 blocker guidance;
- recorded F69–F80 and the selected graph on coordinator #97.

---

## Evidence boundary

This audit is static source analysis. It did not:

- modify Rust source;
- add dependencies or update `Cargo.lock`;
- execute Cargo, Clippy or tests;
- run native Windows/provider/Qdrant cases;
- prove exploitability in an installed product;
- qualify or enable handles, continuations or recipes.

Every implementation issue requires locked Rust 1.98 package checks, strict Clippy and focused nonzero fixtures after code.