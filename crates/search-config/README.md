# search-config

**W1 support package — deterministic local configuration layering and reload planning.**

**Status:** package boundary and implementation contract only; behavior is intentionally unimplemented.

This package owns parsing-independent configuration documents, deterministic layer precedence,
section registration, validation dispatch, redaction, effective-config fingerprints and
reconfiguration plans. Capability packages own their typed section schemas and runtime state.

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
