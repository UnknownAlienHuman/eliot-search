# Preparation family

**Organizational capability family — not a Cargo package.**

## Child packages

- [`search-materializer/`](search-materializer/) — C08: Convert an exact retained revision into a canonical representation with explicit coordinate and loss maps.
- [`search-unitizer/`](search-unitizer/) — C09: Turn a materialization into deterministic unit occurrences and an immutable unit manifest.
- [`search-code-enricher/`](search-code-enricher/) — C10: Produce provider-qualified Rust definitions, references, tests and documentation facts without claiming compiler truth.

## Family invariants

- Every transform has a versioned profile and explicit assurance ceiling.
- Coordinate and loss maps are mandatory when bytes are transformed.
- Tolerant syntax parsing is not compiler truth.

Each writer agent owns exactly one child package and follows that package's `AGENTS.md`.
