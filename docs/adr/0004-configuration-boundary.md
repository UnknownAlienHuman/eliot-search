# ADR 0004 — Give local configuration one deterministic layering owner

- **Status:** accepted
- **Date:** 2026-08-29
- **Scope:** implementation packaging and local settings semantics
- **Architecture:** ELIOT Search 8.4, especially S16, S27, S29-S30, S32-S33 and H2/H8/H16

## Context

The scaffold named `DaemonConfig` and several package-local configuration concepts but assigned no
owner for parsing-independent layering, override precedence, unknown-key behavior, redaction,
effective-config identity or reload classification. Leaving those concerns in `eliot-searchd` would
create a second policy engine in composition code and force every package agent to invent incompatible
settings behavior.

Configuration also crosses security and currentness boundaries: a setting can be live, restrictive,
restart-required, generation-changing, rebuild-requiring or prohibited. Treating all changes as a live
map update would violate process, access, publication and source-currentness invariants.

## Decision

Add `search-config`, a W1 pure support package depending only on `search-contracts`.

It owns configuration documents, deterministic layer precedence, section registration, security-floor
and override checks, effective snapshot fingerprints, redacted diagnostics and reconfiguration plans.
It performs no I/O and applies no runtime changes.

Each capability package remains the sole owner of its typed settings section, defaults, validation and
runtime application. The daemon collects file/environment/CLI inputs, calls `search-config`, dispatches
section inputs to owners, then executes the returned plan through accepted capability APIs.

The integration-owned `config/sections.toml` maps every top-level section to one owner and reload class.
Unknown sections and duplicate owners fail closed.

## Layer order

```text
compiled security-safe defaults
  < local UTF-8 TOML file
  < whitelisted ELIOT_SEARCH__SECTION__KEY environment overrides
  < explicit CLI overrides
```

Higher precedence does not imply permission: fixed values, security floors, arrays/maps and
secret-bearing fields can restrict allowed override sources. Plaintext secrets are forbidden in every
layer; only opaque OS-bound `SecretRef` values are accepted.

## Reload classes

```text
NOOP
APPLY_LIVE
SECURITY_BARRIER
RESTART_DEPENDENCY
DRAIN_AND_RESTART
NEW_COLLECTION_GENERATION
REBUILD_PROJECTION
GATE_REQUIRED
REJECT
```

The most restrictive required action dominates. `search-config` plans but never executes a change.

## Consequences

- configuration semantics are testable without filesystem, environment or daemon fixtures;
- package agents receive only their section and do not parse the full user document;
- secrets and fixed security floors cannot leak through generic maps;
- profile or schema changes cannot be mistaken for live updates;
- Cargo gains one real support package, not a forwarding facade;
- the Architecture 8.4 product authority and embedded hash are unchanged.
