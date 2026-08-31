# Local ticket-issuance advisory plans

This directory is reserved for ordinary, non-authoritative JSON output from
`tools/plan-ticket-issuance.py`.

Generated plans are local preflight artifacts. They are not context manifests, assignment tickets,
leases, evidence receipts, package handoffs, gate receipts or wave receipts. Committing a generated plan
does not authorize any control-plane operation.

All generated `*.json` files are ignored. The directory metadata remains committed so output fencing can
be validated deterministically.
