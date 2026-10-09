# Agent contract — search-contracts

## Current execution authority

Ordinary package work owns only `crates/search-contracts/`. The sole reviewed Wave-1 exception is [issue #237](https://github.com/UnknownAlienHuman/eliot-search/issues/237), executed through the [single-manager packet](../../docs/audit/WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md).

For #237 only, the one manager may also edit:

```text
narrow canonical-digest guard modules and command glue in xtask
root exact crypto dependency pins where required
Cargo.lock integration
focused #237 documentation/fixtures
```

This exception does not authorize semantic migrations in query, source, Qdrant, retention, evaluation, daemon or worker packages. Those remain separate owners.

Before editing, read:

1. [root `AGENTS.md`](../../AGENTS.md);
2. [architecture entrypoint](../../docs/architecture/README.md);
3. [normative Architecture Part I](../../docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md);
4. [ADR-0005](../../docs/adr/0005-standalone-search-product-and-controller-boundary.md);
5. [ADR-0006](../../docs/adr/0006-agent-analysis-framework-product-scope.md);
6. [single-manager packet](../../docs/audit/WAVE1_SINGLE_MANAGER_PACKET_2026-10-09.md);
7. [donor verification register](../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md);
8. #237 and every audit-amendment comment.

When a missing contract blocks correct work outside this exact task, report the exact field, invariant, producer, consumer and compatibility impact to the owning issue. Do not patch around it or copy another owner’s types.

## Mission

Define the complete bounded, vendor-neutral wire and domain contract surface used by every ELIOT Search package. `search-contracts` is also the sole production owner of the closed canonical value vocabulary and canonical JSON/CBOR byte encoding.

Traceability: S3, S7, S10, S19, S20, S23-S26, S30.3, S32, S34, H3-H4, P00.

## Ownership

- newtypes and identifiers;
- recipes and reason codes;
- source/view/membership/residency schemas;
- grants, plans, budgets and candidate/result schemas;
- anchors, handles, protocol envelopes and capability descriptors;
- closed bounded `CanonicalValue` vocabulary;
- deterministic canonical JSON and the accepted RFC 8949 CBOR profile;
- algorithm-qualified digest construction boundary accepted by #237;
- exact decode/restore versus compute semantics for shared digest types.

## Forbidden ownership

- runtime state or I/O;
- filesystem, network, process, secret-store, redb, Qdrant or provider effects;
- Windows or client/vendor public types;
- implicit string/UUID substitution at domain boundaries;
- caller-created authority receipts, completeness or freshness;
- a second canonical codec/value tree or generic dynamic Serde authority;
- package-local semantic migrations owned elsewhere;
- silently ignored security, scope or budget fields.

## Dependency policy

The default remains no external dependency unless an explicit boundary review accepts a smaller exact donor.

The reviewed #237 exception permits only the exact private algorithm dependencies selected in the [donor register](../../docs/audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md):

```toml
blake3 = { version = "=1.8.2", default-features = false, features = ["std", "pure", "zeroize"] }
sha2   = { version = "=0.10.9", default-features = false }
```

The manager must recheck exact source, registry checksum, license, MSRV, transitive/build closure, features and advisories before changing manifests or `Cargo.lock`. The upstream `blake3::pure` feature is documented as unstable/testing-oriented; exact pinning and explicit review are mandatory. If its accepted Rust/target build fails, stop and review the alternative rather than widening features silently.

Forbidden here:

```text
ciborium as production codec
serde/dynamic JSON/CBOR product authority
rayon, mmap or runtime file hashing helpers
BLAKE3 traits-preview/digest features
SHA-2 asm/OID/compress features
another crypto or canonicalization stack
```

Donor types remain private. Public APIs expose only ELIOT-owned types.

## Canonical profile invariant

The current CBOR encoder orders map keys by encoded-key length and then bytewise value. This is the RFC 8949 length-first deterministic ordering profile (§4.2.3). Preserve its accepted bytes and golden vectors. Do not silently switch to Core Deterministic ordering (§4.2.1) or donor defaults under the same profile.

Vector-returning and streaming/hash paths must use one encoder implementation. Every output append/write is checked against its ceiling before growth or effect. An error yields no partial authority digest.

## Required logical surface

These are behavior contracts, not mandated Rust syntax. Preserve the semantics even if the concrete API is improved:

- `Epoch::new(i64) -> Result<Epoch, ContractError>`;
- `Epoch::checked_next(Epoch) -> Result<Epoch, ContractError>`;
- `RecipeId::parse_versioned(&str) -> Result<RecipeId, ContractError>`;
- `SourceView::validate() -> Result<(), ContractError>`;
- `SearchReadGrantClaims::validate_shape() -> Result<(), ContractError>`;
- `SearchTaskPlan::canonical_fingerprint_input() -> CanonicalBytes`;
- `ProviderEnvelope::validate_version_and_limits() -> Result<(), ContractError>`;
- validated canonical digest domains and finite limits;
- honest stored/wire decode constructors distinct from real compute helpers;
- explicit algorithm on content/artifact digests where algorithm identity is load-bearing.

Do not add a generic public operation receipt/digest that callers can mint from arbitrary values. An operation identity belongs to one closed owner schema/domain and retains the full 32-byte digest.

## Failure surface

Use typed errors/reason codes. Relevant public reasons include `EPOCH_OUT_OF_RANGE`, `EPOCH_EXHAUSTED`, `UNKNOWN_LOAD_BEARING_FIELD`, `CONTRACT_VERSION_MISMATCH` and the closed canonical/digest failures accepted by #237. Never turn a degraded, partial, decode-only or legacy state into apparent success.

## Focused seams and exit evidence

Retain or add focused fixtures for:

- exact eleven v1 recipes;
- forbidden epoch sentinels;
- unknown load-bearing fields;
- canonical JSON/CBOR round trips and rejected noncanonical bytes;
- unchanged length-first CBOR golden bytes;
- bounded sink behavior before output growth;
- real BLAKE3/SHA-256 standard vectors and domain separation;
- decode/restore versus compute classification;
- typed repository guard classifications and false-positive canaries;
- vendor-type/dependency boundary.

Write focused fixtures with the code. Per current project direction, broad/full test execution remains a later testing/qualification phase; `--all-targets` must still compile the focused targets now.

## Current minimum source gate

```text
cargo +1.98.0 check --locked -p search-contracts -p xtask --all-targets
cargo +1.98.0 clippy --locked -p search-contracts -p xtask --all-targets -- -D warnings
```

Report the exact commands and raw outcomes. Compilation/Clippy are source gates, not product qualification.

## Size and split guard

- Delivery wave: **W0 / P00**;
- soft `src/` target: **8,000 lines**;
- hard review threshold: **10,000 total hand-written Rust lines**;
- split only on a real security, runtime, replacement, test or dependency boundary;
- never create a forwarding wrapper, `search-canonical` crate or crate-per-type shell.

## Definition of done for #237

The existing canonical bytes remain frozen; one checked bounded encoder feeds returned bytes and real digest helpers; exact private algorithm dependencies are reviewed; decode/restore and compute are honest; algorithm-tagged public values cannot relabel arbitrary bytes within the accepted scope; the current-tree guard classifies every exception under one executable owner; package/guard check and strict Clippy pass; no owner package is migrated or product-qualified by assertion.