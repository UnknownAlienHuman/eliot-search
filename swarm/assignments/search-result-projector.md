# `search-result-projector` implementation packet

**Path:** `crates/search-query/search-result-projector`  
**Capability:** C26  
**Delivery:** W4 / P08  
**Gate:** BLOCKED until validator, handle and exact result contracts are accepted  
**Trace:** S23.2-S23.3, S26, H15.4, P08

## Mission

Project validated candidates and explicit coverage gaps into compact bounded recipe results while
delegating mutable handle state/authorization to `search-handles`.

## Owns

- `SearchCandidateSet` and generic result-card shaping;
- response byte/source/variant budgets;
- coverage/freshness/validation-gap projection;
- deterministic selection of handle subjects;
- handle creation requests through accepted port.

## Must not own

- unvalidated candidates, raw full files or unbounded chunks;
- vendor metadata or client belief/admission;
- hidden omitted/failed legs or validation gaps;
- handle records, expansion authorization or revocation.

## Logical operations

1. `project_candidate_set(validated, gaps, execution, budget, handles) -> Result<SearchCandidateSet, ProjectError>`
2. `select_handle_subjects(validated, quotas) -> BoundedList<HandleSubject>`
3. `project_coverage(execution, gaps) -> Coverage`
4. `enforce_result_budget(result, budget) -> Result<ProjectedResult, ProjectError>`

## Invariants

- only `ValidatedSearchCandidate` enters `candidates`;
- stale/unreadable/access/purge outcomes remain non-evidence gaps;
- default card contains bounded 2–4 recommended exact handles;
- `complete_scope` requires exact report;
- every result binds plan/view/owner/security generations;
- deterministic truncation records omitted material;
- projector has no handle table or expansion path.

## Exit evidence

- golden compact card and exact recipe-result tag;
- invalid candidate type cannot be passed to projection;
- validation gaps contain no evidence excerpt;
- full-file dump rejected;
- complete-scope overclaim rejected;
- security/result fence fixture;
- deterministic budget truncation;
- no mutable handle state.

Target `src/` ≤7,000 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
