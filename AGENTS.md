# Working on ELIOT Search

Read these two documents at the start of implementation:

1. [Architecture](docs/architecture/README.md): product purpose, boundaries and principles. Keep it in
   the lead developer's context.
2. [Implementation](docs/implementation/README.md): current owners, concrete integration, removals and
   donor APIs. Read the code for the part being changed.

The maintainer's current instructions govern. Historical masters, issue trains and package contracts
cannot restore retired legacy, cryptographic or process requirements.

## Make the application work

Trace the entrypoint, data and actual callers before editing. Reuse delivered code where it serves the
result; a type, trait or closed issue does not prove a working consumer. Change an owner and its real
consumers together. Remove the replaced implementation in the same coherent change.

Do not manufacture grants, digests, probe receipts or placeholder IDs to satisfy an obsolete API.
Simplify that API and its users. Internal/pre-release formats need no compatibility layer. Reject an
unsupported old root before writing and leave it intact; no migration/export platform is a prerequisite
for the new application.

Use established dependencies directly where practical. New wrappers, crates and generic engines require
a concrete role in the connected result. Do not preserve a duplicate implementation to host old tests.
Package instructions describe surviving responsibilities, not permanent file quotas or permission walls.

## Verify the actual change

Use the pinned Rust toolchain and locked dependencies for changed production targets and their real
consumers. After integration, exercise the behavior that could fail. Keep exact source/identity,
publication/recovery, scope, cancellation and output checks relevant to that change.

Do not run or repair obsolete all-target harnesses, regenerate retired maps or maintain source-hash
ledgers to obtain a badge. No prescribed scope form, commit freeze, review count or receipt is required.
Report what works, what was removed, meaningful checks and actual remaining limitations in the existing PR.
Keep Actions manual-only. Do not commit credentials, private captures or local logs.

## Current work

This preparation remains documentation-only in [PR #610](https://github.com/UnknownAlienHuman/eliot-search/pull/610).
[#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) and the actual source handoff identify
active work. Preserve #598's manager, private branch and unpublished changes; do not introduce a competing
writer into that work. Coordinate real overlapping files, not an invented crate-per-agent organisation.
Update existing PR/issue bodies rather than posting progress comments.
