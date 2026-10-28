# Analysis registry

Current checkpoint and evidence only. Algorithms live with their operation owners; this record grants no
implementation permission or runtime readiness.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**Checkpoint:** R056, 2026-10-28 — complete relative-link check and remaining instruction cutover.
**Branch:** `codex/refactor-preparation-20261017`, [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).
**Inspected parent:** `44ee1661833c1139f1e0f1486f365a3a523fe814` (R055).
**Verified source tree:** `7fb022f08bcc7cf76dba54889bbb402f40fcad92`.

This change is Markdown-only. Product code, manifests, lockfile and PR workflows are unchanged. The connected
application remains unimplemented; `main` retains the older generation until the PR merges.

Next implementation remains [Foundation](../implementation/README.md#1-foundation--invocation-settings-and-root),
then the one-file retained-source → Qdrant → CLI path. Read that stage, its owner recipes and actual callers.
Do not restart a broad audit or build optional systems before the working path. Preserve unpublished #598 work.

<a id="established-decisions--do-not-rediscover"></a>
## Current decisions

R055's physical/query-profile split, strict empty-init discard/retry, execution-only lower-I/O context and
whole-generation cleanup remain selected. Windows finalization is still an unexecuted candidate, not a safety
or durability guarantee. The [implementation guide](../implementation/README.md#owners) routes the owning rules.

| R056 correction | Owning rule |
|---|---|
| Window state and output | [Window](../../crates/search-query/search-continuation/FUNCTIONS.md#operation-ownership): owned pending state, no mutable map/window borrow across I/O, prepared post-flush commit, healthy-error restoration without refunding work. |
| Locators and lifetime | Continue/Expand retain the old view; internal evidence refs differ from emitted locators; session-local IDs are not reused; [protocol](../../crates/search-provider-protocol/FUNCTIONS.md) serializes terminal/control handling. |
| Stage-specific shared data | [Contracts](../../crates/search-contracts/FUNCTIONS.md): no mandatory sequence/validity/incarnation or unused future DTO family in Stage 2/3A. Point address is not complete identity. |
| Lexical baseline | [Lexical](../../crates/search-lexical/FUNCTIONS.md): source/context once, no local alias inflation or second BM25 implementation. |
| Bootstrap versus Search | [Contexts](../implementation/README.md#minimal-cross-owner-values): root/connection/publication do not require a view; only Search adds scoring. |
| Qdrant transport | [Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md): Rust gRPC endpoint, not the REST port; target examples are not current runnable settings. |
| Historical frameworks | Admission V2 descriptions are history, not permanent contracts; optional Research export has one consumer-driven note, no activation/receipt/size/wave framework. |

<a id="coverage-ledger"></a>
## Documentation work

The full R055 Markdown navigation pass found eight missing anchors. Config, control, Exact and historical audit
links now resolve. Follow-up semantic review covered query/session/handles/protocol, contracts/point identity,
lexical guidance, bootstrap context, nested root instructions, test ownership and optional export/admission notes.
It removed competing instructions rather than introducing new runtime services or verification gates.

Deleted only the two obsolete instruction files in the documentation-only Research export directory. Kept its
short design note and immutable history link. Four admission V2 pages now point to the current recipe and their
exact historical descriptions; no corresponding Rust or user data was deleted.

<a id="delivery-remaining"></a>
## Remaining product work

[Delivery sequence](../implementation/README.md#delivery-sequence) remains the sole stage/order owner: Foundation,
one-file Search, restart/inventory and complete generations, measured conditional incrementality, Exact/session,
then selected depth. No product stage is completed by this documentation change.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence status

| Check | Result / limit |
|---|---|
| Exact input | R055 archive's commit header and reconstructed Git tree matched the parent and tree above. |
| Relative Markdown navigation | Before: 202 files, 578 relative links, 8 broken anchors. After: 200 files, 599 relative links, 0 missing paths/anchors. CommonMark links/table links, generated heading IDs and explicit HTML anchors checked; external URLs were not crawled. |
| Diff | `git diff --check` passed; final change contains Markdown only. |
| Rule agreement | Listed owner/caller-document corrections checked statically; not a proof that every remaining design or source path is correct. |
| Snapshot export | [Run 40787301463](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/40787301463) exported the fixed parent without executing project code. The temporary helper branch was removed; its workflow is not part of PR #610. |
| Qdrant | Endpoint guidance checked against official installation/Rust quickstart; lock remains client 1.19.0. Selected 1.20.x integration remains uncompiled/unexecuted. |
| Compiler history | R031 materializer/unitizer check + minimal Clippy, [run 40084893130](https://github.com/UnknownAlienHuman/eliot-search/actions/runs/40084893130), is narrow historical evidence only. |
| This change | No Rust build/Clippy/test suite executed; no verification framework or permanent workflow added. |
| Product/runtime | Foundation/current binaries, Windows finalization/durability, end-to-end Search, restart, relevance and scale remain NOT_RUN. |

<a id="evidence-index"></a>
## Prior evidence and maintenance

[R055 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/docs/audit/ANALYSIS_REGISTRY.md)
links earlier checkpoints and exact source readsets. The [donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md)
records upstream evidence separately from installed and executed behavior.

Keep one current checkpoint: inspected revision, changed owners, actual checks and unrun work. Preserve prior
readsets through immutable links. Change algorithms at their owner; update incoming links and conflicting issue
bodies in the same slice. Do not copy algorithms or an accumulating job history into this registry.
