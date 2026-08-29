# W6 subject-resolution, comparison and exact-proof implementation packet

W6 provides ambiguity-preserving resolution, descriptive cross-repository comparison and an exact
frozen-denominator verification plane. It does not authorize implementation; current launch authority
remains `swarm/launch-state.toml`.

## Packages

| Package | Operation packet | Qualification |
|---|---|---|
| `search-subject-resolver` | `crates/search-query/search-subject-resolver/FUNCTIONS.md` | `qualification/proof/W6_QUALIFICATION.md` |
| `search-comparator` | `crates/search-query/search-comparator/FUNCTIONS.md` | `qualification/proof/W6_QUALIFICATION.md` |
| `search-exact` | `crates/search-query/search-exact/FUNCTIONS.md` | `qualification/proof/W6_QUALIFICATION.md` |

Machine profile/baseline/probe inputs are in `qualification/proof/`.

## Dependency-safe launch order

```text
accepted W0 contracts/domain/ports
accepted W1–W2 source registry, inventory, access, safe read and revision handoffs
accepted W4 query/validator/handle contracts
accepted W5 currentness/overlay/Rust structural profile handoffs
    ↓
search-subject-resolver
    ↓
search-comparator

search-exact
    ← authoritative inventory/access/revision/overlay/structural ports
```

`search-subject-resolver` and `search-exact` may begin in parallel only when their own direct handoffs
and qualification inputs are accepted. `search-comparator` waits for an accepted resolver API digest.

## Cross-package invariants

1. Material ambiguity is an explicit bounded result; resolver never silently chooses one candidate.
2. Resolver ladder order is explicit reference → qualified key → exact name → signature/kind →
   structural/lexical.
3. Invalid explicit references do not disappear into a lower-tier guess.
4. One resolution product binds one coherent source/workspace/security/profile fence.
5. Same-name/top-rank similarity is not identity; rename/alias collapse requires accepted equivalence.
6. Comparison is descriptive and contains no correctness, best-implementation or adoption verdict.
7. Fork/mirror/copy evidence counts once per proven independent lineage.
8. Tests/docs/callers/definitions/configuration remain separate evidence roles.
9. Mutually exclusive configuration variants are not conflicts; unknown predicates are not universal.
10. Local absence from ordinary comparison evidence is not exact absence proof.
11. Exact denominator comes from authoritative inventory, never Qdrant/top-k/client file lists.
12. Exact execution reopens every exact planned revision/snapshot and accounts for every denominator item.
13. Raw-byte, decoded-text and structural-IR predicates have distinct qualified semantics.
14. Regex uses one exact non-backtracking, resource-bounded engine/profile.
15. Timeout, cancellation, unreadable/revision loss, scope drift, observation gap, unsaved expiry,
    access revocation or purge blocks complete negative proof.
16. `NoMatchInCompleteScope` proves only the compiled predicate over the exact frozen scope.
17. Qdrant payload text is never exact evidence.
18. Historical frozen proof and current-workspace proof have distinct drift/revalidation semantics.

## Hard stop conditions

- resolver emits `RESOLVED` from a materially ambiguous or incomplete higher-priority ladder;
- comparison inflates fork count or produces a normative verdict;
- cfg applicability is guessed or conflicting evidence is silently reconciled;
- exact denominator references indexed candidates/rank output;
- regex/provider/profile identity or resource bounds are incomplete;
- an item failure/omission is excluded from completeness accounting;
- a newer path revision substitutes for an unavailable planned revision;
- incomplete/cancelled/timed-out execution produces complete negative proof;
- semantic analogue absence is inferred from literal/name/regex proof;
- access/purge changes fail to block match/report/handle emission;
- mandatory probe remains `UNAVAILABLE`, raw evidence is absent or producer self-accepts.

Any hard stop keeps P11/P12 blocked.

## Handoff requirements

Each package handoff includes:

- exact accepted direct-dependency commits/API digests;
- operation/profile/policy/fixture digests;
- bounded deterministic/negative/property/fault test outcomes;
- cancellation/deadline and stale/revalidation behavior;
- exact source-backed evidence and content-minimization proof;
- no-vendor-type/no-hidden-verdict/no-Qdrant-denominator API checks;
- all applicable probe IDs from `qualification/proof/probes.toml`;
- line count and split-review state.

The integration owner accepts each package separately and runs the complete W6 corpus before issuing a
G3 proof receipt. Compilation or unit tests alone do not pass P11/P12.
