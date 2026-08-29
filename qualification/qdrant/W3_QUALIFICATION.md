# W3 Qdrant and lexical qualification contract

**Status:** `UNQUALIFIED`  
**Architecture:** ELIOT Search 8.4, S9–S14, H8–H12, P05–P07  
**Scope:** exact local Qdrant artifact, Rust client, process containment, collection schema, sparse lexical
compatibility, mutation/readback semantics, publication and reclaim prerequisites.

No version string, upstream release tag, documentation claim or successful health request is an
acceptance receipt. Indexed mode remains disabled until every mandatory item below is reproduced
against one exact Windows x64 server artifact and one exact Rust client revision.

## Owners

| Evidence | Owner |
|---|---|
| executable path/version/SHA-256, process identity, loopback/ACL/Job Object | `search-qdrant-supervisor` |
| collection capabilities, payload indexes, strict mode, mutations, readback and queries | `search-qdrant-bridge` |
| document/query sparse vectors and profile fixtures | `search-lexical` |
| canonical point identity and collision refusal | `search-point-identity` |
| exact point/payload/vector manifest | `search-projection-planner` |
| publication kill/reopen matrix | `search-publication` |
| route/epoch pin and watermark | `search-epoch-pins` |
| exact ordinary retired-point deletion | `search-index-reclaimer` |
| final P05–P07 evidence acceptance | integration owner + independent reviewer |

One package may produce its own evidence but cannot accept its own handoff.

## Frozen inputs required before execution

Populate [`artifact.toml`](artifact.toml) with exact, non-empty values:

- server semantic/build version;
- server executable SHA-256 and byte length;
- source/release identity and license receipt;
- Windows architecture and required runtime identity;
- Rust client crate version, source checksum and lockfile identity;
- qualified working directory/config template identities;
- OS-secret lease mechanism and process-injection receipt;
- disposable qualification data-root identity.

`latest`, ranges, floating Git revisions, auto-download and auto-upgrade are forbidden.

## Qualification order

1. Verify artifact bytes before execution.
2. Acquire one isolated data-root owner and a purpose/incarnation-bound API-key lease.
3. Start the exact executable under loopback-only binding, ACL and Job Object containment.
4. Verify PID, executable path, executable digest, installation incarnation and data-root identity.
5. Create a disposable one-shard collection from [`collection-schema.toml`](collection-schema.toml).
6. Create every mandatory payload index **before** point ingestion.
7. Enable strict mode and prove unindexed retrieve/update filters are rejected.
8. Execute every mandatory probe in [`probes.toml`](probes.toml).
9. Reproduce lexical document/query golden vectors and collision corpus against one provider path.
10. Execute exact point mutation/readback/delete and response-shape fixtures.
11. Execute publication crash/failpoint recovery against fake ports first, then the qualified bridge.
12. Execute route/epoch pin and ordinary exact reclaim fixtures.
13. Publish immutable raw outputs and a reviewer receipt. Only then set artifact/probe status to
    `QUALIFIED`.

Failure or `UNAVAILABLE` keeps indexed mode disabled. DIRECT and exact/source paths may remain available
with explicit degraded capability state.

## Mandatory admission properties

### Process and secret boundary

- loopback only;
- API authentication required;
- no plaintext secret in argv, repository/config files, logs, metrics, crash-report metadata or receipts;
- exact executable identity is rechecked after start and during health/recovery;
- child terminates with the daemon/Job Object;
- restart is bounded and ends in quarantine;
- orphan/PID reuse/executable replacement never causes blind attachment.

### Collection and filter boundary

- one shard, one active generation plus at most one migration candidate;
- one `ProjectionMembership` per point;
- exact named vector/profile set;
- explicit payload index for every base-eligibility predicate;
- strict mode rejects unindexed retrieval/update filters;
- signed `i64` epoch range behavior matches the fixture;
- missing `valid_until_epoch_exclusive` passes the open-ended `must_not <= E` condition;
- retrieval and IDF population use the same canonical base eligibility plan;
- inaccessible, staged, retired, denied, purged or shadowed points do not affect IDF;
- every schema/profile/topology incompatibility creates a new collection generation.

### Mutation and readback boundary

- correctness-path mutations request synchronous acknowledgement and strongest qualified ordering;
- timeout/cancellation after possible mutation yields `OUTCOME_UNKNOWN`, never a fabricated failure;
- unknown outcomes are resolved by exact ID readback;
- upsert/close/delete uses exact point IDs, not broad payload filters;
- UUID projection collisions compare full 256-bit identity and never overwrite;
- exact counts and readback sets must match manifests before publication commit.

### Lexical boundary

- exactly one accepted provider path per collection generation;
- document/query token IDs and weighting are fixture-compatible;
- no implicit language, stopword, stemming, normalization or hashing defaults;
- profile identity covers provider artifact, tokenizer, Unicode rules, identifier expansion, mapping,
  collision policy, weighting, sparse modifier/schema and fixture digest;
- lexical matches nominate candidates only; exact identity/completeness/absence remain source/exact-plane
  claims.

## Evidence products

Each run publishes immutable references for:

```text
artifact_identity
process_containment
secret_side_channel_scan
collection_schema
strict_mode
signed_epoch_and_missing_upper_bound
sparse_idf_and_independent_idf_population
payload_index_completeness
lexical_document_query_golden
point_collision_nonoverwrite
wait_ordering_mutation_readback
publication_failpoint_matrix
route_epoch_pin_watermark
exact_reclaim_resume
direct_mode_degradation
```

Every record contains exact command/fixture, commit, server/client identities, configuration fingerprint,
platform, start/end time, `PASS | FAIL | UNAVAILABLE`, raw output digest and reviewer. Prose-only evidence
is rejected.

## Current disposition

```text
server artifact: UNSELECTED
server digest: UNSET
Rust client: UNSELECTED
collection schema: DESIGNED_NOT_EXECUTED
capability probes: NOT_EXECUTED
lexical profile: UNQUALIFIED
Windows containment: NOT_EXECUTED
indexed mode: DISABLED
```
