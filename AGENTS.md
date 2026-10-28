# Working on ELIOT Search

Read [the documentation index](docs/README.md), the current stage in
[the implementation sequence](docs/implementation/README.md#delivery-sequence), then only the affected
owner's `FUNCTIONS.md`, local instructions and real callers. Use the
[registry](docs/audit/ANALYSIS_REGISTRY.md#resume) for the current branch and evidence, not as another specification.

## Working method

Use one manager-owned worktree and one integration owner. Delegate only disjoint inspection or edits;
the manager owns interfaces, the final diff and commit. Do not modify unpublished #598 work.

Before a code change, name the entrypoint, concrete owner/effect, immediate consumer, displaced route and
smallest compiler/Clippy closure. Then implement that connected path. Trace current callers before deleting
code; old package maps and tests do not make a disconnected abstraction necessary.

Keep a trait, package, worker or dependency only for a real consumer. Remove displaced implementations and
exclusive fixtures together; a temporarily retained route needs a named next deletion, not a compatibility
promise. Preserve user files and sole retained copies. Never substitute receipts, hashes or planning prose
for real behavior. Do not build a second index, controller or universal lifecycle framework.

## Verification

After production changes, run the smallest locked Rust 1.98 check and Clippy for changed owners and immediate
consumers; select actual binaries when their wiring changes. Broad tests and release/scale qualification follow
the connected product, not preliminary documentation. Do not run unrelated suites to qualify a docs-only change.

For documentation changes, check the diff, links/anchors and agreement with the owning rule. Change the rule once
at its owner and link to it elsewhere. Distinguish target design, inspected source and executed behavior.

Before publishing, recheck the actual branch head. Record inspected revision, changes, checks and unrun work in
the existing registry. Update a conflicting issue body and the PR summary without copying algorithms into them.
Report only checks that ran. Do not merge unrelated source work as part of a documentation update.
