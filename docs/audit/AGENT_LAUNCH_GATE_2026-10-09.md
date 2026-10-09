# Agent launch gate — 2026-10-09

**Base accepted for launch:** `2ae7ecbdf7dced3cc951e5e162dbba10def03dbc`  
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

## Wave 2A — shared foundations after #237

Launch from the exact accepted post-#237 main:

```text
indexed foundations:
  #256 search-point-identity — exact S11 port
  #257 search-unitizer — authoritative typed UnitSet / manifest v3
  #258 search-contracts — shared S9.5/S10.3/epoch contract

root/control foundation:
  #266 one typed data-root admission/open owner

code-reduction foundations:
  #235 canonical provider client
  #238 standard TOML/config cutover
  #241 bounded GlobSet admission policy
  #246 single-literal matcher cutover
  #250 cargo_metadata tooling cutover
  #252 code_identifiers@1
  #226 Markdown/JATS profiles
```

These own disjoint package surfaces, but several change root dependency pins or `Cargo.lock`. **One dependency/lock integration manager at a time** merges root pin/lock changes. Every branch rebases on latest accepted main before final gates.

## Root/source/control serialization

```text
#237 → #266
#266 + #241 → #267

#237 + #266
→ #268 complete legacy inventory/staged redb mapping
→ #269 atomic cutover to one redb authority

#237 + #241 + #267 + #269
→ #270 pre-admission, one source identity index and bounded staging

#239 + #267 + #269 + #270
→ #271 coherent immutable SourceView

#269 + #271
→ #272 immutable corpus/portfolio revisions and exact scope compiler
```

Do not start a blocked child on temporary path/root/security/control helpers.

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
  #272 + all indexed/root/source/access/handle/supervisor owners
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

Read-only source/fixture archives, never merge bases:

```text
#207 f0ac8a1 → #256
#209 e5c14cd → #257/#258/#259
#210 a697e17 → #260/#261
#200 615d64f → #258/#262/#263/#264
```

## Superseded packet/tracking branches

```text
#100 → #267/#215
#104 → #267
#105 → #266
#106 → #268/#269/#261
#107 → #268
#108 → #269
#110 → #241/#267/#269/#270/#271
#128 → #239/#267/#269/#270/#271/#272
#122 → #252–#254
#131 → #246–#249
#138 → #250–#251
```

Do not rebase/cherry-pick these branches or treat their open/closed PR state as implementation evidence.

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
- path/parent/PID/quiet-watcher state used as root/source/currentness authority;
- hidden normalization, transcoding, skip or fallback semantics;
- allocation or collection before the accepted ceiling;
- blind replay after a possible external effect;
- a compatibility implementation left product-reachable after cutover;
- an uncoordinated root dependency/Cargo.lock edit;
- a closed donor/packet branch used as the working base.

## Evidence boundary

This launch gate changes planning/status only. It does not assert current-head Cargo, Clippy, native Windows, Qdrant, installed-product or release qualification.
