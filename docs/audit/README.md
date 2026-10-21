# Audit documents

Use this directory through the following current entrypoints:

1. [Audit coverage and documentation map](AUDIT_COVERAGE_AND_DOCUMENTATION_MAP_2026-10-21.md) — what was
   actually checked, remaining gaps and document authority.
2. [Workspace member disposition](WORKSPACE_MEMBER_DISPOSITION_2026-10-21.md) — target decision for every
   Cargo workspace member.
3. [Distilled architecture/code audit](ARCHITECTURAL_AND_CODE_AUDIT_2026-10-17.md) — source-backed findings
   and the reasons for the refactor.
4. [Donor verification register](DONOR_VERIFICATION_REGISTER_2026-10-09.md) — selected donor mechanisms and
   their limits.

Project direction comes from [`docs/architecture/README.md`](../architecture/README.md) and
[`docs/implementation/README.md`](../implementation/README.md), not from an audit snapshot.

## Historical files

Every other file in this directory is historical or supporting evidence unless a current document above
links to a specific claim. In particular, an old title containing `MASTER`, `CURRENT`, `LAUNCH`, `GATE`,
`ACCEPTANCE`, `STATUS` or `IMPLEMENTATION` does not grant current authority.

Historical audits may still contain useful defect observations, measurements or donor notes. Cite their exact
base and limitation. Do not use them to:

- launch or schedule implementation;
- require P00/W0–W10 waves, tickets, leases, handoffs or gate receipts;
- preserve old internal formats, package boundaries or qualification credentials;
- restore universal migration, purge, retention, pairing or proof platforms;
- claim runtime behavior from source, fixtures or structural validation.

The [old master audit](ELIOT_SEARCH_MASTER_AUDIT_2026-10-09.md) is now an explicit historical pointer. Launch
records, completion audits, backlog/status reconciliations and donor-acceptance files remain dated evidence
only.

Git history is the archive. Do not create a second copied archive or update historical reports to mimic the
new architecture. Shorten/delete them only when needed to prevent false current authority or when their final
consumer is removed.
