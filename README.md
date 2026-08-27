# ELIOT Search

**Local-first data preparation and retrieval provider for ELIOT Memory OS.**

> **Status: structure scaffold.** This repository currently contains the directory layout, ownership
> notes and boundary description only. No implementation, no architecture master, no contracts.
> Continuous integration is deliberately disabled until the contract freeze lands.

---

## What this is

ELIOT Search prepares local data and answers retrieval requests. It is a separate product with its own
repository, its own delivery gates and its own release cadence. ELIOT Memory OS reaches it through a
typed provider contract, never through shared credentials or a shared database.

It owns:

- local source registration and observation;
- source identity, revision occurrences and membership bindings;
- safe no-execute reads, materialization, unitization and code/document enrichment;
- exact, lexical, structural and optional semantic retrieval projections;
- currentness, publication, overlays and coherent source readback;
- typed local-data recipes and compact evidence-oriented result cards;
- local index lifecycle, purge and rebuild.

## What this is not

- **Not the memory system.** It never receives canonical database credentials and never writes
  cognitive state.
- **Not an online research service.** External acquisition, large corpora and long-running
  investigations belong to ELIOT Research.
- **Not a research controller.** Planning and swarm execution stay inside ELIOT Memory OS.
- **Not an authority.** Search returns candidates, coverage and uncertainty. It does not decide what
  ELIOT should believe, and it never returns an ELIOT admission disposition.

## Product boundary

Four contours that must not be conflated:

| Contour | Owns | Repository |
|---|---|---|
| **ELIOT Memory OS** | Principal, WorkScope, task, authority, canonical records, Context Compiler, Governor, Dreamer, verification and completion | `eliot-memory-os` |
| **ELIOT Search** | Local source preparation and retrieval — this repository | `eliot-search` |
| **ELIOT Research** | External corpora, acquisition at scale, investigations and research publications | `eliot-research` |
| **Inquiry discipline** | Protocol, evidence grade, coverage denominator and claim audit | inside ELIOT Memory OS |

Search and Research are **pluggable providers**. Neither is required for the first ELIOT spine.
An absent provider narrows declared coverage and is reported as a gap; it never transfers its
responsibility to another owner and never blocks unrelated local work.

## Non-negotiable invariants

A change that violates one of these requires a new architecture revision, not a local workaround.

```text
one search/index database; the control journal is never a searchable corpus;
original bytes or an immutable admitted revision are the only source truth;
retrieval proposes candidates; ELIOT admits influence;
one projection membership per point; no membership arrays in a payload;
access and currentness filters apply before candidate generation and scoring statistics;
indexed top-k never narrows the denominator of an exact negative proof;
restrictive access and purge fences override query snapshots immediately;
an uncommitted epoch is never observable as current, and an epoch number is never reused;
publication writes are acknowledged and read back before commit;
no source range is called exact without a declared coordinate basis and revision digest;
an unsaved editor buffer is never inferred from a filesystem watcher;
index loss cannot destroy load-bearing ELIOT evidence;
stale or inaccessible candidates are removed before result projection;
a workspace is never called current across an unresolved observation gap.
```

## Repository layout

```text
docs/
  architecture/   authoritative architecture master (single file)
  handoff/        implementation handoff and PR delivery graph
  adr/            architecture decision records
  contracts/      hand-written contract notes not yet generated
  generated/      generated schemas, registries and descriptors — never hand-edited

crates/
  search-contracts       C00  versioned types, recipes, reason codes
  search-domain               pure state machines, validation, ranking rules
  search-control-redb    C02  bounded durable control journal
  search-source          C03–C07  registry, identity, reconciler, safe reader, revision store
  search-prep            C08–C10  materializer, unitizer, code enricher
  search-lexical         C11  deterministic lexical encoder behind a port
  search-index-qdrant    C13–C17  projection, point identity, bridge, publication, pins
  search-query           C18–C27  access, overlay, exact plane, planner, executor, projector
  search-runtime         C01, C28  data-root owner, retention and purge
  search-eliot-adapter   C30  provider protocol translation
  search-eval            C29  content-minimized telemetry and evaluation

bins/
  eliot-searchd                  the daemon; sole owner of the data root
  eliot-search                   standalone CLI over the same daemon
  eliot-search-model-worker      optional; dense/rerank profiles
  eliot-search-doc-worker        optional; document materialization

tests/
  control-corpus/  acceptance corpus
  fixtures/        recorded deterministic fixtures
  property/        property and fault proofs

tools/             development-only generators and drivers
```

A capability cell is a causal responsibility with a contract, an owner, a failure state and a test
seam. A cell does not automatically imply a crate, and a crate may host several cells. Each crate
README states what it owns and — equally important — what it must not own.

## Delivery gates

Depth is added only after the layer beneath it is proven.

| Gate | Proven |
|---|---|
| **G0 Contract** | Identity, membership, epoch, anchors, recipes, reason codes and dependency direction compile and pass pure property tests |
| **G1 Direct** | One daemon owns the root; inventory, exact reads and scans, revision store and no-execute policy work with no index at all |
| **G2 Lexical** | Qualified index build, encoder fixtures, filtered scoring statistics, point manifests and publication fault tests |
| **G3 Code** | Subject resolution, structure, callers/tests/docs and cross-repository comparison on the control corpus |
| **G4 ELIOT edge** | Binding, grants, compact cards, handle expansion, evidence pinning and verification through existing ELIOT surfaces |
| **G5 Product acceptance** | Beats or materially complements baselines without stale or access leakage, within resource budgets |
| **G6 Optional depth** | Dense, rerank and document providers admitted individually, each on measured benefit |

No optional semantic or document work begins before G5 is decided.

## Relationship to ELIOT

ELIOT compiles natural language, task context and WorkScope into a typed request and a scoped read
grant. Search plans and executes retrieval, then returns candidates with coverage, freshness, provider
assurance, source handles and reason codes. ELIOT performs admission, verification and interpretation.

A capability descriptor tells ELIOT which recipes and profiles are actually available, how fresh the
observation is, and which reason codes are currently degraded. Provider availability is planning
information, never permission.

## Continuous integration

CI is disabled for this repository. It is enabled only when the contract freeze provides something
meaningful to verify: format, workspace check, contract tests and dependency policy. Until then a
green pipeline would prove nothing and would invite treating scaffolding as progress.

## License

MIT. See [LICENSE](LICENSE).

The repository is private while the contracts are unstable; the license is MIT so that the
boundary contracts can be shared or open-sourced without a later relicensing step.
