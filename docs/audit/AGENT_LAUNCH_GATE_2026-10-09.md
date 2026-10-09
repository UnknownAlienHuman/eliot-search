# Agent launch gate — 2026-10-09

**Base accepted for launch:** `19481855e44c1e98788e16918471127b5457d237`  
**Coordinator:** #97  
**Product/release ready:** no  
**Controlled implementation wave ready:** yes

## Decision

Do not launch a broad swarm. Launch exactly two non-overlapping managers first:

```text
coding manager A: #237
research/docs manager B: #253
```

### Coding manager A — #237

Exclusive ownership for the first wave:

```text
crates/search-contracts/**
canonical-digest guard modules in xtask
root crypto dependency pins
Cargo.lock integration
```

Allowed result:

```text
freeze existing canonical vectors
→ real bounded BLAKE3/SHA-256 helpers
→ honest decode-vs-compute API
→ full-width operation digest
→ typed current-tree guard and allowlist
→ stop before owner-package migrations
```

The latest #237 comment corrects stale paths in the older issue body. Branch from the exact base above; never use an old planning/selected branch.

### Research manager B — #253

Owns only the Unicode full-case-fold donor/data/profile decision and golden fixtures. It edits no Rust manifest, source, root dependency pin or lockfile and may run concurrently with #237.

## Wave 2 — blocked until #237 merges

```text
#235 canonical provider client
#238 standard TOML/config cutover
#241 bounded GlobSet admission policy
#246 single-literal matcher cutover
#250 cargo_metadata tooling cutover
#252 code_identifiers@1
#226 Markdown/JATS profiles
```

These tasks may need the shared canonical APIs, root dependency pins or `Cargo.lock`; do not start them on temporary local helpers.

## Serialized successors

```text
#246 → #247 → #248 → #249 (blocked integration)
#250 → #251
#252 + #253 → #254
#235 → #116 → #130/#219
#223 → #224 → #225 → #236
#228 → #229 → #230 → #231
#207 → #209 → #210 → #200
```

## Superseded packet branches

The following one-file September PRs are closed and are not implementation bases:

```text
#122 → #252–#254
#131 → #246–#249
#138 → #250–#251
```

## Agent completion report

Every manager reports:

```text
base/result SHA and branch/worktree
exact owned files/packages
root pins/lock changes
donor versions/features/checksums/licenses/advisories
custom code deleted or avoided
authority/invariant boundary
Rust 1.98 locked check outcome
strict Clippy outcome
focused fixture names and nonzero case count
next blocker/owner
no qualification claim beyond retained evidence
```

## Stop conditions

Stop and report instead of improvising if the change would introduce:

- a second authority, catalog, client, index, journal or canonical codec;
- donor public types across ELIOT domain/protocol boundaries;
- runtime download/network fallback;
- caller-issued digest/receipt/complete/fresh authority;
- hidden normalization, transcoding, skip or fallback semantics;
- allocation or collection before the accepted ceiling;
- blind replay after a possible external effect;
- a compatibility implementation left product-reachable after cutover.

## Evidence boundary

This launch gate changes planning/status only. It does not assert current-head Cargo, Clippy, native Windows, Qdrant, installed-product or release qualification.
