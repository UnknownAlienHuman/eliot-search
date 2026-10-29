# search-subject-resolver — inspected functions and cutover

## Evidence boundary

At the established audit revision, `resolve_subject` had package unit-test callers and no production source/index
candidate producer connected through this API. Candidate steps, completeness and context/fence values were
supplied by callers. This is historical caller evidence, not current branch truth.

Before a code cut, repeat the exact current search across manifests, Rust imports/constructors/path inclusion,
non-Rust tooling, features, targets, tests and workflows. Record all surviving consumers and move them before
removing APIs or the package.

## Inspected behavior

| Function family | Actual behavior at the inspected revision |
|---|---|
| context/request validation | Checked supplied fields and disclosure/currentness flags; resolved no scope and read no source. |
| resolution ordering | Bounded and ordered supplied candidate steps into resolved/ambiguous/not-found/scope-empty/incomplete outcomes. |
| occurrence collapse | Grouped candidates by supplied hypothesis/equivalence data; established no alias/rename identity. |
| ambiguity construction | Sorted and bounded supplied candidate hypotheses. |
| receipt/revalidation | Copied and compared supplied context/fence metadata; produced no missing evidence. |

Do not implement historical logical function names merely to complete a contract.

## Connected replacement

Use an explicit source/revision/range when supplied and validate it against the pinned view and retained bytes.
For a cursor, use a containing definition only when actual mapped ranges identify one. A selected semantic
producer may nominate occurrences for the same snapshot. Otherwise bounded lexical/name retrieval nominates
possible definitions and preserves match basis, qualifiers, truncation and ambiguity.

A missing explicit location is not silently replaced. Mentions are not automatically definitions. Same names,
receipt presence or rename hypotheses do not establish identity. Missing or truncated coverage cannot prove
uniqueness or absence.

Keep useful ordering/ambiguity helpers as private query code. Remove mandatory ladder materialization,
receipt/revalidation machinery and duplicate candidate copies with their last callers. Reuse ordinary evidence
validation and `QueryWindow`.

## Verification after code

Run minimal Rust 1.98 check/Clippy for changed production consumers. Through real retained source verify explicit
location success/failure, cursor containment, duplicate names, mention versus definition, repository/configuration
distinctions, truncation and incomplete coverage.
