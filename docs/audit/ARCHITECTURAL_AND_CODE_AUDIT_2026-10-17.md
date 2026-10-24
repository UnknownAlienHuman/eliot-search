# ELIOT Search — distilled architecture and code audit

**Source baseline:** `c8e38b6fc9829f1cd38070773b1b3e5b21de18ad`.
**Replacement documentation rechecked:** `de06d75e534b912bdff70e84fa4bcb53d1d91a42`, 2026-10-21.
Static evidence and design review only. No exhaustive, build, native or runtime qualification claim.

This report records reasons and counterexamples, not a second implementation specification. The
[manifesto](../architecture/README.md) and [implementation](../implementation/README.md) direct the work.
The [coverage record](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) states limits, not permission to code.
The [previous full report](https://github.com/UnknownAlienHuman/eliot-search/blob/8106347f18cf9b8ccc9e6ab9041efd2878ee82d3/docs/audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md)
remains in Git; its copied algorithms are superseded, notably online ObjectGcGate collection.

## 1. Executive conclusion

The primary defect is disconnected and overlapping ownership, not too few lines or insufficient proof types.
Source retention, preparation, Qdrant, evidence and the real CLI must form one application. A fake grant,
receipt, capability or adapter does not connect a missing producer.

The audit itself repeated this failure when it copied algorithms into reports/issues, declared closure from
package dispositions, and removed useful failure/lifetime requirements together with obsolete machinery.
Deleting a complex mechanism is correct only if its necessary observable behavior has a smaller owner or an
explicitly stated unsupported case. A one-file milestone does not replace the product's original goals.

## 2. Measured baseline

These are prior measurements at the inspected baseline, not a new recount or promised net deletion:

| Family | Inventory |
|---|---:|
| Workspace | 52 members |
| Tracked Rust | 1,593 files / 393,383 physical lines, including tests/comments/blanks |
| Daemon Rust | 604 files / 122,212 physical lines |
| Test-named Rust outside xtask | 359 files / 101,276 physical lines |
| xtask | 123 Rust files / 23,823 physical lines |
| Digest ledger | 3,572,618 bytes / 5,960 sites |
| swarm coverage | 205 files / 152,921 lines |

Gross families: source admission/history/registry >1.22 MiB; root/composition about 553.6 KiB; query proof/host
>300 KiB; protocol stacks >500 KiB plus contracts about 486 KiB. These figures do not establish how much
correct production behavior can be deleted. Receiving owners and actual callers decide that.

## 3. Product topology and entrypoints

The baseline has overlapping DIRECT/native/migration/shell services, feature ladders and harness binaries.
The real Qdrant plane is not the normal daemon query path; process fixtures use an oracle or manually join
halves of the product. CLI provider DTOs can still become legacy tab/hex commands.

Source/caller maps: [daemon](../../bins/eliot-searchd/FUNCTIONS.md), [CLI](../../bins/eliot-search/FUNCTIONS.md).
Do not treat removal of cfg(test), changing a default feature or passing constructor fixtures as integration.

## 4. Root, configuration and current control state

A real process lock is wrapped by epochs, slots, succession and receipts. Configuration activation can
reject a valid startup override without a live-apply receipt. The control journal retains completed
operations, reconstructs full snapshots and restores original registry inputs before small mutations.

Current recipes: [root](../../crates/search-runtime/search-runtime-owner/FUNCTIONS.md),
[config](../../crates/search-config/FUNCTIONS.md), [control](../../crates/search-control-redb/FUNCTIONS.md).
They must retain actual exclusion/transaction semantics, not the obsolete authority vocabulary.

## 5. Source acquisition, identity and reconciliation

The baseline acquisition reads/hashes/classifies bytes but does not hand the body to retention. Sync can end
in `SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE`. Receipt/provenance/history packages do not compensate for that gap.
Watcher hints are not complete inventories, and editor move/remove events are not definitive retirement.

The replacement's head-commit -> queue arrow also omitted crash recovery and stale-worker completion.
[Preparation](../../crates/search-prep/search-materializer/FUNCTIONS.md) now derives outstanding work from
current inputs/outcomes and conditionally installs only a still-current result. This is a design correction,
not a reproduced queue failure in the unimplemented new path.

## 6. Immutable objects and storage protection

Revision store and materializer duplicate staging/sync/finalization/readback. Windows directory-sync success
can mask a no-op. Final readback does not demonstrate sudden-power-loss durability.

Keep one [object owner](../../crates/search-source/search-revision-store/FUNCTIONS.md). Plain storage needs
no key platform. Removing six-domain/purge/receipt machinery does not waive the actual selected encryption
algorithm's nonce and binding rules. Private #598 code remains outside this audit; only its public contract
was read. Do not adopt that old programme or claim its implementation was verified.

## 7. Preparation and incremental identity

Adjacent Markdown/JATS stages can repeat full materialization; buffers/manifests are copied and encoded
repeatedly. Revision/absolute offsets/ordinal in UnitId and S11 v1 amplify a local edit into point churn.

Current [unit matching](../../crates/search-prep/search-unitizer/FUNCTIONS.md) conservatively reuses only
unambiguous matches. Duplicate ordinal is not continuity. Content/model-input reuse and occurrence birth
are separate. Exact evidence comes from the published snapshot, not a guessed stable location.

The baseline [projection check](../../crates/search-index-qdrant/search-projection-planner/src/plan.rs)
rejects an empty UnitSet; [manifest validation](../../crates/search-index-qdrant/search-projection-planner/src/manifest_read.rs)
also rejects an empty manifest. A rewrite must explicitly support successful zero-unit input without
converting failed/unsupported extraction into empty success. Supported mapped/lossy representations remain
usable with coordinates/gaps; they do not prove raw-source completeness.

## 8. Qdrant bridge and lexical search

The existing real SDK path is burdened by qualification/auth equality records, process-local replay receipts,
relative deadlines, fixed schema floors and echoed evidence digests. Full upsert versus narrow mutation and
payload merge versus overwrite have different semantics.

The [bridge recipe](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) uses concrete Qdrant
behavior and actual readback. Native BM25/IDF/fusion are donor mechanisms, not proof that runtime integration,
quality or scale has been measured. Partial expected batch progress must not be classified as corruption.
The latest review also distinguishes native generated-vector readback from known-input vector equality;
see section 18. An SDK's ability to update payload/vectors is not permission to alter a published view.

## 9. Publication, view lifetime and cleanup

Three old publication machines hold labels/receipts rather than one real apply/commit path. Raw old-view
acquisition and sampled watermarks can race cleanup. Waiting only for the immediate predecessor misses an
earlier live view.

[Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) owns effect order and
snapshot-backed evidence. [Stopped-root collection](../../crates/search-runtime/search-retention/FUNCTIONS.md)
replaces the rejected online full-root scan per deletion page. This report no longer repeats either
algorithm. Old redb readers, offline maintenance downtime and scratch disk remain explicit costs.
Reader lifetime is only half the condition: an unresolved writer can recreate retired state after a readback
or deletion. Section 18 records why our simplified recovery previously failed to account for it.

## 10. Query, evidence and paging

Baseline defects include leg-prefixed identity, projector raw-score resorting, digest fields echoed beside
unverified bytes, whole-catalog fences per page, prefix rescans, arbitrary revision-range handles and exact
count applied to non-exhaustive top-k.

Use [query](../../crates/search-query/README.md) and [window](../../crates/search-query/search-continuation/FUNCTIONS.md)
recipes. A valid empty published inventory is different from unavailable backend, failed build or incomplete
source coverage. Top-k never proves absence; exact negative evidence requires its stated evaluated inventory.

## 11. Protocol, client and CLI

The old stack duplicates paired/manual-JSON/recipe/provider and line protocols. Our replacement initially
closed the child while returning live locators, blocked the Cancel reader, confused request deadline with
window TTL, and treated local flush as peer receipt. These were defects in the new instructions.

The [protocol map](../../crates/search-provider-protocol/FUNCTIONS.md) now owns these semantics. The actual
[deadline adapter](../../bins/eliot-searchd/src/public_runtime_service/kernel/output_deadline.rs) explicitly
cannot interrupt a blocked write. Capped allocation and real cancellation must be implemented, not asserted
by a wrapper name.

## 12. Shared contracts, ports and domain

Shared contracts catalogued planned recipes, grants, lifecycles and vendor-like schemas. Global ports added
23 traits and about 80 operations, often with fixture implementations only. Domain rules drifted from their
real owners. See [contracts](../../crates/search-contracts/README.md) and the
[workspace disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md).

Package labels/pseudocode are not instructions to generate a trait/service per noun or to rewrite healthy
code. A useful isolated dependency/effect may justify a boundary; a dead test or hypothetical backend does not.

## 13. Tests, tooling and generated governance

Maps, digest approvals, P00 tickets, frozen package counts and module-shape tests preserve the discarded
architecture. See [tooling disposition](../../tools/README.md) and [test cutover](../implementation/TEST_CUTOVER.md).
Preserve causal scenarios, not obsolete source spelling. No audit form, per-file stamp or new acceptance
registry is introduced by this review.

## 14. Optional features and donors

Refusal workers and disconnected overlay/code-intelligence/evaluation frameworks are not capabilities.
Their deletion does not cancel navigation/comparison/documents/large-corpus goals. Keep a goal, a selected
concrete seam and honest gaps, not empty packages for every future feature.

The [donor register](DONOR_VERIFICATION_REGISTER_2026-10-09.md) records concrete versions/mechanisms, not an
exhaustive market survey. Standard libraries replace machinery, not source meaning, restore isolation or
correct liveness. RustCrypto's nonce requirement is checked against
[aes-gcm 0.10.3 usage](https://docs.rs/aes-gcm/0.10.3/aes_gcm/#usage), not a claim of a dependency upgrade.
Qdrant checks in section 18 use the project's 1.19.0 source and official API documentation fetched 2026-10-21;
no vendor upgrade or live-server experiment was performed.

## 15. Implementation order

The [implementation guide](../implementation/README.md) owns dependency order. There is no second queue here.
Protect real private #598 overlap; independent work need not wait for its old programme. Remove replaced
code/docs/tests with actual consumers and run minimal compiler/Clippy after code.

## 16. What this audit does not prove

No full-line or complete-file review, private #598 inspection, build, Windows durability, live Qdrant,
crash/recovery run, performance result or exhaustive contradiction sweep is claimed. A table of package
outcomes is not proof of correctness. Each below counterexample still needs a causal implementation check.

## 17. Old requirements versus mistakes in the replacement

| Evidence inspected in the preceding pass | Replacement defect / counterexample | Useful requirement retained without the old framework |
|---|---|---|
| Old [S28.1/S28.2](https://github.com/UnknownAlienHuman/eliot-search/blob/c8e38b6fc9829f1cd38070773b1b3e5b21de18ad/docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md#s281-rebuildability-first); restore `kernel/lifecycle.rs`; #244 at 2026-10-21 | A copied root is reopened normally while its pending/cleanup still names the original collection. Two different local locks do not isolate that collection. | Explicit archive/import creates a fresh destination generation and never replays copied external work. Original retained evidence remains intact. No paired-snapshot or quarantine service. |
| Public #598 deterministic AES-GCM counter contract; RustCrypto algorithm requirement | Same-user access to a copied key is mistaken for permission to resume a copied/rolled-back nonce counter. | Readability and writable continuity are separate. No same-key/counter writable fork; require supported independent destination key state or explicit unsupported restore. Private implementation untouched. |
| #270's committed head then enqueue; preparation map at 8106347; daemon `preparation_composition.rs` | Crash loses the wake-up; old worker completion overwrites newer current preparation; unchanged unsupported input can retry indefinitely. | Current-input/outcome reconciliation, conditional completion and finite current retry disposition. Queue remains disposable; no durable job/receipt archive. |
| `validate_scope_binding`, `validate_projection_manifest_metadata`; unitizer nonempty baseline | Empty file or last-source removal is rejected, or a failed extraction is incorrectly treated as empty. | Distinguish complete empty, unsupported/incomplete and unavailable index. Publish legitimate zero-unit changes without sentinels; preserve explicit gaps. |
| Publication recovery recipe used one 'operation deadline' | Restarted durable work is tied to an expired Instant, or nested retries renew work forever. | Each recovery attempt has one finite budget; durable input/births/cursor remain fixed between attempts. |
| This report's own section 9 at 8106347 | Audit still prescribed ObjectGcGate after the owning recipe moved to stopped-root collection. | Audit stores observations and links, not a competing copied algorithm. A historical fact is not an instruction. |

These are static source facts plus explicit counterexamples to the proposed design. They are not reports of
observed production data loss. Owner recipes contain the corrections; this material explains why the new
documentation must retain the behavior while dropping the former mechanism.

## 18. Qdrant lifetime, late writes and native-inference readback

This pass rechecked the old architecture's INV-11/12/14, replacement bridge/publication/planner instructions
at de06d75e, public bridge code, #263/#303 and Qdrant 1.19.0 source. It does not assert that the disconnected
new product was run or that the counterexamples were reproduced against a live server.

| Evidence | Defect in the replacement | Owning correction |
|---|---|---|
| Planner says changed retrieval input creates a new point; bridge mutation table allows replacing vectors/filter payload on existing points | An old redb snapshot preserves citation rows but not mutated Qdrant retrieval state. A request can rank/filter on new data while resolving old evidence. | [Bridge](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#immutable-retrieval-state-versus-versioned-evidence): published retrieval state is immutable; one fixed future-validity closure is the exception. |
| `real/mutations/upsert.rs` uses full default UpsertPoints under a local timeout; `publication/FUNCTIONS.md` previously treated desired readback as done | A timed-out original request can remain outstanding while a retry is observed, then arrive after a later closure/deletion. Readback observes present state, not absence of future old effects. | [Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md#apply-observed-state-is-not-dispatch-completion): preserve possible-dispatch state, settle writers or isolate recovery in a fresh physical generation. |
| [Qdrant 1.19.0 update_from_client](https://github.com/qdrant/qdrant/blob/v1.19.0/lib/collection/src/collection/point_ops.rs) dispatches through update_runtime.spawn; official docs describe insert-only and ordering | Client cancellation/reconnect and a local serialized writer are not server completion. Insert-only ignores an existing ID but can insert it again after physical deletion; strong ordering is not the application's old-request version. | Retain the late-writer counterexample. No speculative competing retry or cleanup-complete claim from one absence read; bound unresolved generations/backlog. |
| `real/mutations/readback.rs` compares decoded PointRecord to expected explicit vectors; replacement selects native Document input and forbids a duplicate encoder | The instruction 'compare actual vectors' omitted where expected native coefficients come from. It could induce a second BM25 implementation, fake digest or unfulfillable gate. | [Bridge readback](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#native-lexical-input-and-bounded-readback): known-vector equality versus native generated-vector shape/binding validation, with the selected server's inference trust explicit. |
| Bridge target API and #263 enumerate every vector/payload mutation despite the new-point/closure baseline | A donor capability list again becomes instructions to build unused wrappers; a narrow API can still violate view semantics. | Implement only connected operations. Keep SDK operation differences as reference, not a mandatory framework. |
| Bridge/#263 still reject every empty scope after the preceding pass accepted empty published states | Successful last-source removal or explicit empty selection remains inconsistent at the RPC boundary. | Known empty returns empty without RPC; unknown IDs and missing/incomplete state remain errors/gaps. Empty never becomes an omitted wildcard filter. |

The donor facts are supported by [native BM25](https://qdrant.tech/documentation/inference/inference-bm25/),
[point update semantics](https://qdrant.tech/documentation/manage-data/points/),
[write ordering](https://qdrant.tech/documentation/scaling/consistency-guarantees/) and
[1.19 filtered IDF](https://qdrant.tech/documentation/manage-data/multitenancy/#per-tenant-idf-statistics).
The native-inference contract does not independently prove numerical embedding correctness. The isolated
recovery choice sacrifices rebuild time and may retain unresolved garbage; it does not promise free or
unbounded automatic recovery. Those costs and the late-writer behavior need implementation evidence.
