# Working on the CLI

Read [FUNCTIONS.md](FUNCTIONS.md), then only affected client/protocol/daemon boundaries. Follow the root
working method; do not reload every product/audit document for a CLI change.

Preserve native arguments, typed complete-terminal output, live cancellation, finite diagnostics and owned-child
cleanup. The CLI owns no source scope or store access. Use the one current framed client, not old recipes,
pairing/grants, endpoint discovery, tab/hex conversion or another subprocess loop.

Move real consumers and delete displaced routes/fixtures together. Protect shared revision-key owners and #598
when removing provider-pairing code. After production changes, minimal locked Rust 1.98 check/Clippy must cover
actual CLI/daemon binaries when their wiring changes.
