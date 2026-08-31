# P00 foundation acceptance and W1 unlock matrix

**Status:** normative preparation contract; not executed. This document and its machine registry do not
materialize context, issue a ticket or lease, accept a package, satisfy G0/W0 or authorize W1.

Machine authority: [`../../swarm/p00-foundation-acceptance.toml`](../../swarm/p00-foundation-acceptance.toml).
Operation semantics remain owned by [`TICKET_ISSUANCE_OPERATIONS.md`](TICKET_ISSUANCE_OPERATIONS.md), the
closed schemas under `swarm/schemas/`, `swarm/orchestration.toml` and `swarm/launch-state.toml`.

## 1. Acceptance layers

P00 has three different acceptance layers:

```text
package submission accepted for integration
→ package handoff published

three package handoffs + exact G0 evidence accepted
→ accepted G0 receipt

G0 + three handoffs + no unresolved P00 work
→ accepted W0 receipt + reviewed launch-state advance
```

A package test, review, package handoff, structural workflow or complete Cargo package set does not imply
a later layer.

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

The three ticket/context drafts are non-claimable source material. Their presence satisfies no acceptance
row.

## 3. Required record ladder

Every P00 package uses this explicit append-only progression:

| Step | Immutable record or state | Producer | Claim ceiling |
|---|---|---|---|
| 0 | non-claimable ticket/context drafts | integration owner | no authority |
| 1 | `context_manifest_v1` plus one writer-visible artifact | integration owner | context identity only |
| 2 | `assignment_ticket_v1` | integration owner | package assignment only |
| 3 | `writer_lease_v1` | integration owner | implementation still blocked |
| 4 | `lease_event_v1` / `ACKNOWLEDGED` | exact assigned writer | one package may implement |
| 5 | `package_submission_v1` | integration owner from writer evidence | review candidate only |
| 6 | `independent_review_v1` / accepted verdict | independent reviewer | integration candidate |
| 7 | `package_handoff_v1` | integration owner | package acceptance only |

No step may be inferred, copied from a draft or replaced by prose. Every consumed record binds repository,
immutable commit, path, Git blob and exact complete-file digest.

### 3.1 Separation of duties

For every package:

```text
writer != reviewer
writer != integration acceptance authority
reviewer cannot publish the package handoff
writer cannot edit control-plane records
```

Rejected work returns through a new context/ticket revision and the ordinary `READY → LEASED` transition.
It cannot retain a stale active lease or jump directly back into implementation.

### 3.2 Package-only scope

```text
search-contracts  crates/search-contracts/**
search-domain     crates/search-domain/**
search-ports      crates/search-ports/**
```

Shared Cargo manifests, lockfile, registries, gate evidence, workflows and launch state remain integration
owned. A writer reports a required shared change; it does not widen the lease.

## 4. Dependency and scheduling matrix

| Package | Launch class | Accepted predecessor | Earliest issuance | Parallelism | Handoff effect |
|---|---|---|---|---|---|
| `search-contracts` | `AUTHORIZED` | none | after exact base/context/actors selection | none | unlocks conditional issuance |
| `search-domain` | `CONDITIONAL` | exact accepted contracts handoff and API/schema digest | after contracts readback | with ports | contributes one P00 handoff |
| `search-ports` | `CONDITIONAL` | exact accepted contracts handoff and API/schema digest | after contracts readback | with domain | contributes one P00 handoff |

Domain and ports may run concurrently only with separate contexts, tickets, leases, worktrees, writers and
reviewers. Both bind the same immutable accepted contracts handoff and neither receives contracts
implementation source.

A public API digest without an accepted handoff record is insufficient. A handoff with a mismatched final
commit, exact-file digest or API/schema digest is also insufficient.

## 5. Checkpoints

### P00-A — issue `search-contracts`

Before implementation:

```text
launch state authorizes search-contracts
exact algorithm-tagged base commit selected
distinct writer and reviewer selected
context materialized from that commit
context manifest/artifact committed and read back
assignment ticket committed and read back
no competing active package lease
writer lease committed and read back
writer ACKNOWLEDGED lease event recorded and read back
```

This grants one-package implementation authority only. It accepts no package, gate or wave.

### P00-B — accept `search-contracts`

Before conditional issuance:

```text
package-only final commit
complete immutable submission
public API/schema/reason/fixture identities recomputed
required commands and raw outcomes recorded
unavailable checks remain visible
line budget checked
independent reviewer recomputes scope, contracts, tests and digests
accepted review has no unresolved blocking finding
integration owner publishes and reads back package_handoff_v1
```

Only this accepted handoff may fill the domain/ports dependency slot.

