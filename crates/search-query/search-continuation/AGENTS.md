# Working on search-continuation

Start with the [architecture](../../../docs/architecture/README.md),
[query implementation](../../../docs/implementation/README.md#4-simplify-and-connect-the-query) and
[one-owner design](README.md#one-temporary-owner). Read current source and both shared/native and public
daemon callers before changing the owner.

- Reuse the existing daemon offset and commit-after-delivery mechanism in one surviving window owner.
  Update consumers and remove the replaced engine; do not add another continuation implementation.
- Own real view/retention resources, ordered bounded items, next index and an absolute expiry.
  A permit, supplied boolean or cleanup receipt is not ownership or completion of its external effect.
- Page work follows the selected page, not the previously issued prefix or whole source catalog.
  Bound total retained bytes as well as counts; preserve exact source/revision and ranked order.
- Keep failed output from advancing the cursor. Close a partially written framed session. Do not
  introduce a durable acknowledgement/replay platform for ordinary local reads.
- Expire windows and resources on their actual lifetime; restart may invalidate them. If issued item
  locators depend on the window, paging completion cannot delete their backing items early.
- Enforce real restrictions for selected restricted clients. Simplify trusted-local API and callers
  together; no fake grant or blanket bypass replaces scope, purge or source validation.

Check paging cost, output failure and actual resource release. Fixed recipes, contract-change approval,
legacy checkpoint formats and line quotas do not constrain this implementation.
