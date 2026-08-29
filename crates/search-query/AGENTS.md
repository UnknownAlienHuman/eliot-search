# Family rules

- Access and currentness are pre-candidate constraints, not final filters.
- Query plans are server-owned and vendor-neutral.
- Candidate validation reopens exact source revisions before projection.
- Results are bounded candidate products, never belief or admission decisions.

This directory is not a package. Do not add a family-level `Cargo.toml` or shared implementation. Put behavior in the child package that owns the failure state and test seam.
