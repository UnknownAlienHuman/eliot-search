# P00 foundation acceptance and W1 unlock matrix

**Status:** normative preparation contract; not executed. This document and its machine registry do not
materialize context, issue a ticket or lease, accept a package, satisfy G0/W0 or authorize W1.

Machine authority: [`../../swarm/p00-foundation-acceptance.toml`](../../swarm/p00-foundation-acceptance.toml).
Operation semantics remain owned by [`TICKET_ISSUANCE_OPERATIONS.md`](TICKET_ISSUANCE_OPERATIONS.md), the
closed schemas under `swarm/schemas/`, `swarm/orchestration.toml` and `swarm/launch-state.toml`.

## 1. Purpose

P00 has three different acceptance layers that must not be collapsed:

```text
package submission accepted for integration
→ package handoff published

all three package handoffs + exact G0 evidence accepted
→ G0 accepted

G0 + all three handoffs + no unresolved P00 work
→ W0 receipt + reviewed launch-state advance
```

A green package test, review, package handoff, structural workflow or complete set of three Cargo packages
is not by itself G0 or W0 acceptance.

## 2. Current authority

```text
active phase/wave:              P00 / W0
authorized package:             search-contracts
conditional packages:           search-domain, search-ports
materialized contexts:          0
issued tickets:                 0
active writer leases:           0
package submissions:            0
accepted independent reviews:   0
accepted package handoffs:      0
accepted G0 receipt:             absent
accepted W0 receipt:             absent
W1 authority:                    blocked
```

The three ticket/context drafts are non-claimable source material only. Their presence does not satisfy
any row in this matrix.

## 3. Record ladder per package

Every P00 package uses the same explicit record ladder:

| Step | Required immutable record or state | Producer | Consumer | Claim ceiling |
|---|---|---|---|---|
| 0 | non-claimable ticket/context drafts | integration owner | context materializer | no authority |
| 1 | `context_manifest_v1` plus one writer-visible artifact | integration owner | ticket issuer/writer | context identity only |
| 2 | `assignment_ticket_v1` | integration owner | lease issuer | package assignment only |
| 3 | `writer_lease_v1` | integration owner | exact writer | lease exists, implementation still blocked |
| 4 | `lease_event_v1` / `ACKNOWLEDGED` | assigned writer | orchestrator | one package may enter implementation |
| 5 | `package_submission_v1` | integration owner from writer evidence | independent reviewer | review candidate only |
| 6 | `independent_review_v1` / accepted verdict | independent reviewer | integration owner | submission accepted for integration |
| 7 | `package_handoff_v1` | integration owner | dependent packages/W0 closer | package acceptance only |

No step may be inferred, copied from a draft or replaced by prose. Every record is committed, read back and
verified by immutable repository/commit/path/blob/exact-file digest before consumption.

### 3.1 Writer/reviewer separation

For every package:

```text
writer != reviewer
writer != integration acceptance authority
reviewer cannot publish the package handoff
writer cannot edit ticket/context/lease/submission/review/handoff records
```

A rejected submission returns through a new context/ticket revision and the ordinary `READY → LEASED`
transition. Rejection cannot preserve an old active lease or jump directly back into implementation.

### 3.2 Package-only scope

The complete final diff must stay inside the exact leased scope:

```text
search-contracts  crates/search-contracts/**
search-domain     crates/search-domain/**
search-ports      crates/search-ports/**
```

Shared Cargo manifests, lockfile, registries, gate evidence, workflows and launch state remain integration
owned. A package writer reports a required shared change as a contract/change request; it does not widen
its lease.

## 4. Dependency and scheduling matrix

| Package | Launch class | Required accepted predecessor | Earliest issuance | Parallelism | Handoff effect |
|---|---|---|---|---|---|
| `search-contracts` | `AUTHORIZED` | none | after exact context/base/writer/reviewer selection | none | unlocks conditional issuance for domain/ports |
| `search-domain` | `CONDITIONAL` | exact accepted `search-contracts` handoff and API/schema digest | only after contracts handoff readback | may run with ports | contributes one P00 package handoff |
| `search-ports` | `CONDITIONAL` | exact accepted `search-contracts` handoff and API/schema digest | only after contracts handoff readback | may run with domain | contributes one P00 package handoff |

`search-domain` and `search-ports` may run concurrently only with separate materialized contexts, tickets,
leases, worktrees, writers and independent reviewers. Both must bind the same immutable accepted contracts
handoff identity; neither may consume the contracts implementation source tree.

A contracts API digest without the accepted handoff record is insufficient. A handoff record with a
mismatched final commit, exact-file digest or public API/schema digest is also insufficient.

## 5. Checkpoints

### P00-A — issue `search-contracts`

Required before implementation:

```text
current launch authority says search-contracts is authorized
exact algorithm-tagged immutable base commit selected
distinct writer and reviewer selected
context draft validated and materialized from that exact commit
context manifest and artifact committed and read back
assignment ticket committed and read back
no competing active package lease
writer lease committed and read back
exact writer acknowledgement recorded as ACKNOWLEDGED lease event
```

The checkpoint grants implementation authority to exactly one writer for exactly
`crates/search-contracts/**`. It accepts no package, gate or wave.

### P00-B — accept `search-contracts`

Required before the conditional packages can be issued:

```text
complete package-only final commit
complete immutable package submission
public API/schema/reason/fixture identities recomputed
required commands and raw outcomes recorded
unavailable checks remain visible
line budget checked
independent reviewer recomputes scope, contracts, tests and digests
accepted review contains no unresolved blocking finding
integration owner publishes package_handoff_v1
exact handoff readback succeeds
```

