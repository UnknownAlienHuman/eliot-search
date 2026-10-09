# Wave 2 package-instruction exception matrix — 2026-10-09

**Purpose:** resolve only the exact conflicts between nearest package `AGENTS.md` files and the one-manager Wave-2 integration tasks.  
**Authority:** incorporated by root `AGENTS.md` only after this packet is merged.  
**Coding base:** exact post-`#237` `WAVE2_BASE_SHA` published by `#97`.

This is not general cross-package permission. The manager may exercise one row only while working on that exact issue, from latest accepted `main`, in one clean worktree.

| Issue | Primary package ownership | Additional exact writes allowed | Still forbidden |
|---:|---|---|---|
| `#258` | `crates/search-contracts/**` | focused contract docs/fixtures; no root dependency change expected | Qdrant SDK, bridge/planner code, donor codec/crypto |
| `#250` | `xtask/**` | exact `cargo_metadata` root pin and `Cargo.lock`; package-status docs/fixtures | product daemon/runtime authority, unbounded `MetadataCommand::exec()` |
| `#256` | `search-point-identity/**` | focused donor-port docs/fixtures | `search-contracts` changes after `#258`, Qdrant mutation, direct generic crypto owner |
| `#257` | `search-unitizer/**` | focused contract consumers/fixtures; removal of package-local digest use through accepted `#237` APIs | materializer/parser implementation, projection planner, provisional document schema |
| `#266` | `search-runtime-owner/**` | daemon `owner_composition/**`, narrow entry/command assembly, legacy `DirectStore` open helpers needed for cutover | source/Qdrant/query redesign, second root catalog, broad daemon behavior |
| `#235` | new `crates/search-provider-client/**` | move/delete typed code from `bins/eliot-search/src/provider_client/**`; root workspace membership and `Cargo.lock`; narrow CLI re-export/wiring | daemon internals, legacy loopback promotion, protocol redesign, TCP fallback |
| `#238` | `search-config/**` | exact TOML/Serde pins and `Cargo.lock`; focused package docs | daemon readiness/apply, argv/filesystem/environment acquisition, dynamic config framework |
| `#241` | `search-source-admission/**` | exact `globset` pin and `Cargo.lock`; focused package docs | daemon ingestion, safe-reader/root/source-registry mutation, ignore walker |
| `#246` | `search-exact/**` | exact matcher pins and `Cargo.lock`; focused package docs | denominator/access/handle integration, regex/multi-pattern successor work |
| `#252` | `search-lexical/**` | exact Unicode pins and `Cargo.lock`; profile docs/fixtures | full case folding/prose profile, Qdrant migration, old-vector reinterpretation |
| `#226-C1` | `search-materializer/**` + `search-unitizer/**` | exact `pulldown-cmark` pin and `Cargo.lock`; Markdown profile docs/fixtures | JATS/PDF/HTML renderer, new UnitSet, worker/process framework |
| `#226-C2` | same owners after C1 | exact `quick-xml` pin and `Cargo.lock`; JATS profile docs/fixtures | Markdown rewrite, Serde XML tree, encoding/transcoding features, external entities |

## Serialization rules

```text
#237 merged
→ #258 exclusive search-contracts edit
→ #250
→ #256
→ #257
→ #266
→ #235
→ #238
→ #241
→ #246
→ #252
→ #226-C1
→ #226-C2
```

The manager may reorder only after posting a dependency/overlap proof to `#97`. `#258` remains first and `#226` remains after `#257`.

## Root pin and lock rule

For an allowed dependency slice:

1. subagent returns the exact donor acceptance record;
2. manager independently checks source/API/checksum/license/MSRV/features/advisories;
3. root pin is exact and private to the owning package;
4. manager updates `Cargo.lock` once;
5. `cargo metadata --locked --offline` succeeds;
6. package check/strict Clippy succeeds;
7. PR merges before the next dependency slice branches.

No subagent edits root files. No second branch resolves lock conflicts.

## Compatibility rule

The exception does not allow temporary duplicate public types or product-reachable compatibility implementations. When an old path must remain for durable decode/rebuild, it is explicitly labelled legacy read-only and cannot mint current identities/receipts.

## Stop rule

If the required change falls outside a row, the manager stops and amends the current owner issue/packet. The manager does not widen the row opportunistically.