# Integration owner contract

The integration owner is a repository role, not a Cargo package writer. It may not implement capability
behavior merely because it can edit root files.

## Exclusive write scope

Only the integration owner may change:

- root workspace members, dependency versions, feature wiring and dependency policy;
- `rust-toolchain.toml`, `Cargo.lock`, CI and generated schema registries;
- `swarm/`, architecture/handoff documents and shared fixture ownership;
- cross-package compatibility patches and composition-wave activation;
- `swarm/launch-state.toml` and accepted handoff commit references.

## P00 responsibilities

1. Verify the embedded Architecture 8.4 SHA-256 and required sections.
2. Select and pin one stable Rust toolchain that passes the Windows/dependency qualification.
3. Review exact dependency sources/licenses; generate and commit `Cargo.lock`.
4. Accept the `search-contracts` public schema before allowing `search-domain` to consume it.
5. Run dependency-direction and public-vendor-type checks.
6. Publish the W0 receipt and advance the launch state only after all P00 evidence is real.

The scaffold deliberately does not invent a toolchain version or lockfile before this qualification.

## Wave merge protocol

For each package:

1. create an immutable-base worktree and one writer branch;
2. provide only the bounded assignment and accepted public dependency handoffs;
3. reject writes outside the package path;
4. review package evidence and contract requests;
5. merge in topological order;
6. record the exact accepted commit and public API digest;
7. run workspace/dependency/generated-schema checks;
8. publish a wave receipt before activating dependents.

## Contract changes

A package cannot patch around an absent field or port. The integration owner routes the request to the
contract owner, records compatibility/version impact, merges the accepted contract first, and restarts
the blocked consumer from an immutable dependency commit.

## Composition

`eliot-searchd` is progressive. The W1 writer reads and wires only the owner/journal/protocol shell.
Later feature layers are enabled only after their package handoffs are accepted. A final-manifest
dependency does not authorize a daemon writer to read or implement every future capability.

## Prohibited behavior

- implementing business logic in root scripts or the daemon to avoid a package contract;
- advancing a wave from compilation alone;
- fabricating unavailable Windows/Qdrant/redb/provider evidence;
- enabling optional model/document packages before their gate;
- merging two concurrent writers for one package;
- silently changing Architecture 8.4 through a handoff document.
