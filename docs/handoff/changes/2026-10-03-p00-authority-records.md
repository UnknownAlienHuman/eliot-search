# P00 authority records — contract challenge

## Identity

- Requesting owner: integration owner
- Contract-owning package: `search-contracts`
- Base commit: `bac90ff7753ef7d4d107836c60480df216acfe7f`
- Architecture sections examined: S19.1 `SearchReadGrant`, S32 `Client integration contract`, S33 `Standalone contract`
- Status: `CONTRACT_CHALLENGE` / `PROPOSED_REVIEW_PENDING`
- Claimability: nonclaimable proposal only
- Authority: derivative P00 proposal only; no canonical contract, accepted API, assignment, or launch-state change

## Blocking problem

The P00 type registry defines `ServerRecord` visibility and ownership but no closed authority-record schemas. The current public `search_contracts::authority` module already exposes `ProviderBindingRecord`, `AuthoritativeGrantPolicy`, and `StandalonePolicyRecord`; W0's module packet lists thirteen modules and omits `authority`. The existing Rust declarations and their consumers establish current implementation shape only. They do not establish normative field meaning or authorize a writer to preserve every field.

S19.1 defines `SearchReadGrantClaims`, which is a provider-wire grant. Its fields include one `maximum_budget_class`, `issued_boot_id`, `issued_at`, `expires_at`, `nonce`, and `revocation_generation`. A durable policy is a different server record: it may hold a bounded set of allowed budget classes and a maximum TTL. The policy is not a grant, and those policy fields cannot be serialized as grant claims. The closed P00 wire shape already appears in `docs/contracts/p00/QUERY_AND_RESULTS.md`.

## Producer and consumers

- `search-contracts` owns shared record shapes and visibility only.
- `search-provider-protocol::binding` owns authenticated binding/session state and lifecycle.
- `search-access::grant` owns grant validation and scope intersection with server-authoritative state. Its current function contract establishes those operations only; it does not establish ownership of standalone policy creation or replacement. The mutable owner for `AuthoritativeGrantPolicy` / `StandalonePolicyRecord` is therefore unconfirmed in this proposal and requires an explicit owner/function-contract decision before canonical adoption.
- `search-control-redb::provider_authority` is a durability adapter. It may atomically persist and read the binding/policy pair; it does not own authorization semantics or mint grants.
- `eliot-searchd` composes the authenticated binding, authoritative policy, clock, nonce source, persistence adapter, and access operations. Construction or shape validation alone grants no access.

The current control adapter reads both rows from one published control generation, rejects half-present pairs, and treats the commit receipt as historical evidence. This proposal retains that separation of shape, mutable state owner, durability adapter, and daemon construction.

## Proposed closed P00 shapes

The machine-readable projection is [authority-records.toml](../../../swarm/schema-drafts/p00-authority-records/authority-records.toml). It labels each field as architecture-grounded or a proposed implementation semantic. Every shape, field, owner attribution, and behavior in this projection remains nonclaimable until the contract is accepted. It is not a replacement for the P00 pack.

`ProviderBindingRecord` is a private `ServerRecord`. Its closed proposal contains strong binding, installation, and incarnation IDs; a closed peer role; a bounded peer identity digest; a nonzero pairing generation; a bounded set of permitted client profiles; issuance and optional expiry timestamps; a nonzero revocation generation; and a closed lifecycle state. A profile allowlist filters binding-scoped capability exposure only; it does not grant a recipe, source read, exact scan, or retrieval. A binding expiry, peer-role enum, identity-digest algorithm/transcript, and profile-filter meaning are proposed decisions that need the provider-protocol owner’s review. A missing binding never means a default peer or permission.

`AuthoritativeGrantPolicy` is a private, nested policy value in `StandalonePolicyRecord`, not a provider-wire grant. It holds strong binding/install/incarnation identity and a binding-generation reference; a separate nonzero policy generation; the opaque principal, client scope, and scope-domain identifiers; bounded membership, corpus/portfolio, access-partition, modality, recipe-family, and allowed-budget sets; the existing singular optional portfolio revision; sensitivity and disclosure ceilings; source-read and exact-scan ceilings; and a positive maximum grant TTL. Sets use the exact accepted P00 `ContractBoundsV1` limits and reject duplicates. Empty required sets authorize nothing; they never mean “all.” The relationship between one or more allowed portfolio IDs and the singular optional revision, including cardinality, is unresolved; this proposal defines no per-ID revision mapping. Exact numeric limits come from the accepted W0 bounds digest, not from the current Rust constants.

