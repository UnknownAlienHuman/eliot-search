# Analysis registry

Current next step and evidence only. Algorithms stay with their owners.

<a id="resume"></a>
<a id="resume-after-interruption"></a>
## Resume

**R064 — concrete implementation choices and removal of competing layer recipes, 2026-10-30.**
Inspected main: `69e85b04c0d1e3d4e8ef1742ec128ecd95cc8c04` (R063). Documentation only; source and dependencies are unchanged.

Next implementation remains [Foundation](../implementation/README.md#1-foundation--invocation-settings-and-root)
at the [actual dispatch](../implementation/CODE_MAP.md#existing-dispatch-to-replace). Use the
[implementation choices](../implementation/WORKING_GUIDE.md#implementation-choices) while writing that path;
do not first rewrite all shared types, ports or domain packages. Stop at the stage's connected result.

<a id="established-decisions--do-not-rediscover"></a>
<a id="decisions-and-owners"></a>
## Selected decisions

[Local scope](../architecture/LOCAL_SCOPE.md) is unchanged. Prefer private functions and concrete owners;
justify abstractions by an actual invariant, variation, dependency or ownership need, not names or line quotas.
Keep effect-specific recovery, source integrity, finite work and the pinned view/window lifetime.

<a id="coverage-ledger"></a>
<a id="changed-documentation"></a>
## Documentation changes

Working guide now defines implementation defaults, data/error/cost handling and causal-path review. Root instructions
and PR template link that method without a new form or framework. Shared-data guidance separates useful typed
invariants from cloned DTO/plan/receipt families. Ports/domain/planner entrypoints no longer duplicate algorithms
or prescribe a global layer-rewrite queue. Planner consumes the local scope owner and keeps existing windows;
it no longer specifies removed session/access restrictions.

<a id="delivery-remaining"></a>
## Remaining implementation

Foundation → one retained file through Qdrant and the actual CLI → later delivery stages. Existing legacy code
still needs real caller/Cargo/fixture cutovers. #610 remains unmerged; no source removal is inferred from this edit.
Preserve unpublished work and user bytes/keys. No product stage is completed by documentation.

<a id="evidence-status"></a>
<a id="execution-evidence"></a>
## Evidence

| Check | Result / limit |
|---|---|
| Inputs | All 14 edited files match GitHub blob IDs at the inspected main. An older archive supplied only byte-matching inputs/reference material, not a current checkout. |
| Review | Compared shared-data/domain/ports/planner entrypoints with current owner recipes and local scope. No fresh complete source-caller census or independent-agent review. |
| Navigation/diff | 41 changed-page relative links resolved against candidate files and nine pinned supporting-file reads; 68 previous heading/explicit anchors retained; git diff --check passed. No full repository or external-URL crawl. |
| Scope | Markdown only; no code, Cargo/lockfile, configuration, workflow or test-source change. No new checker or task database. |
| Execution | No Rust build, Clippy, product tests or runtime qualification. Existing library/version selections were not changed or re-audited. |

<a id="evidence-index"></a>
<a id="prior-evidence"></a>
## Previous evidence

[R063 registry](https://github.com/UnknownAlienHuman/eliot-search/blob/69e85b04c0d1e3d4e8ef1742ec128ecd95cc8c04/docs/audit/ANALYSIS_REGISTRY.md)
retains previous checks. Update the next connected result and actual evidence, not another specification or diary.
