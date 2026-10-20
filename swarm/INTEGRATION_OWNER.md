# Current manager boundary

The former integration-owner ticket/lease/wave role is retired. A current manager coordinates one concrete
issue and one non-overlapping branch/worktree; no Search-local control record grants that authority.

The manager must:

- start from the selected exact base and avoid overlapping active private work, especially #598;
- connect one real producer to one real consumer and delete the replaced path in the same slice;
- keep root manifests, lockfile and workflows consistent when source changes require them;
- record persisted-format and unsupported-old-root dispositions explicitly;
- run minimal Rust 1.98 compiler/Clippy for changed production owners and immediate consumers;
- report raw runtime evidence and unavailable checks honestly;
- merge or close the branch through the ordinary GitHub issue/PR workflow.

The manager does **not** publish package API digests, writer leases, accepted handoffs, gate/wave receipts or
launch-state transitions. It does not freeze package boundaries, require contract-first fixture production or
turn structural metadata into product readiness.

Generic agent scheduling, Tasks/WorkScopes/Attempts, reviewer roles and finish authority belong outside ELIOT
Search. Historical integration-owner details remain in Git history and must not be recreated as `v2` tooling.
