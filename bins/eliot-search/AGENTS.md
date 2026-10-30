# Working on the CLI

Read [FUNCTIONS.md](FUNCTIONS.md), then only affected client/protocol/daemon boundaries. Follow the root
working method; do not reload every product/audit document for a CLI change.

Preserve native arguments, typed complete-terminal output, live cancellation, finite diagnostics and owned-child
cleanup. The CLI owns no source scope or store access. Use the one current framed client, not old recipes,
pairing/grants, endpoint discovery, tab/hex conversion or another subprocess loop.

Move real consumers and delete displaced routes/fixtures together. Remove unneeded public credential/key wiring
under [local scope](../../docs/architecture/LOCAL_SCOPE.md#remove-legacy-security-wiring); preserve actual shared
OS/file helpers, user bytes/keys and unpublished #598 work. Do not preserve the old public key layer by default.
After production changes, minimal locked Rust 1.98 check/Clippy must cover
actual CLI/daemon binaries when their wiring changes.
