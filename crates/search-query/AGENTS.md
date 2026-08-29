# Query family rules

- Access and currentness are pre-candidate constraints, not final filters.
- Query plans are server-owned and vendor-neutral; query packages never construct Qdrant/redb clients.
- Candidate validation reopens exact source revisions through a port before projection.
- `search-handles` owns source-handle state and every expansion authorization check.
- `search-result-projector` selects bounded handle subjects but stores no handle state.
- `search-continuation` owns continuation windows/checkpoints only, never general source handles.
- Results are bounded candidate products, never belief, admission or completion decisions.

This directory is not a package. Do not add a family-level `Cargo.toml` or shared implementation. Put
behavior in the child package that owns the failure state and test seam.
