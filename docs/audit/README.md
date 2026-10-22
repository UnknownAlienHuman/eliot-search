# Audit evidence

Current static refactor-audit entrypoints:

1. [Static audit closure and rewrite map](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) — checked surface, errors found in the replacement, rewrite rules and remaining runtime evidence.
2. [Distilled architecture/code audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) — source findings and concrete counterexamples.
3. [Workspace member disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) — receiving owner or deletion decision for every Cargo member; not a target package count.
4. [Donor evidence and limits](DONOR_VERIFICATION_REGISTER_2026-10-09.md) — inspected donor behavior, hidden costs and unverified assumptions.

Project direction comes from [`docs/architecture/README.md`](../architecture/README.md) and [`docs/implementation/README.md`](../implementation/README.md). Audit records explain why; they do not authorize code, freeze APIs, schedule agents or add qualification gates.

The old master audits, launch gates, wave packets, acceptance matrices and generated status records were removed from the current tree. Git history preserves them at their exact commits. Cite a historical fact only with its source/base and do not reuse the old implementation graph.

The static audit is sufficient input for the current-format rewrite. It does not claim line-by-line verification, successful compilation, Windows durability, live-Qdrant behavior, relevance, scale or product readiness. Those claims require the implemented path and actual execution.
