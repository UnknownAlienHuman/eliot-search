# Working on ELIOT Search

Start with the [architecture](docs/architecture/README.md) and the
[implementation guide](docs/implementation/README.md). Keep the short manifesto in the lead developer's
context; load only the code and local implementation notes needed for the current change.

The maintainer's current request governs. Audit tables record findings; they are not another specification,
permission system or prerequisite for coding. Historical waves and package contracts cannot restore retired
compatibility, cryptographic or process requirements.

## Implement the behavior, not the diagram

Trace the real input, owner and consumer. Reuse a correct implementation; change the defective responsibility
and its immediate consumers together, then remove the replaced path. A pseudocode type, candidate donor,
package row or issue number is not an instruction to create a class, trait, crate, registry or worker.

Use direct calls and ordinary data first. Isolate a real dependency, effect or repeated policy only where it
helps the current operation. Do not invent a second implementation to justify a trait or split a function to
satisfy a line quota. Internal/pre-release APIs need no compatibility wrappers. Reject unsupported stored
roots before writes and preserve user data.

A document rule has one home. Architecture states purpose; implementation selects mechanisms; local notes
record exact source seams and unresolved decisions. Other READMEs and issues link to that rule instead of
copying a second algorithm. When a conflict is found, correct the owning rule and affected callers; do not
satisfy both by adding adapters, fake receipts or more versions.

## Scope and verification

Implement only capabilities selected for the current change. Deferring an implementation does not silently
remove a user requirement from the product. A one-file lexical path is a milestone, not the complete product.
Use the pinned compiler and minimal Clippy closure for changed production owners and real consumers. Broader
runtime/native/scale work follows implementation; no source-shape, receipt or qualification gate substitutes
for behavior. Keep Actions manual-only and secrets/private captures out of Git.

## Current work

Preparation remains in [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).
After an interrupted audit, use the [registry resume point](docs/audit/ANALYSIS_REGISTRY.md#resume-after-interruption)
and its topic/checkpoint index. Verify actual PR head before editing; chat summaries and main-branch search are
not current-branch evidence. Record inspected ranges, findings, changed code and execution separately in that
same registry, preserving prior immutable evidence links. Do not requeue a documented defect as unread source.

Use [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) to coordinate actual overlapping work.
Do not inspect or modify #598's unpublished branch, reset its worktree or create a competing object/key
writer. That restriction applies to its owned files and handoff, not all code in the repository. Its old
six-domain, retention, purge and compatibility programme is not inherited by this refactor. Public issue
contracts may be reviewed without touching private work.

Use one manager-owned worktree when working locally. Report findings and remaining uncertainty in the
existing PR/issue bodies; do not create a second audit-control system or claim exhaustive closure.
