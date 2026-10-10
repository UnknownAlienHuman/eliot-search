## Work item

```text
class: SLICE | PROCESS | DOCS
programme_issue:
source_issue:
base_sha:
head_sha:
primary_owner:
state_or_effect:
```

For source work, link `docs/product/EXECUTION_PROTOCOL.md` and paste the final `SCOPE_FROZEN` block below. PROGRAM and GATE coordination/evidence belongs in issues/docs, not a non-mergeable PR.

## SCOPE_FROZEN

```text
freeze_commit:
allowed_production_paths:
narrow_adapter_families:
immediate_reverse_consumers:
public_or_private_api_delta:
persisted_profile_disposition: BYTE_COMPATIBLE | VERSIONED_REBUILD | LEGACY_DECODE_ONLY | UNSUPPORTED_REQUIRES_MIGRATION | NOT_APPLICABLE
legacy_path_and_deletion_owner:
minimum_check:
minimum_clippy:
focused_fixtures:
out_of_scope_owners:
```

- [ ] Scope was frozen no later than the second source commit, or this is a non-source PR.
- [ ] For `SLICE`: this is the sole active source branch and sole open source PR; it starts from the accepted current `main`.
- [ ] No stacked or independent parallel source PR exists.
- [ ] One primary owner, at most two narrow adapter families.

## Causal result

Describe one result that becomes true after merge. Do not describe completion of the entire programme unless this PR genuinely delivers it.

## Changed ownership

| Path group | Owner | Why this slice may change it |
|---|---|---|
|  |  |  |

## Compatibility and deletion

State exact persisted-byte/profile behavior and which legacy production path is removed now, retained read-only, or assigned to a named deletion owner. No silent relabelling or product fallback.

## Findings

Every actual finding keeps exactly one disposition.

| Disposition | Path / symbol | Consequence | Owner issue | Blocks this PR? Why? |
|---|---|---|---|---|
| B0 / F1 / F2 / D / Q |  |  |  |  |

Use `NO_FINDING` only when a reviewed scope produced no finding. Do not collapse F1/F2/D/Q into a generic blocker.

## Review budget

```text
primary owners:
narrow adapter families:
changed production files:
changed production lines:
persisted migrations:
cross-owner cutovers:
```

- [ ] Within the default budget, or a reviewed exception was accepted before further source changes.
- [ ] If split, only tranche 1 is open; successors will branch from newly merged `main`.

## Source gates

Record exact command, toolchain/platform, exit and nonzero focused case count.

```text
check:
clippy:
reverse-consumer compilation:
focused fixtures:
source guards:
```

Known broad baseline debt:

```text
base diagnostic fingerprint:
final diagnostic fingerprint:
candidate delta:
owner issue for unchanged debt:
```

- [ ] No new candidate diagnostic is hidden by an already failing aggregate command.
- [ ] Full native/live/scale/release evidence is not claimed from source compilation.

## Exact-head review

```text
final_head_sha:
reviewer:
review disposition:
load-bearing changes after review: none
```

- [ ] Formal review targets the final head SHA.
- [ ] A security badge, signature, source guard or author comment is not represented as independent execution acceptance.

## Evidence boundary

State exactly what this PR proves and what remains source integration, native qualification, installed qualification or release work.
