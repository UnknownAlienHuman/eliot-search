# search-code-enricher

**C10 — Code structural enrichment.**

**Status:** `SOURCE_PRESENT / LEGACY_PROFILE`; substantial custom Rust enrichment source exists, but the selected canonical structural path is #223 Tree-sitter → #224 SCIP, with optional #225/#236 profiles. Do not extend a parallel parser authority. See [central package status](../../../docs/product/PACKAGE_STATUS.toml).

Produce provider-qualified Rust definitions, references, tests and documentation facts without claiming compiler truth.

## Owns

- Rust structural profile
- definition/reference/test/doc role extraction
- configuration predicates
- provider assurance and parser identity
- structural relation manifest

## Must not own

- compiler-grade certainty from tolerant parsing
- running build scripts or language-server builds
- ranking or final normative comparison
- vendor parser types in public APIs
- opening source stores directly instead of consuming immutable contract inputs

- **Delivery wave:** W5 / P10
- **Soft source-line target:** 8,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