The proposal omits `issued_boot_id` from the durable policy because S19.1 places it on each newly minted grant. It also makes the binding row the sole owner of `revocation_generation`; the grant copies the current value from that row. The policy-generation and binding-generation fields are private currentness fences, not extra wire claims. This is a proposed cleanup of the present Rust shape, not a statement that the existing fields already have these semantics.

The retained P00 scope fields include a bounded set of corpus-or-portfolio IDs and one optional `reference_portfolio_revision`. Neither S19.1 nor the P00 wire packet defines how that single revision maps when more than one portfolio ID is allowed. Policy-to-grant derivation for portfolio scope is blocked on that load-bearing mapping/cardinality decision. This proposal preserves the existing wire fields and does not invent a revision array, order-based pairing, or other architecture semantics.

`StandalonePolicyRecord` is the private durable lifecycle row containing exactly the nested policy, a closed `ACTIVE`/`REVOKED`/`EXPIRED` state, an issuance timestamp, and an optional expiry timestamp. Active policy must be unexpired and bound to the exact current binding generation. Missing, malformed, half-present, revoked, or expired state denies grant issuance. Terminal rows do not reactivate in place; a new authorization requires a new explicit owner action and generation. The canonical state spellings and terminal transition rule are proposed pending review.

The existing `disclosure_ceiling_ref` is intentionally excluded from the proposed closed binding shape. P00 does not define its referent, semantic owner, or relation to the grant’s concrete `disclosure_ceiling`. Keeping an untyped reference would leave a load-bearing field unresolved. If a separate binding ceiling is required, the contract owner must name its exact owner and typed ceiling semantics; until then the grant’s concrete ceiling is governed by the authoritative policy and request intersection.

## Lifecycle, generations, currentness, and atomic read

1. Pairing creates an active binding only after the provider-protocol owner has authenticated the peer. Pairing generation identifies the exact pairing; revocation generation advances on restrictive revocation. Neither counter is zero, wraps, or reuses a retired value. A changed binding identity requires a new binding ID.
2. If an accepted owner/function contract assigns policy mutation, creation or replacement must be an explicit local authorization action. Its policy generation strictly advances. The stored policy’s binding ID, installation ID/incarnation, and binding generation must exactly match the paired binding. Invalid transitions and overflow fail closed. This proposal does not assign the mutation owner.
3. Binding and policy are committed together under one conditional mutation and read from one current published generation. Both absent means unregistered and grants nothing. Exactly one present, mismatched IDs/generations, invalid lifecycle, decode failure, or stale publication means no grant and a typed failure. No read silently repairs or rebases state.
4. A possible write followed by cancellation/timeout is `OUTCOME_UNKNOWN`; recovery uses the original mutation identity and exact readback. A historical commit receipt alone never proves currentness.
5. Each grant issuance rechecks the current authenticated binding and atomic policy pair. Access revocation/purge barriers override snapshots immediately. `search-access` must apply access/currentness before retrieval, IDF, facets, counts, and traces, then repeat the required checks before source readback, emission, and handle/continuation expansion. These authority rows do not define a Qdrant collection generation or replace query snapshot fences.

## Policy-to-grant derivation (partial and blocked)

This proposal does not close a complete policy-to-grant derivation. The constraints below are partial and nonclaimable. The singular portfolio revision mapping/cardinality decision is load-bearing and remains unresolved, so the proposal does not define a deterministic rule for deriving `SearchReadGrantClaims` from a record pair. It infers no revision array, order-based association, or architecture semantics.

