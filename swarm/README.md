# Retired Search-local swarm control plane

`swarm/` contains the checked-in remains of the pre-release Architecture 8.4 wave/ticket/lease campaign. It
is not current implementation authority, package ownership, repository permission, release state or product
configuration.

Do not read individual TOML records or templates as instructions for current work. In particular, none of the
following may authorize or block a change:

```text
launch-state / stages / gates / wave receipts
package, function, module or coverage maps
assignment tickets, contexts, leases, submissions, reviews or handoffs
accepted-evidence, API or record digests
line budgets, fixed package counts or wave-monotonic dependency rules
```

Current work is governed by root `AGENTS.md`, the architecture manifesto, implementation guide, the concrete
issue/PR and one non-overlapping manager branch/worktree. Git commits and raw command/runtime outputs identify
what was changed and measured.

Issues [#214](https://github.com/UnknownAlienHuman/eliot-search/issues/214) and
[#525](https://github.com/UnknownAlienHuman/eliot-search/issues/525) own removal of the controller schemas,
validators, workflows, generated maps and digest ledger. Historical detail remains in Git; do not create an
archive copy or a smaller replacement controller.

`GITHUB_CONNECTOR_ACCESS.md` remains an operational note about using the connected GitHub API. It grants no
product or review authority.
