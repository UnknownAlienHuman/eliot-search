# Implementation and review

Maintainer instructions and the [current architecture](../architecture/README.md) govern the work.
[#352](https://github.com/UnknownAlienHuman/eliot-search/issues/352) and the actual handoff identify the
active manager. Preserve that manager's worktree/private WIP. One writer integrates code and dependency
changes; subagents provide narrow research/review. Dated issue trains do not restart old branches.

## 1. Choose a useful result

Start with a real command, caller or concrete obsolete mechanism. Read its owner and immediate consumers.
Explain in the existing issue/PR what will work, what is removed, what persisted data is affected and
what verification resolves the actual risk. A prescribed form or approval to research is unnecessary.

An owner and its callers may change together when that completes and deletes an old path. Do not split
necessary integration into unused facade PRs to meet a line/file quota. Split unrelated capabilities
when they can ship independently. Finish a coherent change before opening a dependent source successor;
do not displace the existing manager with overlapping writers.

## 2. Remove obligations as well as code

General migration/rollback, six-domain residency, mandatory encryption, delegated local clients, durable
handles and qualification-as-runtime-admission must not enter every implementation by inertia. Apply the
current architecture decision, not a new exemption.

For stored data, say whether existing bytes remain readable or require a fresh format/generation.
Rebuild derived projections; preserve original/sole retained bytes, keys and user configuration. An old
format without a real retained-data/external consumer needs no online migration framework. Never silently
reinterpret identities or erase old roots. Record a specific export need when it exists.

Use the [reuse index](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md) and actual upstream code. Reuse
existing dependencies; inspect version/API/features when adopting a new one. A donor's mechanism does
not require copying its application. Replace the mechanism and remove its last old consumers.

## 3. Implement and review behavior

Complete the selected path before expanding optional capabilities. Review the integrated diff for
incorrect results, data loss, missing callers and duplication. Add related work only when the result
would otherwise be wrong; put independent remaining work in the existing issue.

There is no second-commit `SCOPE_FROZEN` ceremony, mandatory triage alphabet, review round count, prescribed
subagent count or self-submitted formal GitHub review requirement. Get independent review when it helps
resolve the change. New load-bearing code needs review; unchanged bytes need no fresh ritual. A security
badge is not a product test or a reason to manufacture more infrastructure.

## 4. Implement then verify the production closure

Use pinned Rust 1.98, locked dependencies and actual production targets. Usually:

```text
cargo +1.98.0 check --locked -p <owner> --lib
cargo +1.98.0 clippy --locked -p <owner> --lib -- -D warnings
```

Select affected binaries/features and immediate consumers as needed. Daemon `--all-features` also
enables old harnesses; it does not describe the supported application. The wave ladder is slated for
removal. Do not make optional lifecycle/proofs baseline dependencies through `full-baseline`. Check
still-supported features when relevant, not every historical combination.

After implementation, run focused behavior cases that can falsify its result. For integrated search:
real indexing/retrieval/readback, update/restart/index loss, cancellation and scope exclusion. Record
unexecuted native/Qdrant/scale cases honestly. A diagnostic run may explain a defect; broad repeated
suites do not compensate for missing integration.

If a relevant check fails, compare base and candidate once to determine whether the change caused it.
Do not hide new failures in old warning totals or expand shrinking work into unrelated lint cleanup.
Source-shape/token/file-size tests, source-hash ledgers, issue snapshots and committed coverage maps are
no longer merge gates. Preserve actual byte/identity/output behavior and real dependency/unsafe-code
checks; retire checks that only enforce the old layout/process.

## 5. Report what changed

Keep the existing PR body current: behavior delivered, mechanism removed, data disposition, meaningful
commands/outcomes and remaining limitations. Counts support reduction claims; they cannot replace caller
analysis. No parallel status registry, waiver, evidence ledger or progress comment is required.

Close implementation issues when their real result is delivered. Programmes may remain open for useful
remaining capabilities; optional work is not a prerequisite merely because an issue exists. Keep this
documentation preparation separate from the active source PR and Actions manual-only. Release/external
publication remain explicit actions. Never describe compilation or a mock as runtime success.
