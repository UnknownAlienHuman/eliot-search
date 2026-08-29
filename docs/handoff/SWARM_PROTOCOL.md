# Swarm implementation protocol

## 1. Unit of work

One writer owns one Cargo package. One package may have one read-only reviewer and one integration
reviewer. Family directories are navigation only and never become shared implementation locations.

Each writer runs in a dedicated worktree and branch:

```text
worktrees/<wave>/<package>/
swarm/<wave>/<package>
```

The assignment includes one immutable base commit and the accepted handoff commits of direct
dependencies. Writers do not track moving dependency branches.

## 2. Agent context

The orchestrator provides only:

- root `AGENTS.md`;
- nearest family `AGENTS.md`;
- package `AGENTS.md` and README;
- direct dependency README/API notes;
- assigned issue and accepted contract decisions;
- relevant deterministic fixtures.

The full architecture master is not normal context. An agent may open it only to resolve a documented
contradiction or missing load-bearing field, then cites the exact section in a contract request.

## 3. Write isolation

A writer changes only its package directory. It does not edit:

- another package;
- root workspace/toolchain/lockfile/CI;
- `docs/generated/`, `swarm/` or shared control corpus;
- dependency APIs;
- architecture or ADRs.

The integration owner performs approved root, lockfile, generated-schema and cross-package changes in a
separate branch. Shared fixture changes are made by their recorded owner.

## 4. Contract-change path

A blocked writer stops at the boundary and files a request using
`swarm/CONTRACT_CHANGE_TEMPLATE.md`. The request must identify:

- missing or contradictory field/operation;
- owning producer and consuming package;
- invariant and failure mode;
- wire/serialization/version impact;
- security/currentness/retention impact;
- migration and compatibility impact;
- proposed tests;
- whether an ADR or architecture revision is required.

No local duplicate type, stringly-typed escape hatch, vendor leakage or silent fallback is permitted.
The contract owner and integration reviewer decide; downstream work resumes from an immutable accepted
commit.

## 5. Implementation order

Only the active wave may write behavior. Within a wave, packages start when their direct dependencies
have accepted handoffs. The integration owner merges topologically, runs workspace policy checks and
publishes a wave receipt. Later waves consume that receipt, not unreviewed branches.

## 6. Tests and receipts

Package-local invariants and fault tests live with the package. Cross-product fixtures have explicit
owners in `tests/CRATE_FIXTURE_OWNERS.md`. A handoff includes raw commands and outcomes, not a prose
claim that tests passed.

Required receipt fields are in `swarm/PACKAGE_HANDOFF_TEMPLATE.md`. Failed, skipped and unavailable
checks remain visible. No agent invents a result for unavailable Windows/Qdrant/redb tooling.

## 7. Review

The package reviewer checks ownership, invariants, failure semantics, tests, size and dependency
direction. The integration reviewer checks public compatibility, graph direction, generated schemas,
workspace build and gate evidence. Use `swarm/REVIEW_CHECKLIST.md`.

## 8. Merge discipline

- Rebase/merge from the immutable assignment base only through the integration owner.
- Never merge two writers that edited the same package.
- Contract changes merge before their consumers and receive a versioned receipt.
- Optional provider code remains disabled and removable.
- A green compile is not product acceptance; P15 alone records the product verdict.
