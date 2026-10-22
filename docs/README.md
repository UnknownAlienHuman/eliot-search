# Search documentation

Keep only the documents needed for the current change in context.

| Document | Purpose |
|---|---|
| [Architecture](architecture/README.md) | Product purpose, observable requirements, boundaries and principles. |
| [Implementation](implementation/README.md) | Selected current-format mechanisms, owners, order and removal rules. |
| [Audit closure](audit/AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) | Checked surface, replacement mistakes, rewrite rules and remaining runtime evidence. |
| [Product behavior](product/AGENT_ANALYSIS_FRAMEWORK.md) | Navigation, evidence, comparison, document and exact-verification outcomes that must survive simplification. |

[AGENTS.md](../AGENTS.md) defines repository work. The nearest owner `FUNCTIONS.md` contains the one concrete operation recipe. The active issue scopes the change and evidence; it does not copy another implementation specification.

Read supporting evidence only when relevant:

- [distilled source audit](audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md);
- [donor evidence and limits](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md);
- [workspace disposition](audit/WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md);
- [current ownership graph](handoff/DEPENDENCY_GRAPH.md);
- selected configuration, ADR, runtime or optional-profile document tied to the real consumer.

The old Architecture 8.4 master, package-status graph, P00 contracts, handoff packets and `swarm/**` control plane are preserved by Git history, not the current tree. They cannot authorize work, freeze APIs, require compatibility or restore waves/tickets/leases/receipts.

Do not create another document hierarchy, status database or audit gate. Correct the owning rule and its actual callers when a contradiction is found.
