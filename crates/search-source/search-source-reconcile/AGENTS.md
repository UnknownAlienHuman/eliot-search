# search-source-reconcile — agent instructions

Read the [architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md). This package contributes change comparison
to the existing ingestion path. [FUNCTIONS.md](FUNCTIONS.md) distinguishes source from missing wiring.

- Keep the useful comparison of source identity, path, presence and content revision. Return an
  ordinary decision that the source owner applies; pure comparisons need no operation receipt.
- Connect one daemon scheduler to selected roots, watcher hints, inventory and the existing reader/
  revision writer. Do not build a supervisor, event archive or receipt protocol for each root.
- Coalesce repeated path hints. On overflow, restart/resume or uncertain enumeration, rescan the
  affected scope. An event alone cannot confirm new bytes or delete a source.
- Apply confirmed changes through the source owner and short redb transactions. Preserve unknown
  or unreadable scope; do not retire unseen entries after a partial scan or claim it is current.
- Use the selected walker and Git backend described in [FUNCTIONS.md](FUNCTIONS.md). Preserve explicit
  working-tree/index/commit identity and ignore semantics; do not write another Git or ignore parser.
- Remove the old `ReconcileLedger` and receipt-only inputs with their consumers when replacing this
  path. Existing reason enums, hypothetical logical APIs and old source-line quotas are not obligations.

Check actual missed-change, partial-scan, rename, cancellation and unchanged-content behavior when
implementing it. A passing comparison unit test does not establish a connected watcher or sync command.
