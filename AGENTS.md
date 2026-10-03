# ELIOT Search repository instructions

## Product identity

ELIOT Search is a **standalone Rust search product** and an optional typed provider for the wider ELIOT
ecosystem. The implementation target is Architecture 8.4: local source preparation and retrieval around
Qdrant, with redb for technical control state and scoped immutable source/preparation storage.

The supported product must start and operate without ELIOT Memory OS, a Governor, a swarm controller,
agent roles, assignment tickets or writer leases.

### Owned here

- source admission, identity, revision retention and preparation;
- lexical/projection construction;
- Qdrant lifecycle, schema, publication and retrieval;
- access-scoped query planning and candidate validation;
- result projection, handles, currentness, rebuild, retention and recovery;
- standalone daemon/CLI and optional typed provider adapters.

### Not owned here

- Tasks, WorkScopes, Attempts or canonical task history;
- General Manager/Governor state, agent scheduling or native harness lifecycle;
- mailboxes, steer/goal routing, model loops or subagent observation;
- generic assignment-ticket issuance, writer leases, approval/signature profiles;
- review acceptance, finish authority or cross-project orchestration.

Those responsibilities belong to `eliot-swarm-controller` and, after integration, `eliot-memory-os`.
Search may be called by those systems; it must not embed or reimplement them.

## Authority order

Resolve conflicts in this order:

1. `docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`;
2. accepted product ADRs, especially ADR 0005;
3. accepted public contracts, provider schemas and product qualification contracts;
4. nearest package `AGENTS.md`, `FUNCTIONS.md` and package-owned documentation;
5. the current maintainer request, issue/PR acceptance criteria and exact dependency contracts;
6. Cargo manifests and compiled source reality;
7. planning/history material under `swarm/**`, `docs/handoff/**` and `docs/execution/**`.

`swarm/launch-state.toml`, ticket drafts, context manifests, leases, submissions, reviews and handoffs are
**advisory or historical repository metadata only**. They are not product inputs. Their presence, absence
or status cannot authorize, block or widen implementation. Any lower-precedence text saying otherwise is
superseded by ADR 0005.

A maintainer request, issue or PR plus one non-overlapping branch/worktree is sufficient authorization to
work. Do not build a ticket issuer, lease service, PKI, role database or other controller inside Search.

## Required working method

1. Read the architecture section, root instructions, nearest package instructions and named issue/PR
   before editing.
2. Use one branch/worktree per manager and one active writer per overlapping package scope.
3. Fix the product code first. Do not replace missing implementation with planning documents, registries,
   schemas or evidence prose.
4. Keep changes within the actual owning package. Cross-package composition belongs in the daemon or a
   separately reviewed integration branch.
5. Run the smallest meaningful compilation gate after each implementation slice:

   ```text
   cargo +1.98.0 check --locked -p <package> --all-features
   cargo +1.98.0 clippy --locked -p <package> --all-features -- -D warnings
   ```

   Use the applicable target flags when the package is library-only. Full product/process tests are
   deferred until the implementation is complete unless a change requires an immediate focused proof.
6. Never claim execution that was not performed at the exact revision.
7. Keep GitHub Actions manual-only. Do not add push/pull-request triggers merely to obtain a build.
8. Do not commit generated local paths, credentials, API keys, source contents or unredacted logs.

## Product invariants

- Qdrant is the only indexed/search database. No SQLite/FTS, Tantivy, Lucene, local postings database or
  alternate production search engine.
- redb stores bounded technical control state, never a second searchable corpus.
- Qdrant payload is projection metadata, never source evidence. Candidates require authoritative exact
  source/revision readback and validation.
- Retrieval, filtered IDF population and exact denominator use one eligibility contract. Denied data must
  not influence permitted ranking/statistics.
- Paths are locators, not identities. File/native identity, source identity, revision identity,
  representation identity and projection identity remain distinct.
- Immutable revisions and manifests are never rewritten in place. Publication is generation/epoch safe.
- Unknown external mutation outcome is not success and is not blindly replayed.
- Restrictive access/purge/shadow changes fail closed across retrieval, handles and continuations.
- The standalone daemon/CLI and ELIOT adapter share one underlying state and query authority. No adapter
  may create a second implementation path.
- Optional model/document workers remain disabled until independently qualified and are not required for
  the standalone baseline.
- Vendor Qdrant types stay inside `search-qdrant-bridge`; public package boundaries use Eliot-owned types.
- Search never executes indexed source code or delegates query interpretation to an LLM.

## Standalone and ELIOT integration

Standalone operation is the baseline, not a fallback. The daemon owns its installation/root/process and
serves the public local client contract without any external controller.

ELIOT integration is a leaf translation layer over accepted provider protocols. Search returns bounded
candidate/result records, coverage, freshness, assurance, reason codes and opaque handles. It does not
receive canonical ELIOT credentials, task authority, Context Compiler admission or finish authority.

No Search configuration option may make ELIOT Memory OS or `eliot-swarm-controller` mandatory for normal
startup, ingestion, indexing, search, rebuild or recovery.

## Repository metadata boundary

Static package maps and ownership crosswalks may help humans avoid overlapping edits. Keep them small and
non-executable. Do not extend the active tree with generic:

- assignment/lease event state machines;
- approval/signature/profile registries;
- task/attempt/mailbox/scheduler records;
- orchestrator credentials or actor roles;
- controller qualification suites;
- large captures proving only the repository-control tooling itself.

Reusable controller work belongs in `UnknownAlienHuman/eliot-swarm-controller`. Canonical ELIOT task and
memory authority belongs in `UnknownAlienHuman/eliot-memory-os`.

## Evidence and acceptance

Compilation is necessary but not product qualification. Product claims require the exact named evidence:

- real Qdrant artifact/client/schema/query/restart probes for indexed capability;
- native platform evidence for Windows identity, containment, secrets and named pipes;
- real source → durable state → Qdrant → validated result for the product spine;
- restart/recovery, deny, unknown-outcome and rebuild cases;
- installed baseline evidence before release.

Planning records, ticket status, source presence and mock/oracle success cannot substitute for those gates.

## GitHub connector use

Before concluding GitHub is read-only, load the full unfiltered GitHub tool catalog, confirm
`permissions.push == true`, and use GitHub API write actions. A harmless unattached blob may be used as a
write probe. Do not infer capabilities from a filtered tool list or from missing local Git credentials.
