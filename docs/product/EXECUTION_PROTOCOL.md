# Bounded implementation protocol

Maintainer instructions govern the task. [Root instructions](../../AGENTS.md) and Architecture Part I
preserve product boundaries. Current handoff, [#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352),
the actual issue/PR and current source determine work readiness; dated maps do not override them.
One manager writes and integrates. Subagents provide narrow read/research/review results.

## 1. Work items and readiness

| Class | Result |
|---|---|
| PROGRAM | Capability obligations in an issue/document; no implementation branch. |
| SLICE | One causal source result in one mergeable PR. |
| FOLLOW-UP | An out-of-scope defect with owner, path/symbol, consequence and dependency. |
| GATE | Evidence against exact candidate bytes; source fixes produce a new candidate. |

Programme closure is not a source-merge prerequisite. Required APIs/artifacts must actually exist.
PROCESS/DOCS PRs contain no product source, manifests, lockfile or workflows; any source-changing PR is
a SLICE regardless of label. Open PRs are merge candidates, never permanent tracking packets.

Before source edits, record once in the issue or PR body:

```text
class: SLICE
base_sha / branch:
primary_owner / state_or_effect:
allowed_production_paths / narrow_adapters:
immediate_reverse_consumers:
non_goals:
donor_or_spec_sources:
persisted_profile_decision:
replacement_or_deletion_boundary:
minimum_check / minimum_clippy / affected_guards:
deferred_test_cases / qualification_owner:
known_baseline_failures:
```

Name a real owner and exact paths, not `bins/eliot-searchd/src/**`. One owner plus its immediate consumers
may be changed together to cut over and remove a replaced implementation. This explicit closure
supersedes generic package-only restrictions for those paths. It does not include unrelated legacy
cleanup, new caller families or blanket feature activation.

For persisted bytes, digests, schema or generation changes, choose **BYTE_COMPATIBLE**,
**VERSIONED_REBUILD**, **LEGACY_DECODE_ONLY** or **UNSUPPORTED_REQUIRES_MIGRATION** before coding.
A retained decoder names the format, actual reader and deletion owner; it cannot mint new identities.

For donor extraction, add the exact donor head, selected paths/commits, excluded changes, transitive
symbols/callers and a reviewed extraction diff against current main. Old donor PRs are evidence, not
merge bases. Use the [reuse index](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md); verify only the
release/profile being adopted. Preserve required fixtures with their owning behavior.

## 2. Freeze and review budget

Publish `SCOPE_FROZEN` no later than the second source commit: ready fields above, exact base/head,
public/private API delta, retained legacy/deletion owner and out-of-scope owners. A later comment cannot
silently add another schema, lifecycle, recovery owner or caller family.

Default split triggers are one primary owner, at most two narrow adapter families, 30 changed production
files, 2,500 changed production lines, one persisted migration and one cross-owner cutover. Report
fixtures/goldens/generated records separately. These are review triggers, not permission to omit
correctness. Existing package size debt does not require forwarding crates or unrelated cleanup.

If the result exceeds that budget, stop adding scope and either record a reviewed justification before
further changes or define dependency-safe serial tranches. Open only the first source PR. Merge it,
publish new main, then create its successor from that main. No stacked source PRs. A necessary scope
change can instead return to the last frozen head and redefine the slice before continuing. After two
proposed expansions, split/defer rather than adding another continuation.

## 3. Finding triage

| Class | Disposition |
|---|---|
| B0 | Direct same-slice correctness/safety blocker: declared caller does not compile, false authority, data corruption, access/disclosure widening, or an unsafe path that cannot remain unavailable/fail-closed. Name exact path, symbol, caller and causal chain inside the frozen closure. |
| F1 | Same-owner follow-up; frozen result remains correct while affected behavior is unavailable, legacy-only or fail-closed. |
| F2 | Adjacent owner, integration phase or other capability; assign there, do not absorb it by default. |
| D | Unchanged baseline harness/lint/topology debt, with no new diagnostic/caller break/behavior regression. |
| Q | Native/live-Qdrant/fault/scale/installed/release evidence; route to #215/#264 or the named gate. |

Only a causal B0 inside the declared closure and review budget can enter a frozen slice. “Related to the
programme” is insufficient. New candidate production failures cannot hide behind an old failing aggregate.
Each subagent preserves these five dispositions or returns `NO_FINDING`; consolidate duplicates.

## 4. Implement, then verify the production closure

1. Preserve the active manager/WIP. Start the selected source branch from current `origin/main`.
2. Use one pre-code pass of relevant research, with no mandatory subagent count or repeated whole-repo
   audit. Manager independently checks load-bearing source claims and alone edits manifests/lockfile.
3. Implement the real owner/caller result first. Delete the replaced API/writer/fallback with its last
   consumers; preserve identified legacy readers. No dead facade, fake observation or exemption growth.
4. Check the changed production owner and immediate consumers; run strict Clippy and affected guards.
5. Review the integrated diff once, routing unrelated findings through the table above.
6. Obtain independent review bound to the final head; merge the coherent slice after its declared gates.
   Every successor starts from newly merged main. Read-only successor research may overlap final review.

For library owners the usual commands are:

```text
cargo +1.98.0 check --locked -p <owner> --lib --all-features
cargo +1.98.0 clippy --locked -p <owner> --lib --all-features -- -D warnings
```

Select actual production targets and declared consumers. For daemon wiring, check its normal defaults
and required `full-baseline` binary profile explicitly: `--all-features` also enables
`legacy-loopback-harness` and is not the normal product configuration. Do not use broad `--all-targets`
or full-workspace/test graphs as source gates.

Reuse recorded baseline debt under #346/#189. If a production gate fails, compare relevant base and
candidate diagnostics once. New production diagnostics block; identical unrelated debt is D. Do not
repair a broken historical harness merely to turn an aggregate command green.

Record the smallest causal unit/integration/native/crash/fault/resource cases in the deferred inventory.
Compile and execute tests after the complete integrated implementation, against its exact candidate.
A small diagnostic run to understand a defect is debugging evidence, not a merge PASS. Qualification
failures return to their source owner and a new candidate; do not weaken expectations after results.

For reduction work, report added/deleted production code, fixtures, generated output and remaining
callers separately. Moves and tiny forwarding modules do not count as eliminated behavior. A reused
library removes only the mechanism it actually replaces; profile, access and lifecycle owners remain.

## 5. Review, reporting and closure

The PR body is the single current status record: frozen scope; exact base/head; changed owners/paths;
delivered result; deleted/retained legacy; F1/F2/D/Q owners; exact commands/exits; candidate-versus-base
diagnostics; deferred cases and evidence limits. Update at stable checkpoints, not a long comment per
commit. Never claim execution at a revision where it did not occur.

Before merge, a formal GitHub review must target the final head with no unresolved B0/P1 and explicit
other dispositions. A load-bearing commit expires earlier review. In a single-owner repository, a
formal COMMENT submission may record independent reviewer evidence and manager disposition; it is not
self-approval or an independent reviewer identity by itself. A security badge alone is insufficient.

Close an implementation issue when its declared source result is merged and residual obligations have
current owners. A programme can remain open across those deliveries. Historical tracking/gate PRs stay
closed after their useful obligations are preserved in issues/docs; never branch from them or create a
replacement tracking PR. In particular, closed unmerged #344 is a donor at
`6d698eafa573e7a8a5eb9dca6ce17ae7878c3bce`; its old #351 → #354 → #235.core train is not today's queue.

Keep Actions manual-only and do not commit local paths, credentials, source captures or unredacted logs.
Compilation, source presence, mock results and issue/review status do not establish runtime acceptance.
Exact native/Qdrant/recovery/scale/installed qualification and explicit publication remain separate.
