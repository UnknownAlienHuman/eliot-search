# Integration owner contract

The integration owner is a repository role, not a capability writer. Root access does not authorize
implementation of package behavior.

## Exclusive write scope

Only this role may change root Cargo/dependency/features, toolchain, lockfile, CI, architecture,
`docs/contracts/`, generated schemas, handoff docs, `swarm/`, shared fixture ownership and launch state.

## P00 responsibilities

1. Verify the embedded Architecture 8.4 hash and hash every P00 contract-pack file.
2. Resolve all contract challenges, including Part I precedence decisions.
3. Review and accept `search-contracts`; publish its API/schema digest.
4. Launch `search-domain` and `search-ports` only from that immutable accepted commit.
5. Accept their pure-domain and port API/conformance digests.
6. Select/pin a stable Windows-compatible Rust/dependency set, generate `Cargo.lock`, review licenses
   and execute the real P00 policy/test suite.
7. Verify Cargo members, registry, assignments, package directories and dependency sets are identical.
8. Publish a W0 receipt before advancing launch state.

The scaffold does not invent a green toolchain, lockfile or external qualification result.

## Package merge protocol

1. create one immutable-base worktree and writer branch;
2. provide only the bounded read set and accepted dependency digests;
3. reject writes outside package scope;
4. review ownership, port conformance, failure mappings and raw evidence;
5. merge in topological order;
6. record exact accepted commit and API/port digest;
7. run workspace/generated-schema/dependency checks;
8. publish a wave receipt before activating dependents.

## Contract and port changes

A consumer cannot patch around an absent field, reason or port. Route the request to
`search-contracts`, `search-domain` or `search-ports` as appropriate, record compatibility impact,
merge the producer first and restart the blocked consumer from the new immutable handoff.

## Composition

`eliot-searchd` is progressive. A W1 writer sees only contracts/domain/ports and shell dependencies.
Later Cargo features are enabled only after their package handoffs are accepted.

## Prohibited behavior

- business logic in root scripts or daemon wiring to avoid an owner;
- advancing from compilation alone;
- fabricated Windows/Qdrant/redb/provider evidence;
- optional depth before its gate;
- two concurrent writers for one package;
- silent architecture changes through derivative docs.
