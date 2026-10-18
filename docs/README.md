# Search documentation

The lead developer starts with two documents:

| Document | Purpose |
|---|---|
| [Architecture](architecture/README.md) | Concise product manifesto: why Search exists, useful results, boundaries and principles. Keep it in context. |
| [Implementation](implementation/README.md) | What to write, where the real owners/callers are, what to remove and which donor APIs to use. |

[AGENTS.md](../AGENTS.md) describes repository work and verification. The current maintainer request and
these guides supersede historical compatibility, cryptographic and process obligations.

Read more only for the implementation being changed:

- [Source audit](audit/ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md): evidence for defects and removals.
- [Donor index](audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md): inspected implementations and current adoption.
- [Agent workflows](product/AGENT_ANALYSIS_FRAMEWORK.md): examples of useful navigation and evidence.
- [Config](config/), [contracts](contracts/p00/), [current workspace](current/) and [client](client/):
  existing specific formats/behavior; update them with their real consumers.
- [Evaluation](evaluation/) and [selected profiles](optional/): measure capabilities that actually ship.

The old 8.4 master, wave packets, status vocabularies and historical audits are reference material.
They are not a required reading bundle, implementation queue or permission system. A dated source fact
must be checked against current callers before use. Do not create another status/approval registry.
