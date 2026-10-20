# Retired assignment protocol

ELIOT Search no longer uses repository-local assignment tickets, materialized prompt contexts, writer leases,
acknowledgements, submissions, review receipts, package handoffs or launch-state transitions.

For current work:

```text
maintainer request / GitHub issue
→ one manager-owned non-overlapping branch/worktree
→ concrete producer-to-consumer change and deletion
→ minimal changed-owner check/Clippy
→ focused real behavior when available
→ ordinary PR review/merge
```

Exact base revision, bounded scope, dependency compatibility and honest evidence remain useful engineering
facts. They do not require persistent actor state, signatures, control-plane schemas or accepted API digests.

All records under `swarm/tickets`, `context-manifests`, `leases`, `submissions`, `reviews`, `handoffs`,
`supersessions`, `wave-receipts` and their draft/schema/template files are historical removal inputs. Do not
create new records or port their state machines into Rust tooling. Removal is tracked by #214/#525.
