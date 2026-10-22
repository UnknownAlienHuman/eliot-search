# Audit evidence

Current audit entrypoints:

1. [Analysis registry](ANALYSIS_REGISTRY.md) — living checked/partial/pending ledger with exact paths, findings, remaining work and confidence. Update it in every audit/cleanup pass.
2. [Static audit closure and rewrite map](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) — stable rewrite conclusions, errors found in the replacement and remaining runtime evidence.
3. [Distilled architecture/code audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) — source findings and concrete counterexamples.
4. [Workspace member disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) — receiving owner or deletion decision for every Cargo member; not a target package count.
5. [Donor evidence and limits](DONOR_VERIFICATION_REGISTER_2026-10-09.md) — inspected donor behavior, hidden costs and unverified assumptions.

Project direction comes from [`docs/architecture/README.md`](../architecture/README.md) and [`docs/implementation/README.md`](../implementation/README.md). Audit records explain why and what remains; they do not authorize code, freeze APIs, schedule agents or add qualification gates.

The old master audits, launch gates, wave packets, acceptance matrices and generated status records were removed from the current tree. Git history preserves them at their exact commits. Cite a historical fact only with its source/base and recheck it against the current owner before using it.

The static audit is sufficient input for the current-format rewrite, but the registry must continue recording newly discovered contradictions and consumer closures. It does not claim line-by-line verification, successful compilation, Windows durability, live-Qdrant behavior, relevance, scale or product readiness. Those claims require the implemented path and actual execution.
