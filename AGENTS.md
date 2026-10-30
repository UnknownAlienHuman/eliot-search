# Working on ELIOT Search

Read the [resume point](docs/audit/ANALYSIS_REGISTRY.md#resume), the current
[delivery stage](docs/implementation/README.md#delivery-sequence), then only its owner recipe and actual callers.
The [working guide](docs/implementation/WORKING_GUIDE.md) gives commands, checks and update rules;
the [code map](docs/implementation/CODE_MAP.md) identifies real entrypoints.

## Product boundary

Follow [local product scope](docs/architecture/LOCAL_SCOPE.md): one OS account, same-host Qdrant, plain storage,
owned-child stdio. No application encryption, key platform, pairing/grants, remote users or multi-tenant policy.
Keep data integrity, no-execute input handling, bounded work and preservation of user files.

## Implement

Use one manager-owned worktree. Delegate only disjoint reads/edits; one manager owns interfaces and integration.
Leave unpublished #598 work untouched; it is not a prerequisite or selected crypto integration.

Trace entrypoint → effect/state owner → immediate consumer. Implement that connected change and remove the
replaced route with callers. Keep a crate/trait/worker only for a concrete consumer or substantial responsibility.
Use the [implementation choices](docs/implementation/WORKING_GUIDE.md#implementation-choices): private functions
and concrete state first; abstractions must solve an actual ownership, dependency or duplication problem.
Do not replace code with receipts, hashes, status tables or a new lifecycle framework. No file/line quotas.

## Check and publish

After code, run the smallest locked check and Clippy for changed owners and real consumers; include real binaries
when wiring changes. Tests and broad qualification follow the working product, not this documentation task.
For docs, check links/anchors, whitespace, source claims and agreement with the owning rule.

Update the owning recipe once, its affected incoming links and the existing registry's next step/checks. Recheck
branch heads before publishing. Land coherent docs/code changes independently; do not grow a permanent PR.
No source/manifest/workflow changes are hidden inside a documentation-only landing.

## Current versus historical

Main contains new instructions and older implementation. #610 is separate unmerged source work: its deletions,
parser upgrade and compiler evidence are not automatically main state. Inspect the current checkout.
Old 8.4/P00/wave/swarm/qualification packets are historical inputs, not current requirements or launch gates.
