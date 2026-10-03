# Bounded native issuance prerequisites — 2026-10-03

The integration source at `166a4a2f59cc21998293f50071efa774220c4501` passed its
three focused targets: **17/17 tests, exit 0**. Independent Luna source review of that exact commit
returned `ACCEPTABLE_FOR_BOUNDED_INTEGRATION_PUBLICATION`. This is integration-tool publication,
not package acceptance, profile qualification or issued authority. No product package was edited.

## Accepted changes

- Explicit first-four instance profiles bind `MATERIALIZED`, `ISSUED`, `LEASED` and `RECORDED` to
  their exact record kinds. The control-plane registry is version 4, still schema-only with zero issued
  records. Status is distinct from orchestration state and current authority.
- A bounded read-only Rust validator loads the exact registry, four descriptors and four profiles
  (288 KiB maximum combined input). It rejects missing/duplicate/mismatched bindings, profile placeholders
  and unknown profile keys, and requires a root string `canonical_field_order` with exactly one `status`.
  It reports `NON_AUTHORITATIVE` and does not validate complete records, actors or signatures.
- The lease-event descriptor's unchanged canonical order moved before `[event_reason_codes]`. This
  repairs TOML scope while preserving schema version 1, reason mappings, fields, rules and instance order.
- The additive W0 selector supports only `swarm/modules/w0.toml::package[name=<package>]`. Its document
  must have the actual header `schema_version=1`, `project="eliot-search"`, `earliest_wave=0`; caller
  package equality and exactly one matching row are required. The builder loads it from the same immutable
  Git tree. `SelectorDocs`, the original resolver and historical fixtures remain unchanged.
- Two stale orchestration checks and their two fixture literals now require the existing version 6.
  This is separate from control-plane schema version 4.

The integration decisions are recorded in the
[partial instance-closure decision](../../handoff/changes/2026-10-03-control-record-instance-closure.md)
and [W0 selector correction](../../handoff/changes/2026-10-03-w0-module-selector.md).
Nested enum bindings and authoritative mutation remain unaccepted.

## Executed checks and preserved failures

Command at the exact final source:

```text
cargo +1.98.0 test --locked --offline --no-fail-fast -p xtask --test control_record_instance_profiles --test ticket_planner_w0_module_selector --test ticket_issuance_builder
```

| Target | Final outcome |
| --- | --- |
| First-four instance profile structure | 8/8 passed |
| Actual-repository issuance builder | 3/3 passed |
| W0 selector, including the checked-in packet | 6/6 passed |

The [final result](evidence/w0-issuance-prerequisites/combined-w0-integration-final.result.json)
binds source commit, command, clean Git status, time, exit and captured stream hashes. The retained
stderr includes 11 existing xtask warnings. No broad formatter, full workspace suite, production build
or Qdrant bridge run was repeated after these checks.

Earlier attempts remain visible rather than being replaced by the final pass:

1. The worker produced eight profile test captures: five exit-101 results and three exit-0 results,
   including its final 7/7 candidate run. The older stdout/stderr/exit triplets do not independently
   establish a source commit; source identity for them is worker-reported. Two earlier builder captures
   failed with 1/3 passing tests before the independently diagnosed version/selector corrections.
2. Integrated commit `c612a2f8b3e82aa123e71c904d7acbafaa456b5d` yielded 7/8 profile tests. An old test
   mutation searched for an LF-only line ending in a normal Windows CRLF checkout. The other targets did
   not run. Commit `e49e9d675f3ad21942b9b29def37c56d33dcfdcc` changed only that mutation needle, preserving
   copied fixture bytes instead of normalizing them.
3. The e49 run yielded profiles 8/8 and builder 1/3; the selector target did not run. Independent audit
   found that the new resolver and synthetic fixture incorrectly assumed a `stage` field. A read-only
   e49 CLI diagnostic confirmed selector-02 as the sole unexpected repository failure. Its explicit
   base-only input also deliberately produced `PARTIAL_ISSUANCE_SELECTION`; that is recorded separately.
4. The final header correction uses the real packet and adds actual-packet coverage. The final run above
   executes all three targets and passes. Further test repetition is not needed for unchanged source.

The [capture manifest](evidence/w0-issuance-prerequisites/capture-manifest.json) preserves every copied
stream byte-for-byte with SHA-256 and retains each original result JSON where available. Evidence text
conversion is disabled by the enclosing `.gitattributes`.

One final structural PowerShell invocation also passed at source 166a4a2: 47 types, 8 records, 211 fields,
9 signature refs, 28 manual workflows and **0 issued records**. Its
[result](evidence/w0-issuance-prerequisites/combined-w0-structural-contracts.result.json) is structural
validation only; it does not execute the 64 issuance-qualification probes.

## Remaining standalone work

The [ticket-obligation class proposal](../../handoff/changes/2026-10-03-ticket-obligation-classes.md)
was independently accepted for proposal publication at author commit
`0fd970ad3dd7d68b4a81db409ec3708126ca74ba`. It remains unbound and non-normative. It preserves actual
normal exits on expectation mismatch and records the unresolved command-to-unavailable-check link,
exactly-once command coverage and other field-specific closure gaps.

Artifact/approval profile registration and provisioning, actual signature/store verification, durable
operation receipts, acknowledgement encoding and independently executed qualification are still
required before native issuance can be enabled. The Search daemon/CLI and native Qdrant bridge retain
their earlier build blockers. The [local repository study](STANDALONE_STUDY.md) has a preserved ordinary
search baseline; actual Eliot Search retrieval remains unavailable. Governor and Memory OS are not
standalone runtime prerequisites.

This Markdown is an integration progress record. It is not an `independent_review_v1`, package handoff,
gate/wave receipt, qualification result or launch-state change.
