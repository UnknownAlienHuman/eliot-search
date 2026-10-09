# Agent launch gate — 2026-10-09

**Audited implementation base:** `e10f4946db8802a2e5306774dfd045330e607718`  
**Coordinator:** #97  
**Product/release ready:** no  
**Broad swarm ready:** no  
**Controlled implementation micro-swarm ready:** yes

## Operational freeze

The previous launch base was followed by eleven commits. Direct comparison proved that they changed only:

```text
docs/audit/**
docs/product/PACKAGE_STATUS.toml
```

No Rust source, Cargo manifest/lockfile or workflow changed. Therefore `e10f4946db8802a2e5306774dfd045330e607718` is the single accepted implementation base for Wave 1.

No branch or pull request for #237 or #253 existed when this gate was refreshed. Managers must create fresh branches/worktrees from the exact base above. A later documentation-only main commit does not silently change the implementation base. Only a coordinator update may publish a replacement base.

Pause further broad audit/backlog generation while Wave 1 is active unless a newly discovered defect directly changes #237/#253 safety or ownership. New findings go to the relevant current issue; they do not create another competing program.

## Wave 1 — launch now

Launch exactly two non-overlapping managers:

```text
coding manager A: #237
research/docs manager B: #253
```

### Coding manager A — #237

Required branch/worktree:

```text
branch: agent/237-canonical-foundation
base:   e10f4946db8802a2e5306774dfd045330e607718
one manager / one worktree
```

Exclusive ownership:

```text
crates/search-contracts/**
canonical-digest guard modules in xtask
root crypto dependency pins
Cargo.lock integration
```

Stop after common canonical/digest APIs and the typed current-tree guard/allowlist. Do not migrate owner packages in this branch.

### Research manager B — #253

Required branch/worktree:

```text
branch: research/253-unicode-casefold
base:   e10f4946db8802a2e5306774dfd045330e607718
one manager / one worktree
```

Owns only the Unicode full-case-fold donor/data/profile decision and golden fixtures. It edits no Rust manifest, Rust source, root dependency pin or `Cargo.lock`.

## Wave 1 merge rule

1. #237 opens one implementation PR referencing the issue.
2. #253 opens one documentation/research PR or supplies the exact accepted artifact and PR requested by the issue.
3. #237 is reviewed and merged first for shared-code progression.
4. Publish the exact post-#237 `main` SHA on #97 and every Wave 2 issue before Wave 2 coding starts.
5. Rebase each Wave 2 branch on that exact SHA before its final locked check and strict Clippy.

No manager may merge directly to `main`, reuse an old packet/donor branch or share a worktree with another manager.

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

Do not launch all Wave 2A managers blindly. The coordinator publishes a batch of non-overlapping owners after #237 with one named lockfile integrator and explicit merge order.

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

#272/#274 and accepted owners
→ #282 → #283 → #275
→ #284 → #300 → #285 → #276
→ #287 → #288 → #290 → #278 → #291 → #293 → #294 → #279

#260/#261 + #284/#285/#274
→ #300 → #303

#237 + durable owner/time/secret/object authorities
→ #301 → #302

#304 → #305
#307 → #308 → #309
```

Blocked lifecycle, secret, access, handle, continuation, ranking and purge issues are implementation specifications, not additional Wave 1 work.

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
#126/#134/#135/#136 → #300–#305 and #243–#245
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
- a closed donor/packet branch used as the working base;
- starting a blocked child issue because its body is detailed;
- changing the accepted base without a coordinator record.

## Swarm readiness verdict

```text
Product/release execution:        NOT READY
Broad autonomous coding swarm:    NOT READY
Controlled Wave-1 micro-swarm:    READY
Wave 2A:                          BLOCKED BY #237 MERGE
Later graph:                      SPECIFIED, DEPENDENCY-BLOCKED
```

The backlog is substantially specified and donor-optimized, but it is not “fully optimized” in the sense of being safe for arbitrary parallel execution. Shared foundations, control/root/source authority and destructive lifecycle owners still require strict serialization.

## Evidence boundary

This launch gate is operational planning. It does not assert exact-head Cargo, Clippy, native Windows, Qdrant, installed-product or release qualification. The current accepted implementation base has no associated GitHub status checks or workflow runs; #237 must produce its own exact locked evidence.