# Wave 2 package-instruction exception matrix — current revision

**Authority:** root `AGENTS.md`, exact source issue, [bounded execution protocol](../product/EXECUTION_PROTOCOL.md), and this matrix.  
**Current source base:** `de07c2214097a596066d6afac308394d9b4bcd62`.

One manager owns commits, root dependency pins and `Cargo.lock`. An exception permits one frozen source slice; it does not authorize the whole issue/programme in one PR.

| Issue / slice | Primary ownership | Exact additional scope | Still forbidden |
|---|---|---|---|
| #256 completed | `search-point-identity` | focused profile/docs/fixtures and exact ledger rows | deleting imported legacy exports before #329; daemon/planner rewrite |
| #257 completed | `search-unitizer` | one named materializer ingress, focused docs/fixtures/ledger | daemon/planner/store integration; provisional second UnitSet |
| #266.A root admission | `search-runtime-owner`, daemon `owner_composition` | existing-only root open helpers, explicit initialization, narrow immediate command/caller assembly | source/query/Qdrant/provider redesign; control schema; second root catalogue |
| #266.B focused lifecycle proof | same owners after A | only production seams and fixtures required to prove A's lifecycle invariants | general harness repair; unrelated migration/recovery owners |
| #266.C catalog evidence | owner-private catalog intent/recovery observation | retained input/release, bounded named inspection/discovery, exact focused callers | effect reconciliation, deletion qualification, redb repair, ordinary recovery authority |
| #235.core | new `search-provider-client` | typed CLI client move/deletion, workspace member/lockfile, narrow CLI wiring | daemon internals; TCP/token/line protocol promotion; native cutover |
| #238 | `search-config` | exact TOML/Serde pins, docs, shared exact-byte SHA exception | runtime apply/readiness/argv; changing old digest semantics silently |
| #241 | `search-source-admission` | exact GlobSet pin/lockfile/docs; shared digest compatibility API | ingestion/safe-reader/root/registry mutation; ignore walker |
| #246 | `search-exact` | exact matcher pins/lockfile/docs | denominator/access/handles; regex successor work |
| #252 | `search-lexical` | exact Unicode pins/lockfile/profile docs | prose/full fold; Qdrant migration; old-vector reinterpretation |
| #226.markdown | materializer + unitizer | Markdown pin/lockfile/profile fixtures after #257/#331 interface | JATS/PDF/rendering; new UnitSet/document catalogue |
| #226.jats | same owners after Markdown | XML pin/lockfile/JATS fixtures | Markdown rewrite; dynamic XML tree; external entities |
| #329 after #259/#262 | point-identity legacy retirement | migrated planner/bridge imports; exact daemon projection caller/fixtures; ledger/docs | new schema/transport/general daemon rewrite; qualification claim |

Rows name ownership envelopes. The exact issue and `SCOPE_FROZEN` block narrow them further. All unlisted nearest-package prohibitions remain effective.

## Current merge order

```text
#266 bounded tranches
→ minimum typed root API on main
→ #235.core
→ #238 → #241 → #246 → #252
→ #226 Markdown → #226 JATS
```

`#235.core` requires the minimum root API it actually consumes. It does not wait for every #266 follow-up, harness, native cleanup, redb inspection or final qualification item.

## #266 split boundary

PR #344 exceeded the default review budget. Process issues #349/#350 require scope freeze or split. The following separate issues remain outside #266 source tranches unless an exact `B0` dependency is proved:

```text
#343 redb existing-only non-mutating inspection
#345 record-artifact unknown-publication staging retention
#346 remaining all-target fixture compilation
#347 native original-object intent unlink / late cleanup
#348 original request to durable-effect reconciliation
```

The #266 tranches may keep these operations unavailable/fail-closed. They may not fabricate success, automatically clear evidence or widen ordinary authority.

## Review budget and amendment rule

Default split triggers from the execution protocol:

```text
one primary owner
at most two narrow adapter families
at most 30 changed production files
at most 2,500 changed production lines
at most one persisted migration or one cross-owner cutover
```

When a threshold is crossed, stop feature work and split. A new “narrow continuation” comment does not amend a frozen slice. A reviewed exception must precede additional source changes and must name why the result cannot be split safely.

## Build-safe identity migration

Delivered #256 added S11 without breaking current consumers. Existing legacy APIs remain unchanged and explicitly legacy only until #259/#262 cut consumers over. #329 removes old callers/exports and exact ledger exceptions before #264. No normal new-generation path may fall back to legacy identity.

## Exact-byte SHA compatibility

`search_contracts::sha256_raw` computes a domain-separated digest. Old config fingerprints already contain their legacy framing and require exact-byte SHA parity.

#238 may add one bounded exact-byte SHA-256 primitive inside the existing shared owner:

```text
crates/search-contracts/src/digest.rs
crates/search-contracts/src/lib.rs export only
focused shared vectors/docs
exact ledger refresh
```

Use the existing private RustCrypto implementation, no prefix, finite input ceiling and official vectors. Config v2 uses a new canonical profile; v1 compatibility cannot be relabelled as v2.

## Dependency and ledger discipline

The manager verifies exact source/checksum/license/MSRV/features/advisories only when adopting or changing a dependency. Reuse accepted pins. Resolve root manifest/lock changes once per slice.

Refresh only exact reviewed ledger rows. Issue closure does not prove every prior occurrence migrated. Retained legacy or pending sites keep their real executable owner.

## Finding routing

After `SCOPE_FROZEN`, classify every discovery:

```text
B0 same-slice compile/safety blocker
F1 same-owner follow-up
F2 adjacent-owner follow-up
D  unchanged baseline debt
Q  qualification evidence
```

Only B0 may widen the source tranche, and only with a concrete path/symbol/caller causal statement. No exception creates a second source, index, schema, digest, client, journal, store, owner or qualification authority.
