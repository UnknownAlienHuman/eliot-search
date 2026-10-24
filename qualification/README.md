# Current evidence boundary

This directory is not a runtime input, capability registry, package acceptance system or product-readiness authority.

The complete historical qualification tree immediately before removal is preserved in Git at commit
[`2f9c9720f3e14799a0f8dfefa00077d2fa6a9887`](https://github.com/UnknownAlienHuman/eliot-search/tree/2f9c9720f3e14799a0f8dfefa00077d2fa6a9887/qualification).
It includes old W1-W10 milestones, agent drafts, gate maps, probe registries, package failures, Qdrant 1.19 evaluation inputs and earlier unit-level reports. Those files describe exact historical revisions only; they must not be copied forward as a current PASS or constructor credential.

The current tree retains only [`CURRENT_FAILURES.txt`](CURRENT_FAILURES.txt), a concise source-backed blocker summary. New runtime evidence belongs in the issue or PR that executed the concrete behavior, with exact commit, platform/backend, command, inputs and raw outcome. Do not recreate a central qualification database or feed its records into application startup.

Current implementation and evidence rules are in:

- [`docs/architecture/README.md`](../docs/architecture/README.md);
- [`docs/implementation/README.md`](../docs/implementation/README.md);
- [`docs/audit/ANALYSIS_REGISTRY.md`](../docs/audit/ANALYSIS_REGISTRY.md);
- the nearest owner `FUNCTIONS.md` and active issue.

Compilation, Clippy, live Qdrant, Windows durability, recovery, relevance and scale remain separate executed claims. A source file, fixture, report or old green probe does not establish them.
