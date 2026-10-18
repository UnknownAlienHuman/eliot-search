# search-exact functions

## Existing functions and callers

| Source/API | Actual behavior |
|---|---|
| [literal.rs](src/literal.rs): `scan_chunks`, `validate_query` | Validate finite UTF-8 chunks/query and execute the private Aho-Corasick NFA. Byte coordinates, overlaps and first-extra-match truncation are implemented. Daemon DIRECT callers exist. |
| [proof.rs](src/proof.rs): `validate_predicate_profile`, `compile_predicate`, `execute_predicate` | Validate supplied profile/serialized predicate; literal execution uses the separate private `find_literal`. Other predicates require a supplied `QualifiedPredicateEngine`. |
| `freeze_denominator` | Sort/check an already supplied `InventoryCapture`; it does not enumerate source storage. |
| `compile_exact_scan`, `validate_plan_before_execution` | Assemble a plan from supplied identity, predicate and denominator, then compare supplied live-state fences. |
| `execute_item` | Scan an already captured `ExactItemReadback` using an optional engine and `ExactMatchProjector`. It compares the supplied observed digest/length; it does not open the revision or hash its bytes. |
| `execute_exact_scan` | Consume `Vec<ExactItemReadback>`, build a revision map, apply item/byte/match bounds and record per-item outcomes. The input bytes have already been allocated. |
| `assemble_execution_report`, `classify_completeness`, `verify_execution_report` | Check supplied item accounting and classify it. These functions cannot establish that an external inventory or readback assertion is true. |
| `checkpoint_execution`, `resume_execution`, `revalidate_complete_negative` | Bind/check supplied checkpoint or live-state metadata. `resume_execution` returns remaining revision IDs; it neither reads nor scans them. |
| [spine.rs](src/spine.rs): `freeze_spine_denominator`, `classify_spine_completeness` | Pure bounded item-list/completeness helpers; they do not provide another source reader. |

The proof/spine entry points currently have package-test callers. They do not demonstrate a connected
corpus scan. Change the existing scan owner and its actual daemon callers together; the package split
and old logical signatures are not constraints on that integration.

## Remove the second matcher

`proof::find_literal` loops over every possible start and compares a query-sized window. Its
`max_steps` counts starts, not the individual byte comparisons. A long repeated-prefix input can do
substantially more work than that counter suggests. `literal::scan_chunks` already uses the donor
state machine and carries state across chunks.

Use that same Aho core for both explicit scanning callers, compiled once per predicate. Keep matching
state within one source; reset between sources so a suffix of one file cannot match a prefix of another.
Generalize the existing private byte matcher where raw-byte input is needed, preserving text decoding
and source-coordinate mapping at the caller. `scan_chunks` currently accepts `&str`; do not pretend it
already accepts arbitrary raw bytes.

Unify ASCII-insensitive semantics: ASCII letters fold, other bytes remain unchanged. Currently
`proof::compile_predicate` rejects non-ASCII patterns and `find_literal` rejects non-ASCII input, while
`literal::scan_chunks` permits both. For example, ASCII `HELLO` in `α hello` can match through the
literal primitive but the proof matcher rejects the input. This is a deliberate semantic correction,
not a compatibility mode or another predicate version to run alongside the old one.

## Read the inventory as work advances

The target flow is selected source inventory/revisions → one compiled predicate → bounded retained
read → matcher → exact range/result accounting. Use existing source storage and validation. The
current `InventoryCapture` and `ExactItemReadback` APIs need to change with those consumers.

Read one source or a bounded batch after reserving its allowed bytes. Verify the actual returned bytes
against the retained revision, scan, project ranges and release the read buffer. Check cancellation and
the operation deadline between bounded reads/scanning chunks; a preloaded whole-corpus vector defeats
the read budget. Returned matches/output are bounded separately from input and scanned-byte accounting.

One frozen item list plus completed/failed/unread accounting is enough for ordinary exact execution.
Keep source/revision/range and actual predicate/encoding semantics. Remove duplicate denominator
encodings, generic fence digests, receipt-only trust checks and unused durable checkpoint machinery
with their last consumers. Apply actual scope/source restrictions for the selected client mode;
removing grant metadata is not permission to scan outside that scope.

Completeness requires every item of the stated inventory to have been evaluated on its specified
revision. An input limit, missing revision, unreadable encoding, interrupted scan or incomplete inventory
prevents complete-no-match. Report available matches with their remaining gaps. Newer files at the same
paths cannot silently replace frozen revisions. Empty or truncated index/provider results never stand
in for this scan.

Keep regex in the existing selected donor implementation and structural predicates tied to actual
prepared syntax data. Do not create an engine/qualification registry merely because the old enum lists
more predicates. The [README](README.md#one-exact-path) owns the result semantics and exclusions.
