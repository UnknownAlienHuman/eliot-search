# search-exact

An explicit exact scan evaluates a stated predicate over a stated set of retained source revisions.
Its result says what matched, what was actually scanned and why anything remained unread. Follow the
[architecture](../../../docs/architecture/README.md) and
[implementation](../../../docs/implementation/README.md).

## Current source

[literal.rs](src/literal.rs) implements `scan_chunks` with the private Aho-Corasick 1.1.5 contiguous
NFA. It handles borrowed UTF-8 chunks, overlapping byte ranges, ASCII-only case folding and truncation
at the first extra match. Daemon source callers exist in
[DIRECT layout scanning](../../../bins/eliot-searchd/src/direct_preparation/layout.rs) and
[development scanning](../../../bins/eliot-searchd/src/development/kernel/scan.rs).

[proof.rs](src/proof.rs) implements plans, supplied inventory/readback processing, reports and
checkpoints. Its scan entry point currently has test callers, not a connected authoritative inventory
reader. It also contains a second literal matcher: `find_literal` compares each possible byte window.
In ASCII-insensitive mode it rejects any non-ASCII input; `literal::scan_chunks` accepts Unicode text
while folding only ASCII. A label saying the predicates agree does not remove this difference.

`execute_exact_scan` takes `Vec<ExactItemReadback>`, with every supplied item's bytes already allocated.
Its later item/byte budget cannot bound the memory used to capture that vector.
[FUNCTIONS.md](FUNCTIONS.md) separates these actual APIs from the integration work.

## One exact path

Use one donor matcher core and one loop over the frozen source inventory. Compile a literal matcher
once, preserve state across chunks of one source, and reset it between source revisions. Reuse the
existing Aho machinery; remove `proof::find_literal` and its conflicting semantics with the last caller.
Keep raw-byte and decoded-text modes explicit instead of casting arbitrary bytes to UTF-8.

Resolve the selected inventory through the existing source owner, then reopen and verify each retained
revision as it is scanned. Bound reads before allocation and release each source before advancing.
Preserve actual source/range checks and selected restricted-client policy. Replace supplied trust
booleans, per-record proof receipts and the preloaded readback vector; no additional storage or durable
verification-job platform is needed for an interactive scan.

Report complete-with-matches, complete-no-match, incomplete or invalidated execution. Matches remain
useful in an incomplete result, but finding one does not certify that every match was returned.
A complete negative requires a complete inventory, the exact requested revisions, successful predicate
execution on every item and no unaccounted omission. Cancellation, limits, unsupported input or missing
revisions remain explicit. A result for a retained historical inventory does not prove a current-tree
claim after that tree changes.

Ranked candidates, an empty SCIP/LSP lookup and lexical/semantic similarity do not prove source-wide
absence. Regex or syntax checks are selected extensions with concrete semantics and actual producers;
none establishes the absence of an arbitrary equivalent implementation.
