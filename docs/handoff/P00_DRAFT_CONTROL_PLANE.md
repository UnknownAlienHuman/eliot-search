# Historical P00 draft control plane

This file specified an unissued repository-control plane for immutable context artifacts, assignment tickets,
writer leases, acknowledgements, submissions, independent reviews, package handoffs, supersessions and wave
receipts.

That system is retired. It never controlled a working product, and preserving it would require thousands of
TOML records, canonical digests, reason registries, validators and generated coverage routes while the actual
source → index → evidence path remains disconnected.

Current development uses:

```text
one Git issue with a concrete behavior/removal boundary
→ one manager-owned worktree/branch
→ ordinary commits and PR review
→ minimal compiler/Clippy for changed production owners/consumers
→ focused runtime evidence when the feature exists
```

The issue/PR contains the base, scope, dependencies, completion criteria and explicit unresolved facts. Git
provides immutable source/diff/history. No second ticket/context/lease/handoff database is required.

Do not materialize or validate the old draft records, enforce source-count/line ceilings, issue accepted-
package digests or recreate this protocol in another tool. Historical drafts and layouts may be deleted with
their last validator/consumer under [#214](https://github.com/UnknownAlienHuman/eliot-search/issues/214).

The current implementation sequence is [`../implementation/README.md`](../implementation/README.md).
