# Working on the CLI

Read [FUNCTIONS.md](FUNCTIONS.md), then only the affected daemon/root boundaries. Follow the repository
working method; do not reload every product/audit document for a CLI change.

The current CLI is a one-shot root operator. Preserve native arguments, local help/version, bounded child
output, response validation and ownership of the daemon child. It owns no source scope or store access.
The framed search client, query results and live sessions belong to later delivery stages.

Move actual consumers together and delete displaced code and exclusive fixtures. Do not restore old command,
credential, endpoint-discovery or harness routes to satisfy obsolete tests. Follow
[local scope](../../docs/architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring); preserve user bytes/keys
and unpublished #598 work.

After production changes, minimal locked Rust 1.98 check/Clippy must cover the actual CLI and daemon binaries
when their wiring changes. Child timeout/pipe behavior and installed-baseline claims require their own executed
evidence; a helper probe or passing package check does not establish them.
