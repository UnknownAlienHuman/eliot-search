# Historical accepted-evidence digest v1

This profile hashed an accepted package handoff's `evidence[]` array into another canonical SHA-256 identity
and surrounded it with machine profiles, parity fixtures, wrappers and a qualification workflow.

The current project does not use accepted package handoffs, gates or waves as implementation authority.
Therefore the digest is redundant: Git commits/PRs and direct raw command/runtime outputs already identify the
actual evidence when it matters.

Retire together:

```text
xtask accepted-evidence compute/validate modules
swarm/accepted-evidence-digest-v1.toml
related type-rule/context-planner entries
PowerShell wrappers
qualification/accepted-evidence fixtures/workflow
documentation and coverage-map consumers
```

Do not replace it with a new canonical record, signed handoff, review receipt or smaller digest registry.
Keep real content/artifact hashes and behavior results only where a concrete product/release consumer uses
them. Historical bytes remain in Git.

Current tooling direction is [`../../xtask/README.md`](../../xtask/README.md).