Only this accepted handoff may satisfy the conditional dependency slot for `search-domain` and
`search-ports`.

### P00-C — issue and accept `search-domain` and `search-ports`

Each package independently repeats the full record ladder. Their tickets must bind:

```text
same accepted search-contracts handoff record
same accepted contracts final commit and public API/schema digest
package-specific context artifact
package-specific write scope
package-specific writer and independent reviewer
package-specific required evidence and line budget
```

Completing both package handoffs yields three accepted P00 package handoffs. It still does not produce G0
or W0 acceptance.

### P00-D — close G0 and W0

The integration owner may prepare the W0 closure only when:

```text
accepted handoffs exist for contracts, domain and ports
all handoffs pass exact immutable readback
all ten G0 evidence IDs have PASS records
all evidence records bind immutable raw output
all evidence has independent review
no active P00 writer lease remains
no unreviewed submission remains
no unresolved blocking contract challenge remains
```

The accepted G0 receipt, W0 receipt and launch-state update must be one separately reviewed integration
change. A package handoff cannot accept the gate/wave, and a gate receipt cannot silently edit launch
state.

## 6. Exact G0 evidence ownership

The required set must equal `swarm/gates.toml` exactly.

| G0 evidence ID | Primary producer | Required package inputs | Acceptance rule |
|---|---|---|---|
| `architecture_hash_challenge` | integration owner | none | exact architecture hash/challenge raw result |
| `workspace_registry_assignment_parity` | integration owner | all three accepted handoffs | Cargo/package/function/stage/assignment closure |
| `dependency_graph_acyclic` | integration owner | all three accepted handoffs | exact graph result, no ignored edge |
| `dependency_direction_policy` | integration owner | contracts/domain/ports | exact dependency/public-type boundary result |
| `recipe_set_exact` | `search-contracts` evidence, independently reviewed | contracts handoff | exact eleven-recipe fixture result |
| `epoch_and_sentinel_contract` | `search-contracts` evidence, independently reviewed | contracts handoff | exact epoch/sentinel fixtures and negatives |
| `canonical_public_schema_fixtures` | `search-contracts` evidence, independently reviewed | contracts handoff | canonical JSON/CBOR/schema fixture digests |
| `reason_code_registry` | `search-contracts` evidence, independently reviewed | contracts handoff | exact public reason registry and unknown rejection |
| `contract_domain_tests` | integration owner | all three accepted handoffs | cross-package contract/domain/port compatibility suite |
| `dependency_source_and_license_policy` | integration owner | all three accepted handoffs | exact dependency source/license/pinning result |

A package handoff may carry evidence candidates, but the W0 closer independently validates and freezes the
G0 evidence records. `UNAVAILABLE`, missing raw output, stale commit binding, self-review or prose-only
claims cannot satisfy the gate.

## 7. W0 receipt contents

The W0 receipt must bind at least:

```text
repository and exact reviewed integration commit
architecture version/hash
G0 receipt reference and exact-file digest
three package handoff references and exact-file digests
three accepted final commits
three public API/schema digests
complete G0 evidence record set and raw-output digests
toolchain/dependency/lockfile identity used for closure
command outcomes and explicitly unavailable checks
no-active-lease and zero-unreviewed-submission checks
independent reviewer identity
launch-state before/after digest
```

The receipt is append-only. Correction requires a new receipt and explicit supersession; the accepted
package handoffs remain immutable.

## 8. W1 unlock matrix

`swarm/stages.toml` requires both accepted `G0` and accepted `W0` before W1. The launch state may move to
wave 1 only after P00-D.

| W1 condition | Required value |
|---|---|
| accepted gate | `G0` |
| accepted completion receipt | `W0` |
| active wave after reviewed update | `1` |
| active phase/stage | `P01-P02` / `W1` semantics |
| P00 package handoffs | contracts, domain, ports all exact and accepted |
| active P00 leases | `0` |
| unreviewed P00 submissions | `0` |
| W1 package authorization | explicit launch-state update; never inferred from stage/package presence |

The following do not unlock W1:

```text
three crates compile
three package handoffs exist but G0 is incomplete
G0 passes but W0 receipt is absent
W0 receipt exists but launch state was not reviewed/advanced
a workflow is green
configuration or W1 packets exist
Cargo.lock exists
an agent starts work on a W1 branch
```

## 9. Failure and recovery rules

Hard stops include:

- conditional ticket issued before the exact contracts handoff exists;
- writer and reviewer identity collision;
- context/ticket/lease disagreement on base, scope, profile or dependency handoff;
- implementation before an `ACKNOWLEDGED` lease event;
- incomplete or out-of-scope changed-file set;
- accepted review without independent recomputation;
- package handoff published by writer/reviewer rather than integration owner;
- G0 evidence set differs from the exact gate registry;
- evidence PASS without immutable raw output and independent review;
- active lease or unreviewed submission at W0 closure;
- W0 receipt and launch update committed as unrelated, unbound changes;
- W1 authorization inferred from a stage packet, Cargo member, config or workflow.

Unknown mutation outcome uses the recovery contract in `TICKET_ISSUANCE_OPERATIONS.md`. Blind retry,
in-place record repair and optimistic absence inference are forbidden.

## 10. Current disposition

```text
matrix/registry:                 defined
package implementation:         absent
accepted search-contracts:      absent
conditional issuance:           blocked
accepted P00 package handoffs:  0 / 3
accepted G0:                     absent
accepted W0:                     absent
W1 launch authority:             blocked
```
