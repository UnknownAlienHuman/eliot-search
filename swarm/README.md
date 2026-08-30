# Swarm control files

The repository contains **45 one-writer packages**: 41 libraries and 4 binaries. Cargo membership,
function/stage packets and qualification templates describe boundaries; `launch-state.toml` alone decides
what may run now.

Foundation order:

```text
search-contracts
  ├─ search-domain
  └─ search-ports
```

## Machine registries

- `crates.toml` — package paths, direct dependencies, earliest waves, assignments and limits;
- `function-packets.toml` — exact primary function/contract packet and package-only write scope for all
  packages;
- `stages.toml` — exact W0–W10 package sets, shared current-stage context and gate/completion-receipt
  ordering;
- `stage-readsets.toml` — exact replacement context for the **23 package assignments** reused after their
  earliest wave;
- `gates.toml` — central G0–G6 evidence requirements;
- `launch-state.toml` — current authorization and advancement conditions.

These registries are complementary:

```text
crates.toml
  tells the orchestrator what the package is and what it depends on

function-packets.toml
  tells the orchestrator which base contract and write scope the package owns

stages.toml
  tells the orchestrator why the package is active at the current stage and which shared files apply

stage-readsets.toml
  replaces earlier stage documents with accepted public handoffs when a package returns later

launch-state.toml
  tells the orchestrator whether a ticket may be issued now
```

Presence in the first four files is never implementation authorization.

## Agent and integration files

- `assignments/` — one capability assignment per package;
- `ASSIGNMENT_PROTOCOL.md` — ticket, context, implementation and handoff procedure;
- `INTEGRATION_OWNER.md` — root/cross-package owner;
- `CONTRACT_CHANGE_TEMPLATE.md` — missing/changed contract or port request;
- `PACKAGE_HANDOFF_TEMPLATE.md` — completion receipt;
- `REVIEW_CHECKLIST.md` — package and integration review;
- `../docs/handoff/SWARM_STAGE_READSETS.md` — exact stage-context assembly and examples.

## Context rule

An earliest-wave package receives its package/function entries, assignment/base contract and current
stage shared read set. A package reused later receives one replacement override. The previous stage
packet and dependency implementation internals are not replayed; exact accepted public handoff receipts
replace them.

`eliot-searchd` is progressively re-ticketed after W1 for source, index, query, currentness, proof,
lifecycle, generic-edge and optional composition. These are sequential one-writer package assignments,
not concurrent daemon agents.

Other explicit reentries include:

```text
eliot-search          W8 standalone client delta
search-eval           W9 Product Pulse, then W10 candidate-specific evaluation
search-publication    W7 lifecycle, then W10 scale migration
```

Static context is capped at sixteen files. Ticket-added handoff and fixture references are separately
bounded. Architecture access remains exception-only after a concrete contract challenge.

## Current state

```text
current stage/wave:       P00 / W0
authorized:               search-contracts
conditional:              search-domain, search-ports
later stages:             BLOCKED
stage assignments:        68
later-stage overrides:    23
unique reused packages:   13
maximum static files:     16
runtime evidence:         absent
```

The orchestrator verifies all registry digests and accepted dependency/prior-stage handoffs, creates an
isolated package worktree, rejects out-of-scope writes and merges accepted packages in dependency/stage
order.
