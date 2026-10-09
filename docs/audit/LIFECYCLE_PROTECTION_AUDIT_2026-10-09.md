# Lifecycle protection, reclaim and purge audit — 2026-10-09

**Repository:** `UnknownAlienHuman/eliot-search`  
**Audited source:** `fb97705c74a5ab3dd6efb1ff0f2da5e1c0c4afdf`  
**Scope:** epoch pins, retention leases, CAS mark/sweep, Qdrant retired-point reclaim and security purge  
**Product qualification performed:** none

## Result

The lifecycle packages contain useful bounded pure kernels and correct high-level intent: exact-ID deletion, pin-aware retirement, Nix-style reachability and deny-before-delete purge ordering. They are not yet safe deletion authorities. Current APIs accept caller time, Booleans, roots, inventories, graph edges, watermarks, receipts and status records that can manufacture reclaimability or phase completion.

The accepted graph is now:

```text
#260/#261 publication
→ #300 route/epoch pin authority
→ #303 ordinary retired-point reclaim

#301 durable owner-timed retention leases
→ #302 owner-derived CAS mark-sweep

#304 authoritative purge core
→ #305 real all-plane purge integration
→ #264/#215 qualification

#301/#302/#304/#305
→ #243 backup → #244 restore → #245 migration
```

PRs #126, #134, #135 and #136 are closed as packet-only.

---

## Findings

### F135 — CRITICAL — Route pins can protect arbitrary routes

`acquire_route_pin` accepts a caller route and inserts it without proving that it is the active route or one exact retired-route protection target.

**Owner:** #300.

### F136 — CRITICAL — Foreign pins can be released by guessing an owner ID

`release_owner_pins(&OpaqueId)` has no owner capability, session binding or cleanup-operation proof. Any caller that presents the same opaque owner ID can release another query/continuation owner’s protections.

**Owner:** #300.

### F137 — CRITICAL — Caller time controls pin expiry and renewal

Pin acquisition, continuation renewal and expiration accept caller `now_ms`. A large value can expire protections early; renewal can extend protection from untrusted time and is not bound to the exact continuation lifetime.

**Owner:** #300.

### F138 — CRITICAL — Pin snapshots and reclaim watermarks are forgeable

`PinRegistrySnapshot`, `RetiredVisibilityFence` and `ReclamationWatermark` are public structs. Reclaim code accepts a watermark by value, allowing a caller to construct `reclaimable=true` with zero blockers.

**Owner:** #300/#303.

### F139 — HIGH — Route publication and cleanup faults are caller-shaped

Active route publication accepts raw route/epoch values rather than #261 publication readback. Weak-pin release can convert registry disappearance/poisoning to `false`, after which `Drop` considers the guard released and loses a typed cleanup fault.

**Owner:** #300.

### F140 — CRITICAL — Failed lease mutations consume replay capacity

Lease create/renew/end register the operation before validating target/state/capacity/transition. A failed request can permanently consume one `max_operations` slot. Repeated invalid requests can block all later legitimate lease work.

**Owner:** #301.

### F141 — HIGH — Lease operation and terminal history is permanently exhaustible

The lease operation map and expired/released/revoked records have no replay-safe compaction policy. Never compacting eventually exhausts capacity; deleting blindly breaks idempotency/audit evidence.

**Owner:** #301.

### F142 — CRITICAL — Lease lifetime and expiry are caller-controlled and non-durable

Create/renew/active/expire paths use caller timestamps. Expiry mutates process memory, returns only a count, uses saturating revision increments and records no durable operation/readback state.

**Owner:** #301.

### F143 — CRITICAL — CAS protection authority is caller data

Sweep protection accepts caller roots, pins, tombstones, leases and `PinEvidence { fresh: bool }`. A digest over supplied sets does not prove completeness or owner provenance.

**Owner:** #302.

### F144 — CRITICAL — Omitting one graph edge can authorize deletion of a reachable object

Mark traversal accepts a caller object graph and inventory. An omitted outgoing edge is interpreted as no edge; a reachable object can therefore fall into the dead-set candidate list.

**Owner:** #302.

### F145 — CRITICAL — Sweep plans, receipts and final inventory are caller-constructible

Protection sets, intents, manifests, plans, batches, receipts and final inventory are public data. There is no authoritative durable sweep journal proving which effects occurred.

**Owner:** #302.

### F146 — CRITICAL — Sweep identities are fake/truncated and mutation input is ignored

Protection/mark digests use a package-local FNV-like mixer wrapped as BLAKE3. Batch IDs use a short digest prefix. `CasMutation.input_digest` is accepted but not verified.

**Owner:** #302 through #237.

### F147 — CRITICAL — Qdrant reclaim accepts agreeing caller publication proof and watermark