### P00-C — issue and accept `search-domain` and `search-ports`

Each package independently repeats the full record ladder. Its ticket binds:

```text
same accepted search-contracts handoff record
same accepted contracts final commit and API/schema digest
package-specific context artifact and write scope
package-specific writer and reviewer
package-specific evidence and line budget
```

Two resulting handoffs plus the contracts handoff yield three accepted P00 packages. They still do not
produce G0 or W0 acceptance.

### P00-D — close G0 and W0

The integration owner may prepare closure only when:

```text
all three package handoffs exist and pass exact readback
all ten G0 evidence IDs have PASS records
all G0 evidence binds immutable raw output
all evidence has independent review
no active P00 writer lease remains
no unreviewed submission remains
no blocking contract challenge remains
```

The G0 receipt, W0 receipt and launch-state update belong to one separately reviewed integration change.
A package handoff cannot accept a gate/wave, and a gate receipt cannot silently advance launch state.

## 6. Exact G0 evidence ownership

The required set equals `swarm/gates.toml` exactly.

| Evidence ID | Primary producer | Required package inputs | Acceptance rule |
|---|---|---|---|
| `architecture_hash_challenge` | integration owner | none | exact architecture hash challenge raw result |
| `workspace_registry_assignment_parity` | integration owner | all three handoffs | Cargo/package/function/stage/assignment closure |
| `dependency_graph_acyclic` | integration owner | all three handoffs | exact graph result with no ignored edge |
| `dependency_direction_policy` | integration owner | all three packages | exact dependency/public-type boundary result |
| `recipe_set_exact` | contracts evidence | contracts handoff | exact eleven-recipe fixture result |
| `epoch_and_sentinel_contract` | contracts evidence | contracts handoff | exact epoch/sentinel fixtures and negatives |
| `canonical_public_schema_fixtures` | contracts evidence | contracts handoff | canonical JSON/CBOR/schema fixture digests |
| `reason_code_registry` | contracts evidence | contracts handoff | exact reason registry and unknown rejection |
| `contract_domain_tests` | integration owner | all three handoffs | cross-package compatibility suite |
| `dependency_source_and_license_policy` | integration owner | all three handoffs | exact source/license/pinning result |

Every row requires `PASS`, immutable raw output and independent review. `UNAVAILABLE`, stale commit binding,
self-review or prose-only evidence cannot satisfy G0.

## 7. W0 receipt contents

The append-only W0 receipt binds at least:

```text
repository and exact reviewed integration commit
architecture version/hash
G0 receipt ref and exact-file digest
three package handoff refs and exact-file digests
three accepted final commits
three public API/schema digests
complete G0 evidence records and raw-output digests
toolchain/dependency/lockfile identity used for closure
command outcomes and explicitly unavailable checks
no-active-lease and zero-unreviewed-submission checks
independent reviewer identity
launch-state before/after digest
```

Correction requires a new receipt and explicit supersession. Accepted package handoffs remain immutable.

## 8. W1 unlock matrix

`swarm/stages.toml` requires accepted `G0` and accepted `W0`. Launch may move to wave 1 only after P00-D.

| Condition | Required value |
|---|---|
| accepted gate | `G0` |
| accepted completion receipt | `W0` |
| active wave after reviewed update | `1` |
| active phase/stage | `P01-P02` / `W1` semantics |
| P00 handoffs | contracts, domain and ports exact and accepted |
| active P00 leases | `0` |
| unreviewed P00 submissions | `0` |
| W1 package authorization | explicit launch-state update |

A single satisfied condition does not unlock W1. The following combinations also do not unlock W1:

```text
three crates compile
three handoffs exist but G0 is incomplete
G0 passes but W0 receipt is absent
W0 exists but launch state was not reviewed and advanced
a workflow is green
configuration or W1 packets exist
Cargo.lock exists
an agent starts work on a W1 branch
```

## 9. Failure and recovery

Hard stops include:

- conditional ticket before the exact contracts handoff;
- writer/reviewer identity collision;
- context/ticket/lease disagreement on base, scope, profile or dependency;
- implementation before `ACKNOWLEDGED`;
- incomplete or out-of-scope final diff;
- accepted review without independent recomputation;
- handoff published by writer/reviewer rather than integration owner;
- G0 evidence set differing from the gate registry;
- PASS without raw output and independent review;
- active lease or unreviewed submission at W0 closure;
- unbound W0 receipt and launch update;
- W1 authority inferred from stage, package, config or workflow presence.

Unknown mutation outcome follows `TICKET_ISSUANCE_OPERATIONS.md`. Blind retry, in-place repair and optimistic
absence inference are forbidden.

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
