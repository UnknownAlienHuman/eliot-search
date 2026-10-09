# Agent launch gate — 2026-10-09

**Base accepted for launch:** `f1757d5b528d63abde28eff443f1070ff88fff4d`  
**Coordinator:** #97  
**Product/release ready:** no  
**Controlled implementation wave ready:** yes

## Wave 1 — launch now

Do not launch a broad swarm. Launch exactly two non-overlapping managers:

```text
coding manager A: #237
research/docs manager B: #253
```

### Coding manager A — #237

Exclusive ownership:

```text
crates/search-contracts/**
canonical-digest guard modules in xtask
root crypto dependency pins
Cargo.lock integration
```

Stop after common canonical/digest APIs and the typed current-tree guard/allowlist. Do not migrate owner packages in this branch.

### Research manager B — #253

Owns only the Unicode full-case-fold donor/data/profile decision and golden fixtures. It edits no Rust manifest, source, root dependency pin or lockfile.

## Wave 2A — indexed foundations after #237

Launch these three managers from the exact accepted post-#237 main:

```text
#256 search-point-identity: exact S11 port from donor #207
#257 search-unitizer: authoritative typed UnitSet / manifest v3
#258 search-contracts: shared S9.5/S10.3/epoch contract
```

They own disjoint package surfaces. #256 and #257 consume #237 APIs; they do not modify `search-contracts`. #258 owns the additional indexed-search contract module.

## Wave 2B — donor code reduction after #237

Queue separately:

```text
#235 canonical provider client
#238 standard TOML/config cutover
#241 bounded GlobSet admission policy
#246 single-literal matcher cutover
#250 cargo_metadata tooling cutover
#252 code_identifiers@1
#226 Markdown/JATS profiles
```

Many of these update root dependency pins/Cargo.lock. Package code may be developed in separate worktrees, but **one dependency/lock integration manager at a time** merges root pin/lock changes. Every branch rebases on latest accepted main before final gates.

## Indexed-spine serialization

```text
#256 + #257 + #258
→ #259 projection planner

publication lane:
  #259 → #260 → #261

bridge lane:
  #256 + #258 → #262

join:
  #261 + #262 → #263

product qualification:
  all above + root/source/access/handle/supervisor owners
  → #264
```

## Other serialized successors

```text
#246 → #247 → #248 → #249
#250 → #251
#252 + #253 → #254
#235 → #116 → #130/#219
#223 → #224 → #225 → #236
#228 → #229 → #230 → #231
```

## Closed source-donor branches

These branches are read-only source/fixture archives and are not merge bases:

```text
#207 head f0ac8a1 → #256
#209 head e5c14cd → #257/#258/#259
#210 head a697e17 → #260/#261
#200 head 615d64f → #258/#262/#263/#264
```

Do not rebase/cherry-pick them wholesale. Their manual codec/crypto/schema and old stacked bases conflict with current ownership.

## Superseded packet branches

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
- a compatibility implementation left product-reachable after cutover;
- an uncoordinated root dependency/Cargo.lock edit;
- a closed donor branch used as the working base.

## Evidence boundary

This launch gate changes planning/status only. It does not assert current-head Cargo, Clippy, native Windows, Qdrant, installed-product or release qualification.