If a future accepted owner/function contract, portfolio mapping decision, and typed codec establish issuance, daemon composition could read a current binding/policy pair and pass bounded client requests through the authenticated protocol and grant-validation/access path. For scope dimensions whose policy relationship is already explicit, the proposal suggests intersecting requested memberships, corpus scope, access partitions, modalities, and recipe families with policy ceilings. Portfolio scope remains blocked. Empty intersections would fail closed. A request could select exactly one budget class present in the policy's allowed set; a set of policy classes must never be copied into the wire claim. Requested sensitivity, disclosure, source-read, and exact-scan values could only narrow policy ceilings. Exact scan without source-read permission would be invalid. These are proposed constraints, not a resolved derivation contract.

Under a separately accepted issuance rule, the issuer would create a fresh `GrantId`, nonce, current `issued_boot_id`, and timestamps for each grant. The proposal suggests that `expires_at` be no later than the request TTL, policy maximum TTL, active binding expiry, or active policy expiry, and that `revocation_generation` be copied from the same current binding row. No client-provided identity, generation, principal, binding, installation, boot ID, grant ID, nonce, or wider budget/scope should become authoritative. `search-provider-protocol::binding` authenticates the peer; the cited `search-access::grant` contract validates grants and intersects scope. Neither fact assigns standalone policy creation or replacement, and neither record construction nor control-store readback mints a grant.

## Visibility and codec boundary

All three shapes are `ServerRecord`, not legal `ProviderEnvelope` results. Their IDs, digests, and opaque references have the exact P00 strong/bounded wrappers; raw credentials, plaintext keys, vendor/native types, vendor identifiers, collection names, point IDs, and source content are forbidden. Unknown fields, unknown enum tags, duplicate set members, invalid bounds, and invalid cross-record references fail closed.

This request does not define or bless the durable typed codec. The current generic `CanonicalValue` JSON/CBOR boundary remains a separate blocker: an accepted contract still needs exact typed mappings, canonical encodings, version behavior, size/depth limits, and strict unknown-field rejection. The existing private control-store codec is adapter evidence, not a substitute for that P00 contract.

## Minimal canonical changes after independent review

If accepted, add these exact shapes and visibility rules to `docs/contracts/p00/TYPE_REGISTRY.md`, which is already included by the P00 manifest. Add `authority` to the `search-contracts` module list in `swarm/modules/w0.toml` and raise the package module count from 13 to 14 and the W0 total from 38 to 39; retain `lib` as the only public entry. The W0 shared read set already includes the P00 manifest, so no `swarm/stages.toml` or `swarm/stage-readsets.toml` read-set change is needed. Keep the existing P00 grant-claims shape in `QUERY_AND_RESULTS.md` separate from server records. The integration owner must also add coverage-crosswalk schema/type-ownership rows for `ProviderBindingRecord`, `AuthoritativeGrantPolicy`, and `StandalonePolicyRecord` in the appropriate `swarm/coverage/**` registries; the exact rows and state-owner attribution must follow the accepted owner decision. No current coverage row or proposal text authorizes those additions before acceptance. No change is proposed to Cargo, package/function registries, launch state, ticket/context/lease records, or accepted handoffs.

## Unresolved load-bearing decisions

- W0 must publish exact `ContractBoundsV1` values/digest for all set, identifier, opaque-reference, and total authority-record byte limits. The current Rust constants are not accepted numeric limits.
- Each grant is bounded by the explicit positive-u64 maximum_ttl_ms in its policy. P00 states no global duration ceiling; review must decide whether to add one and assign its exact value and owner. Checked timestamp overflow denies issuance.
- The provider-protocol owner must close peer-role variants and the canonical peer-identity digest input/domain; the digest is not itself proof of authentication.
- The mutable owner and operation contract for standalone policy creation/replacement are unconfirmed. The cited `search-access` grant contract covers validation and scope intersection only; an accepted function/ownership contract must assign policy mutation without deriving it from the current Rust structs.
- The one `reference_portfolio_revision` field has no defined association with a set that may contain multiple portfolio IDs. This mapping/cardinality decision blocks portfolio policy-to-grant derivation; preserve the current wire fields until the owner decision is recorded.
- The meaning and owner of current `disclosure_ceiling_ref` must be resolved as stated above; it is excluded from this proposal.
- Independent review must accept or reject the proposed profile-filter semantics, optional binding/policy expiry, terminal spelling/transitions, and movement of boot/revocation fields to grant issuance.
- A separate typed durable-codec contract is still required. This proposal must not be treated as resolving it.

