# Query selection — direct handlers

The unused planner Rust package, manifest and exclusive fixtures are removed.
The real search handler calls the concrete Backend and pinned evidence reader directly.

New Search captures one published view and selected sources. Exact is added only with a retained-source
scanner. Continue/Expand use their existing session view. Unsupported operations fail before effects.
Pass the typed request and its already owned context directly; no recipe, capability catalogue, grant,
plan fingerprint or per-source leg protocol.

[Source selection](../search-access/FUNCTIONS.md) owns candidate versus IDF population.
Preserve backend order and the original deadline, cancellation and finite limits.
