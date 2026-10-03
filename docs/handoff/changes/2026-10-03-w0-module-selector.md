# Contract change request — W0 module selector parity

Status: **ACCEPTED_INTEGRATION_CORRECTION** (2026-10-03). Integration tooling only; no ticket, lease, writer authority,
package acceptance or launch-state change is created by this request.

## Identity

- Requesting owner: integration tooling.
- Contract owner: integration-owned planner and bounded-context contracts.
- Base: `e190b9bfa7631b87306a6188007a0867e2bcf243`.
- Reproducer: committed candidate `231fa170ec33b385ea9591e2f689d520823083ee`; its actual-repository
  builder diagnostic rejects `swarm/modules/w0.toml::package[name=search-contracts]` as unsupported.

## Blocking problem

Root AGENTS.md and AUTHORITY_MAP.md require the exact package module entry in every writer context.
The committed P00 context draft includes this selector. The existing context-artifact candidate
contract explicitly allows it, and its extractor resolves exactly one matching package row.
`TICKET_ISSUANCE_PLANNER_V2.md` section 5 and the shared ticket planner's resolver still omit this
registry path. A structurally valid mandatory context therefore cannot reach the advisory ticket plan.
Dropping the module entry would violate the bounded-context contract.

## Proposed correction

Add exactly this form to the planner's closed W0 grammar:

```text
swarm/modules/w0.toml::package[name=<package>]
```

The selected identity must equal the caller's already validated package name. The immutable Git
document must contain exactly one matching `[[package]]` row; missing or duplicated matches return
`CONTEXT_SELECTOR_NOT_UNIQUE`. Unsupported paths, stages, expressions or package identities remain
`CONTEXT_SELECTOR_INVALID`. No arbitrary module path, future-stage packet, glob, dependency source or
additional context source is allowed. Existing file/count/byte ceilings remain unchanged.

Keep source identity at the exact requested Git commit. Supporting the declared selector does not
materialize an authoritative context, satisfy a prerequisite, select a writer, create a lease or permit
implementation. The planner remains advisory and emits no control records.

The integration owner must update the declared grammar and its exact implementation/consumer bindings
together after independent acceptance. Preserve captured historical fixtures; add focused valid,
missing/duplicate, wrong-package and wrong-path coverage rather than rewriting their historical bytes.
Public Rust helper compatibility and the complete caller set must be checked before implementation.

## Related source mismatch

The same diagnostic also reports the planner expecting orchestration schema 5 while the committed
registry and launch binding are both 6. Aligning those two expected versions and their fixture pins with
the existing version-6 contract is an implementation correction, separate from this grammar extension.
It must preserve the exact-version check and current launch classifications.

## Decision

Accepted classification: compatible derivative contract correction aligning the ticket planner with
the already mandatory W0 module-context contract. Independent Luna review confirmed the contradiction,
exact path/package/uniqueness boundary and unchanged source/stage scope before this integration-owner
decision. Part I behavior, package write scope, ordinary/P00 context ceilings, qualification and issued
state are unchanged. No architecture-master read or product change is requested.

The implementation must preserve the exported `SelectorDocs` shape and `resolve_selector` signature.
An additive resolver entry point accepts the W0 module document; the issuance builder loads that document
from its same immutable `GitTree`. Existing callers and captured selector fixtures remain valid under
their original entry point. The extended entry point delegates all previously supported forms to that
original resolver. A new public helper is integration tooling, not a Search package API or wire change.

Acceptance of this correction is a contract decision. It is not an executed test, authoritative review
receipt, qualification, package handoff, or implementation completion. The corrected source and focused
negative coverage require subsequent independent review before publication.
