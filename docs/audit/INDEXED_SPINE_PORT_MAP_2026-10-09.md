# Indexed spine port map — 2026-10-09

**Audited main:** `f1757d5b528d63abde28eff443f1070ff88fff4d`  
**Coordinator:** #97  
**Product enabled:** no

## Decision

The old stacked chain `#207 → #209 → #210 → #200` is not a merge plan. Those PRs are closed immutable source-donor snapshots.

| Donor | Exact head | Useful material | Rejected material | Executable owner |
|---|---|---|---|---|
| #207 | `f0ac8a1470336475f28e3fb30610947c83bbb6d1` | eight-field S11 key, full digest/address split, stateless collision fixtures | manual CBOR, direct BLAKE3 owner, old stack | #256 |
| current unitizer v2 | current main | bounded deterministic spans/digests | lacks UnitId/UnitKind/NativeAnchor and complete typed UnitSet authority | #257 |
| #209 | `e5c14cde20292fec59e5cabcc921eda3ae5aaa67` | S9.5 point/manifest/diff model | caller UnitSet, duplicate schema/digest writer | #258/#259 |
| #210 | `a697e175bc22e5a8ebf5491f867ec0518c0ab28e` | manifest-integrity delta, typed membership fence | stacked branch, incomplete submit/restore/effect proof | #260/#261 |
| #200 | `615d64f70d8b953cc585990659b865cfe250092b` | Qdrant 1.19 codecs, schema verification, live fixtures, total budget | duplicate ID/schema/filter, float epoch guard, local replay ledger | #258/#262/#263 |

## Executable graph

```text
#237
├─ #256 point identity
├─ #257 UnitSet v3
└─ #258 shared indexed contract

#256 + #257 + #258 → #259
#259 → #260 → #261
#256 + #258 → #262
#261 + #262 → #263
all owners → #264
```

## Load-bearing findings

### F56 — stacked donor branches are incompatible merge bases

They were created on obsolete bases and repeat identity/schema/codec ownership. A wholesale rebase would preserve parallel authority.

### F57 — unit-manifest v2 does not prove the planner denominator

The current durable descriptor has ordinal/span/line/boundary/digest, but not contract UnitId, UnitKind, NativeAnchor, structural identity or configuration predicate. `Representation.unit_manifest_digest` therefore cannot yet authorize the exact complete occurrence set required for projection replacement.

### F58 — #207 duplicates canonical ownership

Port its S11 model through #237. Do not copy manual canonical CBOR or package-local generic BLAKE3.

### F59 — #209 accepts a self-asserted complete unit list

`Vec<PreparedUnit>` can be a self-consistent subset. #259 must consume #257 `VerifiedUnitSet`, not a caller list/count/digest.

### F60 — #200 has several authority defects

- duplicate `QdrantPointId`/identity comparison;
- copied payload/index/filter contract;
- `i64 → f64 → i64` epoch validation that can accept `i64::MAX`;
- physical-name keyed route admission;
- process-local mutation ledger treated like replay authority;
- sparse-only/live capability not bound clearly to planner profile.

### F61 — #210 is a small delta, not a complete publication owner

Submit and restore need one canonical admission function. Stage/closure/control/snapshot advancement needs owner-issued exact effect/readback proofs and a durable recovery journal.

## Fresh generation rule

All identity/payload/manifest/schema changes require a new `CollectionGenerationId`. Old points are rebuilt from retained source/preparation. No old identity or payload is relabelled, mixed or adopted in place.

## Completion boundary

The indexed path remains disabled until #264 proves one fresh generation through:

```text
source truth
→ UnitSet
→ manifest
→ durable publication
→ Qdrant exact readback
→ scoped retrieval and IDF
→ source/access revalidation
→ range-bound output
→ restart/unknown-outcome recovery
```
