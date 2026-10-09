# Wave 2 package-instruction exception matrix — 2026-10-09

**Purpose:** exact integration exceptions, incorporated by root `AGENTS.md`.  
**Verified implementation snapshot:** `17383079d316cc090eae04912534e2d16e6dc79d`.  
**Completed:** #237, #253, #258 and #250. Use the latest accepted base on #97, not a historical launch SHA.

One manager owns the integration worktree, commits, dependency pins and lockfile. This matrix does not authorize a second writer/integration manager or general cross-package edits.

| Issue | Primary package ownership | Additional exact writes allowed | Still forbidden |
|---:|---|---|---|
| `#258` completed | `crates/search-contracts/**` | accepted indexed contract/docs/fixtures | reopening the completed port as a parallel schema owner |
| `#250` completed | `xtask/**` | accepted metadata/status tooling and dependency pins | silent checker weakening; product runtime authority |
| `#256` | `search-point-identity/**` | focused port docs/fixtures and exact canonical-ledger refresh | deleting still-imported legacy exports before consumer cutover; daemon/planner rewrites; new codec or generic crypto owner |
| `#257` | `search-unitizer/**` | focused contract adapters/fixtures and exact ledger refresh | materializer/parser implementation; a provisional second UnitSet |
| `#266` | `search-runtime-owner/**` | daemon `owner_composition/**`, narrow entry/command assembly, legacy `DirectStore` open helpers needed for cutover | source/Qdrant/query redesign; second root catalog |
| `#235` | new `crates/search-provider-client/**` | move/delete typed CLI client code, workspace membership, lockfile, narrow CLI wiring | daemon internals, legacy loopback promotion, protocol redesign, TCP fallback |
| `#238` | `search-config/**` | exact TOML/Serde pins; focused package docs; the narrow shared exact-byte SHA extension described below | daemon readiness/apply; input acquisition; changing existing digest-domain semantics |
| `#241` | `search-source-admission/**` | exact GlobSet pin/lockfile and package docs; reuse accepted shared SHA API | daemon ingestion, safe-reader/root/registry mutation, ignore walker |
| `#246` | `search-exact/**` | exact matcher pins/lockfile and package docs | denominator/access/handles or regex successor work |
| `#252` | `search-lexical/**` | exact Unicode pins/lockfile and profile docs | full folding/prose implementation, Qdrant migration, old-vector reinterpretation |
| `#226-C1` | materializer + unitizer | exact Markdown pin/lockfile and fixtures after #257 | JATS/PDF/rendering, new UnitSet, worker framework |
| `#226-C2` | same owners after C1 | exact XML pin/lockfile and JATS fixtures | Markdown rewrite, XML dynamic tree, external entities |

## Current order

Completed #237/#253 and #258/#250 must not be repeated.

```text
#256 → #257 → #266 → #235
→ #238 → #241 → #246 → #252
→ #226 Markdown → #226 JATS
```

#324 is a separate documentation repair. #327 is a small security lockfile repair coordinated by the same manager before any affected TLS/network profile is accepted. Neither authorizes another independent integration branch manager.

## Build-safe identity migration: exact #256 exception

The current daemon imports `PointIdentityRegistry` from `search-point-identity`, and its projection ownership test names that symbol. Removing the export in a package-only #256 PR would break consumers excluded from that PR's old check command.

#256 therefore **adds the new stateless S11 profile in a distinct cohesive module first**. Existing legacy API/bytes remain unchanged only for existing consumers; they are not S11, cannot enter the new collection generation, and must not be extended or wrapped in a second generic facade. Do not add a fallback from S11 to legacy.

#259/#262 and the daemon integration owner remove old production callers and then legacy exports in the same accepted consumer-cutover series. #264 cannot qualify a new indexed generation while a normal path can silently select legacy identity. Record exact retained symbols and deletion owners in #256's handoff. This staged removal is explicit migration debt, not completion of the full product cutover.

Do not break downstream compilation merely to claim immediate code deletion. Check direct reverse consumers; retain pre-existing diagnostics separately from any newly introduced failure. Do not suppress new errors or revive old donor branches.

## Exact-byte SHA compatibility: narrow #238 exception

The merged `search_contracts::sha256_raw` hashes `domain || NUL || bytes`; it is **not** standard SHA-256 of `bytes` alone. Old config fingerprints already contain a legacy domain/framing and compute standard SHA-256 of that complete preimage. The new domain helper is not a byte-compatible replacement.

Before removing the old implementation, #238 may add one explicitly named bounded **exact-byte SHA-256** primitive in the existing shared owner:

```text
crates/search-contracts/src/digest.rs
crates/search-contracts/src/lib.rs only for its export
focused shared digest fixtures/documentation
exact canonical-digest-ledger refresh
```

Requirements: same private RustCrypto donor; caller-byte limit checked before hashing; no extra prefix; official standard vectors; old config preimage parity; clear documentation that a content checksum grants no semantic authority. Existing raw-domain/canonical helpers and golden bytes remain unchanged. No new crate, generic receipt, unbounded reader or new hashing framework.

New v2 config identities still use a closed canonical domain. Exact-byte compatibility does not permit relabelling v1 as v2. If a required old input exceeds the accepted primitive's ceiling, preserve an explicit unsupported/legacy state and obtain a separately reviewed streaming/size profile; never silently clamp or buffer unbounded input.

## Dependency and ledger discipline

Only the manager changes root pins/Cargo.lock. Verify exact source/checksum/license/MSRV/features/advisories, resolve once, run locked package check and strict Clippy, and merge before the next slice. Reuse accepted dependencies rather than reselecting them from a stale candidate table.

Refresh only reviewed ledger sites. A closed implementation issue is not proof that every legacy occurrence it once owned was migrated. Preserve classifications and phases until the actual owner change occurs.

## Stop rule

Changes outside an exact row require an explicit scope amendment before implementation. No exception creates another source, index, schema, digest, client, journal or qualification authority.
