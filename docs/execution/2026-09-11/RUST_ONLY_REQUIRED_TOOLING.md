# Rust-only required tooling

Issue: #188

Normative references:

- `docs/execution/2026-09-05/tasks/T41.md`
- PR #138
- `xtask/src/agent_drafts.rs`
- frozen `qualification/**/cases-v1.toml` inventories

## Current baseline

Rust replacements exist on `main` for accepted-evidence validation and W1–W3
agent-draft validation. Their Python implementations were removed only after a
Rust owner existed. W4, milestone packets and other required validators still
contain Python entrypoints.

## Work order

1. Port W4 agent-draft validation into `xtask::agent_drafts`, sharing only
   genuinely common TOML/file/report helpers.
2. Switch `tools/validate-w4-agent-drafts.ps1`, the manual workflow and the W4
   qualification README to the Rust command; then delete the Python file.
3. Port W1–W3 milestone-packet validators as one bounded family with
   wave-specific rule modules.
4. Continue through required coverage/package-map/context/ticket validators in
   small slices; never delete first and reduce validation coverage later.
5. Separate optional developer helpers from required release/product tooling.

## Implementation rules

- preserve stable report fields and exit-code semantics where external wrappers
  consume them;
- preserve all negative causal rules represented by the frozen case inventory;
- keep workflows manual-only and read-only;
- do not issue tickets, leases, handoffs, gates or launch authority;
- do not add Python/Node as a hidden subprocess of the Rust command;
- split oversized Rust modules rather than replacing many large Python files
  with one large Rust file.

## Acceptance

- no required wrapper, workflow, qualification README or release command invokes
  Python or Node;
- every removed Python validator has one explicit Rust command owner;
- frozen positive/negative cases remain represented;
- optional leftovers are documented as non-required or deleted;
- tests not yet executed are reported as `NOT_RUN`, never as passing.