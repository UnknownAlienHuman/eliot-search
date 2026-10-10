# search-config

**W1 support package — deterministic local configuration layering and reload planning.**

**Status:** W1 deterministic mechanics implemented and locally qualified; runtime acquisition and package-specific application remain external.

This package owns bounded TOML configuration documents, deterministic layer precedence,
section registration, validation dispatch, redaction, effective-config fingerprints and
reconfiguration plans. Capability packages own their typed section schemas and runtime state.

The #386 source slice removes copied SHA-256 and uses the shared exact-byte RustCrypto owner.
Legacy v1 preimages and bounded path digests keep their bytes; hashing and preimage growth have
a hard 8 MiB ceiling. Oversized diagnostic paths yield size metadata without a digest or text.
Focused compatibility fixtures are AUTHORED_NOT_RUN. The typed TOML cutover, internal
section-digest ownership, canonical v2 identity and qualification remain open under #238/#109/#215.

File/env/CLI documents now share one private builder and checked aggregate budget. It enforces
sections, per-section fields, individual identifiers/text/lists and totals before config-owned
allocation/growth. W1 aggregate ceilings are 1 MiB identifier bytes, 4 MiB decoded value bytes and
8,192 list items. The existing public constructors/header rules stay intact; their accepted
in-budget v1 values and persisted digests keep their bytes. Previously unchecked oversized captured
values or aggregates now reject. Adversarial boundary fixtures are AUTHORED_NOT_RUN; donor
materialization and the explicit TOML/v2 migration remain #389/#238 work.

Captured document Debug now reports fixed primitive metadata: schema, profile presence/byte
extent, source kind and entry/section counts. It omits unvalidated profile and payload text,
including when embedded in `ConfigLayers`, while preserving typed accessors and persisted bytes.
Disclosure fixtures are AUTHORED_NOT_RUN under #238/#215.

Redacted views now enforce a 1 MiB W1 logical diagnostic-byte ceiling as well as their entry ceiling.
They charge the profile/header, names and visible values before cloning or hashing, omit over-budget
entries whole, and return a content-free error if the header cannot fit or limits are invalid. This
logical payload accounting does not measure allocator residency or serialized JSON/Debug bytes.
The daemon status caller handles the fallible view without changing runtime readiness or persisted
configuration identities. Diagnostic boundary fixtures are AUTHORED_NOT_RUN under #238/#109/#215;
the maintained parser and explicit tagged v2 fingerprint cutover remain #389/#238 work.

## Owns

- config source/layer identity and deterministic merge semantics
- top-level section registry and unknown-section rejection
- fixed/security-floor/override classification
- effective snapshot fingerprint and redacted diagnostic projection
- change classification: live, security barrier, restart, new generation or reject

## Must not own

- reading files/environment/process arguments
- package-specific runtime state or vendor configuration objects
- plaintext secrets or permissive fallback on unknown settings
- silently applying restart/rebuild/security changes as live updates

- **Delivery wave:** W1 / P01
- **Soft source-line target:** 5,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