## Impact

- Security/access: restrictive; shape construction does not authorize; absent, stale, empty, expired, or terminal rows deny issuance.
- Currentness/epoch/publication: exact atomic pair and published generation required; access and query fences remain separate.
- Source identity/readback: no source identity or source bytes enter these records.
- Residency/retention/purge: these records do not own content residency or purge state; live restrictive access/purge fences still win.
- Protocol/wire compatibility: no provider-wire change; the existing S19.1 grant claims stay separate.
- Migration/backward compatibility: no accepted API exists yet. Any persisted experimental rows require an explicit versioned migration/revalidation plan before implementation.
- Resource budget: all collections and strings are bounded by the accepted P00 bounds digest; numeric TTL/record-size limits remain blocking decisions.

## Required evidence after acceptance

Contract/property/fault fixtures must cover closed fields and tags, maximum and duplicate bounds, no-default behavior, exact binding/policy identity and generation matches, every absent/half-present/terminal/expired pair, non-reusable generations, atomic commit/readback, stale publication, unknown-outcome recovery, request-scope never-widens properties, single selected budget class, grant TTL minima, current revocation before retrieval and every access checkpoint, and proof that ServerRecord values cannot be emitted as provider results. No such tests or acceptance evidence were produced in this design pass.

## Decision needed

Classify as a compatible P00 schema completion only after the open owner and portfolio-revision decisions are closed and independent review confirms that accepted fields refine S19.1/S32/S33 without changing their behavior. Any accepted new peer identity, disclosure, profile, TTL, or portfolio-revision authority rule needs an explicit owner decision; an architecture change is required if that rule changes Part I. Until then, portfolio policy-to-grant derivation and canonical adoption remain blocked. Current status remains `PROPOSED_REVIEW_PENDING`.

## Source and validation evidence

Exact base-file SHA-256 values and bounded source scopes are in [authority-records.toml](../../../swarm/schema-drafts/p00-authority-records/authority-records.toml). Each current source hash is SHA-256 over the exact raw Git blob bytes at `base_commit`, paired with its full tagged Git object ID. The earlier 27 hashes are retained as `previous_worktree_sha256`; they describe checked-out CRLF worktree bytes at original hash capture. The source commit for that historical capture was not recorded, and those values are not Git-blob hashes. The P00 manifest's `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c` is the bounded Architecture 8.4 Part I semantic digest. The full master's raw Git blob SHA-256 at the base commit is `238b99fcb0c2d058cc6dc7583ab2c68549ad50d0da743df940803e7970aaffa8`, object ID `sha1:a28ed7d8e20fb056fc875a54a8f1b8484a6cbde9`. These values use different bases; there is no full-master-versus-Part-I digest discrepancy. Architecture content was read only in S19.1, S32, and S33; the full master was not loaded.

Historical worktree-byte Taplo capture: `C:\Users\kleym\.cargo\bin\taplo.exe --version` returned `taplo 0.10.0` with exit 0. One parse ran from `C:\Users\kleym\.codex\worktrees\p00-authority-record-design\eliot-search` with command `C:\Users\kleym\.cargo\bin\taplo.exe check C:\Users\kleym\.codex\worktrees\p00-authority-record-design\eliot-search\swarm\schema-drafts\p00-authority-records\authority-records.toml`. It parsed the checked-out CRLF worktree file, not the exact Git blob bytes at the base commit. Its input was 19,145 bytes with SHA-256 `A483FD49477CB2EC7004B7787264535D3EDE45DE06A9B51F9C9393ED0263223B`; the retained byte-for-byte copy has the same hash. Exit code was 0. Captured check stdout and stderr are each 0 bytes (SHA-256 `E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855`). This historical capture does not validate Git-blob input. The raw output files, runtime version output, input copy, and manifest are retained under `C:\Users\kleym\AppData\Local\Temp\eliot-search-p00-authority-records-eaad44b8a132421f84799362d5442eda` (`capture-manifest.json`). No repository tests, build, or package acceptance checks were run.