A public retired manifest plus public publication proof can create a committed manifest by field equality. Planning/resume then accepts a public reclaim watermark. No owner publication readback or manifest digest recomputation is required.

**Owner:** #303.

### F148 — HIGH — Reclaim readback is not route/generation bound

The index-admin readback accepts point IDs without an inseparable route/collection-generation/schema/epoch identity. Public checkpoints and completed receipts drive resume rather than a durable operation journal.

**Owner:** #303.

### F149 — CRITICAL — Purge authority and manifest identity are caller-issued

`PurgeAuthority.authorized` is a public Boolean. Purge manifest validation does not recompute the manifest digest or derive the complete target population from exact owners.

**Owner:** #304.

### F150 — CRITICAL — Live-deny proof ignores load-bearing fields

`PurgeBarrierProof` is public. The current gate compares selected IDs/revisions but ignores `live_generation` and `live_snapshot_digest` and does not read the actual security owner.

**Owner:** #304 through #274.

### F151 — CRITICAL — Invalidation, plane status and resume are caller Booleans

Handle/continuation/cache/overlay invalidation is represented as all-true Booleans; the invalidation digest is ignored. Plane completion and outcome-unknown status are public Booleans. Resume accepts `fence_still_live` and `generation_matches` from the caller.

**Owner:** #304.

### F152 — CRITICAL — Ordinary reclaim and purge are distinguished by caller assertion

One helper receives a Boolean saying whether a receipt is ordinary reclaim. CAS purge target/protected sets are caller data. Receipt types and owner evidence do not structurally prevent cross-use.

**Owner:** #304/#305.

### F153 — CRITICAL — Generic layer receipts can fabricate complete purge

One public `PurgeLayerReceipt` shape can advance multiple planes when IDs match. Readback digest/receipt provenance is not verified. The coordinator retains only the most recent receipt rather than a complete recoverable transaction history.

**Owner:** #304.

### F154 — CRITICAL — Tombstone, logical receipt, physical erasure and backup disposition are caller supplied

Terminal completion accepts caller logical non-accessibility receipt, tombstone digest, physical-erasure evidence and backup-disposition enum/receipt. They are not derived from the complete per-plane journal and exact governed backup inventory.

**Owner:** #304/#305.

---

## Required design boundaries

### Pin authority

```text
#261 publication route observation
+ #284 continuation record/lifetime
+ accepted time owner
→ #300 private guards and opaque current reclaim permits
```

No caller time, raw route publication, release-by-known-owner-ID or public watermark.

### Durable leases

```text
typed owner/object/lifetime mutation
→ precondition validation
→ atomic durable lease + operation result
→ bounded owner-timed expiry and replay-safe compaction
```

Failed requests do not consume operation capacity.

### CAS sweep

```text
complete owner roots + #300 pins + #301 leases
+ complete CAS inventory and every edge under one generation
→ private protection snapshot/mark/plan
→ durable exact batch effects/readback
```

Missing edge/inventory/root blocks deletion.

### Ordinary Qdrant reclaim

```text
#260/#261 committed retired manifest
+ #300 current pin permit
+ #262 exact route/generation
→ private exact-ID plan/journal
→ #263 delete/readback on the same route/generation
```

It is not purge evidence.

### Purge

```text
privileged authenticated request
+ exact #272 affected scope/target manifest
→ durable intent and #274 live deny first
→ exact typed per-plane effects/readback
→ #243 backup/legal disposition
→ canonical permanent logical tombstone
```

Every unknown effect remains pending and denied through restart. Physical erasure is never inferred from logical deletion.

---

## Required serialization

```text
#260/#261 + #284 + #274
→ #300
→ #285/#276 and #303

#237 + #267 + control/time owners
→ #301
→ #302

#260/#261/#262/#263/#300
→ #303

#237/#266/#269/#274/#282–#285/#293/#294/#300–#303/#243
→ #304
→ #305

#301/#302/#304/#305
→ #243 → #244 → #245
```

Agents must not implement destructive lifecycle effects from public snapshot/receipt structs or old packet branches.

---

## GitHub reconciliation performed

- created #300 for pin authority;
- created #301 for durable leases;
- created #302 for CAS mark-sweep;
- created #303 for ordinary Qdrant reclaim;
- created #304 for purge core;
- created #305 for real all-plane purge integration;
- corrected the #300/#285 dependency cycle;
- updated #243/#244/#245 and #264;
- closed packet PRs #126/#134/#135/#136;
- recorded F135–F154 on coordinator #97.

---

## Evidence boundary

This is static source analysis. It did not modify Rust source/dependencies, run Cargo/Clippy/tests, perform Qdrant/CAS deletion, exercise native Windows/provider paths or qualify lifecycle safety. Every implementation issue requires locked Rust 1.98 checks, strict Clippy and focused nonzero fixtures after code.