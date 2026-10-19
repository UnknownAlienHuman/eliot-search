# search-eval — current source and reduction map

## Current inventory

The package is evaluation-only and has no external Rust caller in public main. Production source is roughly
150 KiB and already models most of a qualification product:

| Area | Current behavior | Disposition |
|---|---|---|
| `core/*` | corpus, registry, policy, run and baseline models | replace with small explicit case/result/qrel records |
| `evidence/*` | attempts, schedules, probes and ledger | delete; raw runner rows are enough |
| `audits/*` | admission/fault/leakage/protocol/reproducibility audit engines | delete from generic package; #215 describes concrete scenarios |
| `metrics/*` | scores, comparison, resources, SLO and support | keep only standard metrics/raw-duration summaries actually used |
| `report/*` | report assembly, coverage, receipts, review and verdict authority | delete; reports are descriptive serialization |
| `fingerprint.rs` | package-local identity/fingerprint | delete; no evaluation identity authority needed |

## Minimal input

A small evaluator may use Serde records such as:

```text
EvalCase {
  case_id
  query / operation
  selected source IDs or exact corpus label
  expected limit/profile label
}

Qrel {
  case_id
  exact source/evidence identity
  grade
}

CaseResult {
  case_id
  status
  ordered evidence identities
  gaps / partial state
  raw integer durations
  optional explicitly named raw counters
}
```

Bounds are applied before retaining arrays/strings. Case IDs are unique. Missing/duplicate/foreign rows are
errors. Evaluation never opens live product state or reinterprets a source path/title as evidence.

## Minimal operations

```text
read_cases(path, limits)
read_qrels(path, limits)
read_results(path, limits)
validate_denominator(cases, results)
compute_relevance(results, qrels, k)
compute_status_counts(results)
compute_raw_duration_summary(results)
compare_compatible_runs(left, right)
write_summary(summary, output_limit)
```

Compatibility is explicit and small: the same case set, corpus/scope label, retrieval profile and metric
configuration. Incompatible runs report why and produce no misleading delta.

Metrics use straightforward checked arithmetic and documented formulas. Keep raw ranked results and raw
integer durations; no histogram-only evidence, hidden rounding profile or single blended score.

## Runner

A runner is separate effectful tooling and may be a small command/script. It uses the installed public
CLI/protocol, not private query/Qdrant/redb APIs. It records every declared case including failure/timeout/
cancel/not-run. It may retain exact commands, candidate identifiers, environment summary and raw outputs under
an explicit directory.

Do not add:

- `search-eval-runner` framework by default;
- canonical run IDs or immutable artifact store;
- in-toto/DSSE statement/predicate/reviewer packages;
- recursive prerequisite verifier;
- Windows Job/process evidence ontology;
- fault executor, disclosure scanner or before/after inventory graph;
- attestation references consumed by product runtime.

A selected release may add a narrow helper for one measured need, then keep it outside product dependencies.

## Removal sequence

1. Confirm no product package depends on `search-eval`.
2. Select the concrete #215/#264 cases and file format actually needed.
3. Keep/rewrite only small parsing and standard metric functions.
4. Delete audits/evidence/report-receipt/review/SLO/fingerprint modules and their tests.
5. Delete #233/#234/#240 framework dependencies/docs and status entries.
6. If no release workflow consumes the remaining Rust library, delete the package and implement the small
   evaluator in existing release tooling instead.

## Verification

- explicit finite files and no live product reads;
- every planned case represented once;
- failures/partials/not-run remain in summaries;
- hand-computable standard metrics;
- raw duration summary reproducible from retained samples;
- incompatible run comparison refuses;
- no source/query/credential content leaks into ordinary summaries beyond explicitly selected evidence.
