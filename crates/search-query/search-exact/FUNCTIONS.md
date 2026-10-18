# search-exact — current API and removal boundary

## Current implementation

| Source/API | Actual behavior |
|---|---|
| `literal.rs::scan_chunks` | Uses the private Aho-Corasick contiguous NFA over borrowed UTF-8 chunks. Handles overlaps, byte ranges, ASCII-only folding and first-extra-match truncation. Real DIRECT callers exist. |
| `proof.rs::compile_predicate` / `execute_predicate` | Build a supplied predicate profile and use a second `find_literal` loop; other predicates depend on a caller engine. |
| `freeze_denominator` / `freeze_spine_denominator` | Sort/check caller-supplied inventory values; neither enumerates source storage. |
| `compile_exact_scan` / `validate_plan_before_execution` | Assemble and compare caller plan/fence values. No source effect occurs. |
| `execute_item` | Compares supplied digest/length fields and scans already captured bytes; it does not open or hash the revision. |
| `execute_exact_scan` | Accepts `Vec<ExactItemReadback>` after all source bytes were allocated, then applies budgets too late to bound capture memory. |
| report/checkpoint/resume helpers | Validate supplied accounting and return remaining IDs; no durable read or scan is performed. |

Gross source inventory is approximately:

```text
literal.rs   9,275 bytes   useful matcher
proof.rs    64,553 bytes   proof/plan/checkpoint platform
spine.rs     8,724 bytes   second denominator representation
```

## Replacement flow

```text
resolved scope + pinned PublishedView
→ ordered exact source/revision inventory
→ compile one predicate
→ for each item:
     reserve bounded read
     open exact retained revision
     validate actual bytes/length/identity
     scan
     project exact ranges
     release buffer
→ EvidenceCard matches + item/byte accounting + gaps
→ terminal complete/incomplete classification
```

The source/view owner supplies the inventory and retained reader. Exact code owns predicate execution and
accounting only. It does not create access grants, handles, Qdrant candidates or a second catalogue.

## Remove

- `proof::find_literal` and duplicate semantics;
- public scan-plan/permit/reference types and compile/execute recipe split;
- supplied `InventoryCapture` / whole-corpus `ExactItemReadback` path;
- duplicate spine denominator;
- generic fence/plan/report digests and trust booleans;
- durable checkpoint/resume and per-item proof receipts;
- handle-specific output after `EvidenceCard` / `QueryWindow` cutover.

Retain only nontrivial matcher adapters, simple inventory-item outcomes and terminal completeness. If these
fit naturally in daemon query composition after cutover, do not preserve a package boundary for old tests.

## Matcher semantics

Use the same Aho core for literal execution. ASCII-insensitive mode folds ASCII letters and leaves other
bytes unchanged. Matcher state spans chunks of one source only. Input/match limits are charged during the
actual read/scan; output has a separate bound.

Regex is an optional single `regex-automata` adapter with frozen application options and finite compile
size. No convenience-regex/backtracking path, silent engine fallback or fake streaming. Multi-literal is
not baseline; add it only when one actual request needs it.

## Verification

Check one real `ExactRequest` through the daemon/source reader: exact historical revision, source removal or
missing bytes, chunk boundaries, overlaps, UTF-8/non-ASCII policy, match ceiling, input limit, cancellation,
complete no-match and incomplete-with-matches. Index top-k never satisfies completeness.
