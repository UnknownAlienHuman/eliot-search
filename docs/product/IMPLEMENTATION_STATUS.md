# Current implementation status

**Snapshot:** `main` at `50b1e1b4f561e8f4edac86c4a9882c32fe613356`, 2026-10-03.

This file records the difference between a documented contract, source code, product integration and
qualification. It is not a release receipt. Update it whenever a load-bearing package or public
capability changes.

## Status vocabulary

| State | Meaning |
|---|---|
| `CONTRACT` | A current product/operation contract exists. |
| `SOURCE` | A substantive Rust kernel or adapter exists in the active tree. |
| `INTEGRATED` | The supported daemon/CLI path actually calls the implementation. |
| `CHECKED` | The exact revision has recorded package/workspace compile and strict Clippy evidence. |
| `QUALIFIED` | Required real Qdrant/native/process/fault evidence has passed at the exact revision. |
| `ENABLED` | The capability is truthfully advertised and available through the supported product edge. |
| `LEGACY` | Source exists but implements an obsolete or noncanonical contract and must not be published. |
| `DISABLED` | The product deliberately refuses the capability until qualification exists. |
| `NOT_RUN` | Required execution evidence is absent. |

`SOURCE` is not `INTEGRATED`; `CHECKED` is not `QUALIFIED`; a draft PR is not part of `main`; file
presence is never `ENABLED`.

## Product spine

| Capability | Contract | Active source | Product integration | Qualification / enabled state |
|---|---|---|---|---|
| Standalone product boundary | ADR 0005/0006 | root daemon/CLI and provider packages exist | partial development/DIRECT paths exist | not a qualified release |
| Source admission, identity and registry | Architecture S7/S16 | substantial kernels exist | primary paths remain under reconciliation/cutover work | native/restart product qualification incomplete |
| Immutable revision/preparation state | S6/S15/S17 | revision, materializer and unitizer kernels exist | mixed legacy/canonical composition remains | end-to-end durable source spine not qualified |
| Deterministic sparse lexical encoding | S12 | analyzer, profile validation, mapping, weighting, `encode_document` and `encode_query` exist | not yet installed as one accepted live collection/query profile | no accepted lexical profile/real product qualification |
| Rust structural enrichment | S17/S21 | bounded no-execute Rust syntax/fact/relation kernel exists | no accepted parser profile is wired through the product | parser artifact/profile qualification absent |
| Real Qdrant transport | S9/S10 | `search-qdrant-bridge::real::RealDataPlane` and live probes exist | daemon query composition still documents the process-test/oracle path | exact product route not qualified/enabled |
| Point identity | S11 | current `main` contains a legacy profile using the old key/digest model | not suitable for canonical collection publication | canonical replacement is draft PR #207 |
| Projection planning | S9.5/S11.3 | current `main` contains a legacy payload/manifest planner | not suitable for canonical collection publication | canonical replacement is draft PR #209 |
| Publication coordinator | S13 | substantial pure coordinator/recovery source exists | authoritative redb + real-Qdrant effect composition remains incomplete | live crash/restart qualification absent; draft PR #210 aligns manifests |
| Access-scoped indexed retrieval | S19/S21 | planner, executor, candidate validator and Qdrant query kernels exist | supported daemon path is not yet the complete real-Qdrant pipeline | not enabled; #200/#211 and dependent integration remain |
| Existing eleven v1 recipes | S20 | contracts and multiple kernels exist | public provider/CLI execution is incomplete | capability-by-capability qualification absent |
| Free-text agent retrieval and scope orientation | ADR 0006 | no accepted public contract/implementation | absent | tracked by #213 |
| Baseline agent ranking profile | S12/S21 | primitives exist; one complete profile is not frozen | absent | tracked by #221 |
| Corpus and repository-portfolio management | S7.7/S19 | identities and registry models exist | no complete supported fleet-management UX | tracked by #218 |
| Generic agent tool adapter | S32 leaf-adapter boundary | generic provider protocol exists | no accepted MCP/JSON/stdio leaf adapter | tracked by #219 |
| Exact verification plane | S25 | literal/proof kernels exist | complete public denominator/readback path remains incomplete | product proof qualification absent |
| Large multi-repository analysis | ADR 0006 | portfolio/fusion/lineage primitives exist | no accepted fleet-scale product path | tracked by #215 |
| Research/document analysis | S17 optional profile | document worker deliberately refuses activation | absent | `DISABLED`; tracked by #216 |
| Optional dense/rerank/multivector | S29 optional profile | model contract/worker shell exists | no qualified model path | `DISABLED` |
| Controller cleanup | ADR 0005 | residual xtask/workflow/schema surfaces remain | not a product capability | removal tracked by #214 |
| Installed Windows release | S37 / T43 | source tree only | incomplete | not qualified or enabled |

## Current Qdrant alignment warning

The active tree is not yet one coherent canonical collection generation:

- the real bridge exists, but current daemon indexed composition still describes an in-memory/process-test
  port rather than the final live adapter;
- the active point-identity and projection-planner source use legacy fields/profiles;
- draft PRs #200, #207, #209 and #210 contain the aligned S9.5/S10.3/S11 package stack;
- PR #211 contains daemon compile/topology follow-up over that stack;
- issue #205 still requires a contract decision for exact epoch range transport.

Do not build a new collection from a mixture of legacy `main` identities/payloads and aligned draft
contracts. Integrate the stack atomically, mint a new collection generation and execute the real Qdrant
qualification.

## Current agent-product gaps

The product cannot yet honestly promise the full workflow described by ADR 0006:

```text
register a repository fleet
→ create an immutable portfolio
→ ask an ordinary bounded question
→ receive orientation/recommended reading
→ inspect and compare evidence
→ run exact verification
→ expand exact source-backed ranges
```

The missing contracts and user paths are split deliberately:

- #213 — free-text retrieval and scope orientation;
- #218 — corpus/repository-portfolio management;
- #219 — generic agent tool adapter;
- #221 — concrete lexical/structural ranking profile;
- #215 — large-fleet scale/quality qualification;
- #216 — research/document profiles.

## Documentation debt

The combined architecture master still contains a historical Part II handoff, and many package READMEs
use obsolete single-state wording such as `intentionally unimplemented`. Current entry points override
that wording, but the files themselves must be reconciled under #220. Residual controller tooling and
authority prose are removed under #214.

## Required next implementation order

1. keep controller cleanup separate from product code (#214);
2. integrate the canonical Qdrant identity → projection → publication → daemon stack;
3. obtain exact-head daemon check and strict Clippy;
4. execute real source → durable state → Qdrant → validated-result qualification;
5. finish the existing v1 public recipe paths;
6. add #218, #213, #221 and #219 in dependency order;
7. execute #215 and enabled document-profile qualification from #216;
8. complete lifecycle, Windows native and installed-release gates.

Until those steps pass, the repository is an advanced implementation candidate, not a finished product.
