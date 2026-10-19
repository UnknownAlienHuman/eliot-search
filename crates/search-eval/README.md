# search-eval — optional post-product evaluator

Evaluation compares actual public product results after the connected application works. It is tooling, not a
runtime owner, release credential, evidence database or prerequisite for implementation.

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and release issue
[#215](https://github.com/UnknownAlienHuman/eliot-search/issues/215).

## Verified current state

The package has no external Rust caller in public main. Its production source is roughly **150 KiB** across:

- corpus/baseline/run registries;
- evidence attempts, ledgers and schedules;
- protocol/fault/leakage/reproducibility audits;
- metric/SLO/resource/comparison machinery;
- report, verdict, receipt and independent-review models;
- a package-local fingerprint implementation.

Issues #233/#234/#240 proposed another immutable run store, canonical identity hierarchy, in-toto/DSSE
attestation system, recursive verifier, reviewer quorum, Windows Job sampler, disclosure scanner and fault
runner. That is a second product. It must not precede or gate Search.

## Small useful evaluator

After #264 proves the real CLI path, a pure tool may consume explicit files:

```text
cases.jsonl
  case_id, query, selected sources, requested limit/profile

qrels.jsonl              // optional when relevance is being measured
  case_id, exact expected source/evidence ID, grade

results.jsonl
  case_id, terminal status, ordered EvidenceCard identities,
  truthful gaps/coverage, raw integer durations and optional bounded resource observations
```

It may compute:

- case status counts (`PASS/FAIL/PARTIAL/NOT_RUN` are descriptive test outcomes, not runtime credentials);
- Recall/Precision/MRR/nDCG at declared `k` where qrels exist;
- per-case and aggregate latency from retained raw integer durations;
- simple before/after differences between compatible runs;
- explicit missing/invalid/mismatched case rows.

Use exact source/revision/range identities already returned by the current product. Path/title/chunk text is
not evidence identity. Failed/partial/timeout/cancelled cases remain present. No metric turns a correctness,
scope or corruption failure into success.

## Runner boundary

The smallest release runner is an ordinary tooling command or script which:

1. starts/opens the exact built candidate through the same public CLI/protocol;
2. executes a finite declared case list;
3. records raw request/result/duration/exit data;
4. optionally records a few clearly named OS process counters;
5. exits nonzero or leaves explicit failure rows on missing cases.

It does not need a new workspace package, process identity ontology, Job evidence store, histogram framework,
fault DSL, inventory graph or immutable publication service. For selected Windows resource testing, use the
existing process/Job owner or a small dedicated helper and retain raw integer samples; descriptive reports
are enough.

## Release evidence

A release may archive:

- exact source/build/package identifiers;
- commands/configuration/corpus and raw results;
- evaluator version and generated summary;
- ordinary package signatures/checksums when the release process selects them.

These artifacts are records for humans and reproducibility. They are not passed back into product
constructors, do not authorize readiness and need no in-toto/DSSE/reviewer-quorum operating system.
Independent review is a normal review/release process, not a public Rust contract.

## Remove or simplify

Delete after current consumers are confirmed absent:

- evidence ledger/schedule/attempt authority;
- package-local fingerprint/canonical identity system;
- report receipts, verdict authority and independent-review records;
- generic audit registry and SLO/acceptance policy engine;
- runtime/package/profile attestation schemas;
- fixtures preserving those pre-release APIs.

Keep only the small pure input validation and standard metric functions which #215/#264 actually consume.
If the remaining tool is simpler outside Rust, delete the package entirely rather than preserving a crate for
status purposes.

## Completion

This package never blocks implementation. It is complete when one actual post-product result set can be
validated and summarized deterministically from explicit finite files, with failures and missing cases still
visible. Product truth comes from the connected run and raw evidence, not an evaluator receipt.
