# Working on search-source-admission

Read the [architecture](../../../docs/architecture/README.md),
[implementation](../../../docs/implementation/README.md), [README](README.md) and
[source map](FUNCTIONS.md). Trace the native source caller before preserving a policy/receipt type.

Own one pure bounded admission policy:

- validate a portable root-relative locator and available metadata;
- refuse mandatory unsafe classes before body I/O;
- produce explicit allow-read, deny, review, unsupported or incomplete;
- accept post-read facts only to narrow/finalize the decision;
- expose content-free errors and finite work limits.

Do not own filesystem reads, source identity/history, registry mutation, currentness, query access or a
durable admission ledger. Ordinary local ingestion needs no receipt, original-input archive or canonical
batch identity. Remove those packages/types with their last legacy consumer rather than manufacturing a
new current caller.

Use the existing reviewed GlobSet matcher and evaluator. Do not add a second wildcard/regex engine,
canonical writer or permission wrapper. Change the evaluator and native ingestion consumer together; no
contract-change ceremony, package-only edit wall, historical byte promise or line quota blocks the current
format.

Preserve actual fail-closed safety rules while deleting compatibility/proof scaffolding. Verify through the
connected source path that denied input is never read and the accepted owned body goes directly to
retention. Package fixtures alone are not completion.
