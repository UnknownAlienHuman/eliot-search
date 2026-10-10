# Wave 2 package-instruction exception matrix — 2026-10-09

**Authority:** exact integration exceptions incorporated by root `AGENTS.md`.  
**Verified implementation snapshot:** `17383079d316cc090eae04912534e2d16e6dc79d`.  
**Completed:** #237, #253, #258 and #250. Use the latest accepted base on #97, not an old launch SHA.

One manager owns integration, commits, dependency pins and Cargo.lock. This matrix does not authorize another integration manager or general cross-package changes.

| Issue | Primary ownership | Exact additional scope | Still forbidden |
|---|---|---|---|
| #258 completed | search-contracts | accepted indexed contract/docs/fixtures | parallel schema owner |
| #250 completed | xtask | accepted metadata/status tooling and pins | checker weakening; product runtime authority |
| #256 | search-point-identity | focused port docs/fixtures; exact canonical-ledger refresh | deleting still-imported exports; daemon/planner rewrites; new codec/crypto owner |
| #257 | search-unitizer | focused contract adapters/fixtures; exact ledger refresh; unitizer manifest/lock wiring to the existing search-materializer API and one named materialization ingress adapter | materializer/parser implementation; provisional second UnitSet; daemon/planner/store edits |
| #266 | search-runtime-owner | daemon owner_composition, narrow command assembly, legacy DirectStore open helpers; exact native/existing-secret/source-root-loader/shutdown adapters in the addendum below | query/source/Qdrant redesign; second root catalog |
| #235 | new search-provider-client | typed CLI client move/deletion, workspace membership/lockfile, narrow CLI wiring | daemon internals; loopback promotion; protocol redesign; TCP fallback |
| #238 | search-config | exact TOML/Serde pins/docs and the shared exact-byte SHA exception below | readiness/apply/input acquisition; changing existing digest domains |
| #241 | search-source-admission | exact GlobSet pin/lockfile/docs; reuse shared SHA compatibility API | ingestion, safe-reader/root/registry mutation; ignore walker |
| #246 | search-exact | exact matcher pins/lockfile/docs | denominator/access/handles; regex successor work |
| #252 | search-lexical | exact Unicode pins/lockfile/profile docs | prose/full folding; Qdrant migration; old-vector reinterpretation |
| #226-C1 | materializer + unitizer | Markdown pin/lockfile/fixtures after #257 | JATS/PDF/rendering; new UnitSet; worker framework |
| #226-C2 | same owners after C1 | XML pin/lockfile/JATS fixtures | Markdown rewrite; XML dynamic tree; external entities |
| #329 after #259/#262 | search-point-identity legacy retirement | narrow identity imports/arguments in migrated planner/bridge; daemon projection_composition/kernel/compose.rs and corresponding fixtures; exact ledger/docs | new schema/codec/transport; general daemon rewrite; final qualification claims |

Rows name existing packages at their current Cargo paths, not new top-level directories. The exact task body narrows the row further. All unlisted nearest-package prohibitions remain effective.

## #266 existing-root composition addendum

