# Current implementation status

**Snapshot:** `main` at `3ccef79a6850a9fd4caa7941c40e4a331cf4e0dc`, 2026-10-08.  
**Coordinator:** [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97).  
**Machine-readable inventory:** [`PACKAGE_STATUS.toml`](PACKAGE_STATUS.toml).

This document separates contract definition, source presence, product integration, execution evidence,
native qualification and public enablement. It is not a release receipt.

## Status vocabulary

| State | Meaning |
|---|---|
| `CONTRACT_DEFINED` | A current type, operation or product contract exists. |
| `SOURCE_PRESENT` | Substantive source exists in the active workspace. |
| `PRODUCT_PATH_INTEGRATED` | The supported daemon/CLI path actually uses the source. |
| `CHECK_CLIPPY_EVIDENCE` | Exact commands and outcomes exist for the named revision. |
| `LIVE_NATIVE_QUALIFIED` | Required real OS/process/Qdrant/fault evidence passed. |
| `PUBLIC_CAPABILITY_ENABLED` | The supported product edge truthfully exposes the capability. |
| `LEGACY` | Source exists under an obsolete identity, schema or product path. |
| `SUPERSEDED` | A task or implementation alternative has a selected successor. |
| `DISABLED` | The product deliberately refuses the capability. |
| `NOT_RUN` | Required execution evidence is absent. |
| `UNAVAILABLE` | A required dependency or effect owner is unavailable. |
| `UNQUALIFIED` | Source/integration may exist, but acceptance evidence is incomplete. |

```text
SOURCE_PRESENT != PRODUCT_PATH_INTEGRATED
CHECK_CLIPPY_EVIDENCE != LIVE_NATIVE_QUALIFIED
LIVE_NATIVE_QUALIFIED != PUBLIC_CAPABILITY_ENABLED
```

## Current product boundary

ELIOT Search is a standalone local-first, source-backed analysis and retrieval product. Qdrant owns
the physical indexed substrate. ELIOT owns source/revision authority, materialization, UnitSets,
projection manifests, access scopes, deterministic planning/ranking, exact source validation,
provenance, coverage, handles and lifecycle state.

It is not a controller, hosted crawler or a second search database around Qdrant.

## Selected implementation graph

```text
#237 canonical values/digests
→ #207 exact point identity
→ #209 complete projection manifest
→ #210 immutable generation publication/recovery
→ #200 bounded Qdrant bridge
→ #125/#127 access/query/provider/daemon composition
```

A payload, identity, index, manifest or epoch-contract change creates a new collection generation.
Legacy points are rebuilt; they are never relabelled.

Closed alternatives `#197`, `#202`, `#204` and `#211` are not active implementation choices.

## Product-layer order

```text
#218 immutable corpus/portfolio/scope authority
→ #213 retrieve_evidence@1 + orient_scope@1
→ #221 deterministic ranking profile
→ #219 thin optional agent adapter
```

The existing eleven `RecipeIdV1` meanings remain stable. New orientation/retrieval contracts are
additive. Current floating/order-sensitive RRF helpers are not an accepted ranking profile.

## Source and currentness order

```text
#100/#104 native data-root/final-handle authority
+ #237 canonical identities
+ #241 compiled admission policy
+ #239 watcher hints/gaps
+ #110 durable ingestion
→ #128 coherent multi-root SourceView currentness
→ #218 immutable authorized scopes
```

A file parent is not an admitted root. Watcher quietness is not currentness. Missing or partial
inventory is not deletion evidence. Denied bodies must not be acquired before content-free
pre-admission.

## Code and document donors

```text
#223 Tree-sitter structural model
→ #224 SCIP importer
→ optional #225 Ctags / #236 SARIF

#226 inert Markdown/JATS
→ #227 isolated document worker
→ independent provider/profile qualification under #216
```

Donor source stays private behind ELIOT-owned bounded profiles. Donor defaults, network access,
runtime downloads and assurance upgrades are not accepted implicitly.

## Qualification graph

```text
#233 immutable queries/qrels/results
→ #234 verified statements/reviewer policy
→ #240 installed-product runner and raw resource/effect evidence
→ #137 Product Pulse
→ #215 preregistered qualification tiers
```

Package fixtures, source review, a green command at another revision or a development loopback path
do not satisfy installed-product qualification.

## Historical evidence boundary

Historical evidence retained by the repository includes:

- Windows Rust 1.98 daemon check at source revision `3a5d92f`, with warnings;
- strict-Clippy evidence for selected owner packages at named revisions;
- failed strict-Clippy/binary-test compilation at saved integration revision `69ba90d`.

Those observations remain attached to their exact revisions. This status cutover performs no new
Cargo, Clippy, test, Windows, Qdrant or installed-product execution.

## Legacy issue-wave retirement

The early P00/P01/P02/W0/W1/W2 issues `#51–#89` are closed as `SUPERSEDED`, not as qualification.
Their remaining work is assigned as follows:

```text
config                         #238 → #109
admission                      #241 → #110
source registry/currentness    #110 → #128 → #218
provider client/transport      #235 → #116
redb                           #106 → #108
runtime/root                   #100/#104/#105/#120
secrets                        #115/#120
safe reader/Git                #104/#129
revision/retention/restore     #111/#134/#243 → #244 → #245
materialization/documents      #112/#113/#216/#226/#227
installed qualification        #240 → #215
```

Issue `#48` is closed: the normative P00 type registry already contains the exact named forms for
`UtcTimestamp`, `MetadataKey` and `UnresolvedSource`. The stale reconciliation note was the defect.

## Agent authority order

```text
root/package AGENTS.md
→ #97 coordinator
→ exact current owner issue/PR
→ PACKAGE_STATUS.toml
→ current source
→ package README responsibility notes
→ historical handoff/issues only for archaeology
```

A stale issue or README never authorizes a second package, parser, catalog, client, protocol, watcher,
restore journal, search index or release workflow.

## Release state

The workspace contains substantial source, but the complete installed Windows product, canonical
indexed pipeline, lifecycle recovery and all advertised optional profiles remain unqualified until
their exact owner and #215 evidence requirements pass.
