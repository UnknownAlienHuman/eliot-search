# Local ticket-issuance advisory plans

This directory is reserved for ordinary, non-authoritative JSON output from:

```powershell
cargo run --locked --quiet -p xtask -- build ticket-issuance-plan
```

`tools/plan-ticket-issuance.ps1` is the Windows compatibility entrypoint for the same Rust command.

Generated plans are local preflight artifacts. They are not context manifests, assignment tickets, leases, evidence receipts, package handoffs, gate receipts or wave receipts. Committing a generated plan does not authorize any control-plane operation.

All generated `*.json` files are ignored. The directory metadata remains committed so output fencing can be validated deterministically.