The [child-context addendum](https://github.com/UnknownAlienHuman/eliot-search/issues/266#issuecomment-6095063694)
retains the original request in the existing plaintext DIRECT store and passes its
checkpoints through the existing ingest, preparation, read/search, manifest and GC
bodies. Exact extensions name
[`direct_store_ingest/kernel/{entry,batch,plan,policy}.rs`, `preparation_store/kernel/batch.rs`
and `directory_manifest/kernel/{load,paths,persist,sync,migration}.rs`](https://github.com/UnknownAlienHuman/eliot-search/issues/266#issuecomment-6095091808),
[`maintenance.rs`, `maintenance_guard.rs` and `control_migration_objects.rs`](https://github.com/UnknownAlienHuman/eliot-search/issues/266#issuecomment-6095118478),
and the actual
[`storage_security.rs`, `source_migration_mapping.rs` and `control_migration_plan.rs`
consumers](https://github.com/UnknownAlienHuman/eliot-search/issues/266#issuecomment-6095208075).
Narrow service diagnostics/dispatch/mutation/query/reporting/runtime adapters pass
the existing store checkpoint; no query algorithm changes. `direct_store/{model,store}.rs`,
`direct_store_catalog.rs`, `secure_direct_store/kernel/{catalog,lifecycle,read,search}.rs`
and `development/kernel/owner.rs` bind the same request to the same borrowed owner.
Service commands may replace an expired startup/previous-command context only after
the exact borrowed owner is verified. No second secure-store request or timer is added.
Cancellation and uncertain manifest publication preserve pending objects for reconciliation.
This is cooperative checking around existing calls, not preemptible filesystem/vendor I/O.

The [operation-context addendum](https://github.com/UnknownAlienHuman/eliot-search/issues/266#issuecomment-6094825469)
includes `owner_composition/kernel/operation.rs` and narrow request-aware output/cancellation
in `public_runtime_service/kernel/session.rs`, with narrow native argument/deadline plumbing
in the same directory's `entry.rs` and `mutation.rs`. The context binds exact bounded native CLI
arguments, one original deadline and cancellation; it is not root authority. Service commands
borrow the same live owner. No new dependency, task registry or provider protocol is included.

The [lifetime/redaction addendum](https://github.com/UnknownAlienHuman/eliot-search/issues/266#issuecomment-6094629373)
retains the fixed native layout objects through admitted child lifetimes and bounds
`secure_commands/kernel/output.rs` failures to closed reason tokens. No new identity catalog
or mutation/recovery algorithm is introduced.

Explicit initialize/named-initialization-recovery wiring includes only
`owner_composition/kernel/initialization.rs` and the existing
`secure_commands/kernel/{support,dispatch}.rs`. The existing installation codec
may persist format 2 native control/lock bindings; ordinary format 1 opens cannot
silently upgrade. An exact retained intent fences unknown initialization outcomes.
Recovery never retries missing creates or credentials.

Existing migration child parameter wiring also covers `control_migration_plan.rs`,
`control_migration.rs`, `control_migration_cutover.rs` and
`control_migration_cutover/{marker_io,operation}.rs`. Only borrowed admission and
content-free marker/status inspection change; artifact/schema/replay owners remain.

The same administrative adapters retain the original request through source-history,
source mapping and directory/revision page boundaries. Existing page limits may only
tighten the admitted absolute deadline. Existing catalog replay adapters without a
request are fixture-only after cutover. No import algorithm, artifact lifecycle,
durable schema, hash preimage or recovery authority changes under this continuation.

The [owner issue addendum](https://github.com/UnknownAlienHuman/eliot-search/issues/266#issuecomment-6094110299)
extends only the admission/open boundary to these actual reverse-consumer adapters:

- `development/kernel/owner.rs`, `sealed_root_lock.rs` and `owner_composition/**`: existing-only
  exclusion/co-hold, bounded state inspection, explicit initialization and abandoned-owner fencing;
- `direct_store/store.rs`, `secure_direct_store/kernel/lifecycle.rs`,
  `revision_protection/protector/model.rs`, `revision_protection_windows.rs` and
  `revision_protection_windows/credential.rs`: existing-only catalog/credential opening, without
  changing encryption or secret semantics;
- `secure_direct_store/{kernel.rs,kernel/lifecycle.rs}`, `secure_direct_store.rs`,
  `secure_commands/kernel/{entry,output}.rs` and `app/kernel/{output,status}.rs`: typed child-open views and
  removal of duplicate private command wiring under the
  [exact extension](https://github.com/UnknownAlienHuman/eliot-search/issues/266#issuecomment-6094257692);
- `catalog_presence.rs`, `catalog_quarantine.rs` and `source_roots/kernel/catalog.rs`: admission
  barriers, exact named recovery and an existing-only registration loader; no source policy redesign;
- actual callers in `app/kernel/{commands,dispatch,source_root_commands}.rs`,
  `secure_commands/kernel/{store,commands,dispatch}.rs`, `public_runtime_service/kernel/{runtime,dispatch}.rs`,
  `preparation_composition.rs`, `source_migration_command.rs` and optional provider/control-migration
  entry wiring: borrow the same authority, stop admission and close children before `RELEASED`;
- owning focused fixtures/docs and exact changed ledger sites only.

All paths above are beneath `bins/eliot-searchd/src/`. No new dependency, root catalog, provider
protocol, source/query/Qdrant implementation or crypto profile is authorized. The parent remains the
only repository writer and integrator; code-writing subagents prepare disjoint scratch patches under
the maintainer's explicit instruction. An ordinary open cannot perform migration or initialize state.

## Current order

```text
#256 → #257 → #266 → #235
→ #238 → #241 → #246 → #252
→ #226 Markdown → #226 JATS
```

#324 is a separate documentation repair. #327 is a small security lockfile repair by the same manager before affected TLS/network acceptance. #329 executes after its planner/bridge consumers are ready and before #264 final indexed qualification; it must not create a dependency cycle.

## Build-safe identity migration: #256 then #329

The current daemon imports `PointIdentityRegistry`; its ownership test names that symbol. Removing it in a package-only #256 PR breaks consumers outside the old gate command.

#256 adds the stateless S11 profile first in a distinct cohesive module. Existing legacy APIs/bytes remain unchanged only for current consumers: no extension, new fallback, copied facade or admission into the new collection generation. Retained legacy ledger sites keep their pending classifications and move to executable removal owner #329, not to an already closed issue or an accepted classification.

After #259/#262 provide the real consumer APIs, #329 removes old callers and exports in one coherent integration series. #264 cannot qualify a new generation while normal routing can silently select legacy identity. Closing #256 accepts the new API, not the completed product cutover or retirement.

Check direct reverse consumers. Record existing diagnostics separately; a package-only PASS cannot conceal a new unresolved import or type error. Do not break downstream compilation merely to claim immediate code deletion.

## Exact-byte SHA compatibility: narrow #238 exception

Merged `search_contracts::sha256_raw` computes `SHA256(domain || NUL || bytes)`, not standard SHA-256 of the given bytes. Old config fingerprints already carry legacy framing inside their preimage. An additional prefix does not preserve v1 digests.

#238 may add one explicitly named bounded exact-byte SHA-256 primitive in the existing shared owner:

```text
crates/search-contracts/src/digest.rs
crates/search-contracts/src/lib.rs only for its export
focused shared digest fixtures/documentation
exact canonical-digest-ledger refresh
```

Use the same private RustCrypto donor, enforce the byte ceiling before hashing and add no prefix. Require official standard vectors and exact old-config-preimage parity. Document that a checksum grants no semantic authority. Existing raw-domain/canonical helpers and golden bytes remain unchanged. No new crate, generic receipt, unbounded reader or hashing framework.

Config v2 still uses a new closed canonical domain/profile. Exact-byte compatibility cannot relabel v1 as v2. A required old input beyond the accepted ceiling needs explicit legacy/unsupported handling or a separately reviewed streaming/size profile; never clamp or buffer unbounded input.

## Dependency and ledger discipline

The manager verifies exact source/checksum/license/MSRV/features/advisories, resolves root pins/Cargo.lock once, runs scoped locked check/strict Clippy and merges before the next slice. Reuse accepted dependencies instead of reselecting stale candidate pins.

Refresh only reviewed ledger sites. A closed implementation issue does not prove all old occurrences it once owned were migrated. Keep source hashes, classifications, phases and executable owners truthful.

## Stop rule

Changes outside an exact row require scope amendment before implementation. No exception creates another source, index, schema, digest, client, journal or qualification authority.
