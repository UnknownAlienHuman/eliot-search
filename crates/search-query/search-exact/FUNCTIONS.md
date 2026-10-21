# Exact search: one predicate, stated input domain, real reads

The [query contract](../README.md) owns user-visible intent. Exact execution uses one pinned source inventory,
not a Qdrant top-k denominator. The matcher is a library mechanism, not an authority or job service.

## What the current code actually supplies

At `c8e38b6`, [literal.rs](src/literal.rs) correctly preserves overlapping matches and automaton state
across its supplied UTF-8 chunks. ASCII-insensitive mode leaves non-ASCII bytes unchanged. Keep those
semantics. But `scan_chunks(&[&str], ...)` builds an NFA on every call, takes already available chunk
references and has no deadline/cancellation check in the byte loop. It is not a compile-once I/O scanner.

`proof.rs` supplies a second literal loop, caller-made plans/readback vectors and report/checkpoint types;
`spine.rs` supplies a second denominator. Neither substitutes for reading actual retained data. The previous
map's phrase 'use the same Aho core' did not specify the required change to its real consumer.

## Input semantics are part of the request

Retain the useful distinction from old architecture S25, without its public compiled-plan protocol:

| Selected domain | What is scanned | What a no-match can establish |
|---|---|---|
| Raw bytes | Exact retained source bytes; a text literal is encoded as the request's specified bytes | That byte pattern was absent from the stated fully scanned revisions |
| Decoded text | One explicitly named supported decoding/normalization representation | Absence in that representation, not automatically in the original bytes |
| Extracted document text | A supported retained derived representation with extraction coverage and coordinates | Absence in extracted text only; missing/OCR/unsupported regions remain gaps |

Raw-byte literal search is the minimal baseline. Do not implicitly decode UTF-16 as UTF-8 or silently remove
newlines/normalize before a request called raw. A UTF-16 file containing the characters `foo` does not
contain the three UTF-8 bytes `66 6f 6f` contiguously. A raw no-match and a decoded-text match can both be
correct. Responses name their domain/representation so agents cannot turn one into the other.

Decoded/extracted modes are exposed only when their real representation reader exists. Unsupported input
or mapping is a gap/error, not an empty successful scan. ASCII-insensitive is not Unicode case folding.
No-match over retained published revisions is not a claim about the unobserved live working tree.

## One execution

```text
resolve scope and its pinned source/revision inventory
-> validate predicate, domain and finite limits even for an empty scope
-> compile the private matcher once
-> for each inventory item:
     open the exact retained object/selected representation
     reset matcher state and offsets at the source boundary
     consume bounded blocks, verify the object and collect bounded match ranges
     map ranges only through the selected actual representation
     emit verified evidence or an explicit item gap
     release item-local buffers
-> report scanned inventory, input/output limits, gaps and terminal status
```

The compiled matcher and block-feed state stay private to this request. No public compile/execute API,
serialized automaton, job ID, capability, lease or registry is introduced.

For the single-literal baseline, the already-pinned Aho-Corasick 1.1.5 `Automaton` interface supports the
existing standard-match state transition loop over bytes. Reuse the NFA for all items; reset its state per
source, not per I/O block. An arbitrary byte block may split a UTF-8 codepoint, so the raw scanner must not
require each block to be a valid `&str`. Preserve overlap and absolute offsets without concatenating the
corpus. A future decoded mode uses its actual incremental decoder or explicitly bounded whole-source read.

Check cancellation/deadline between bounded blocks and before more I/O/output. Charge input as consumed,
not merely after a complete `Vec<ExactItemReadback>` was allocated. Pattern construction has its own small
input/compiled-size ceiling. Work is linear in scanned bytes plus matches, with one pattern build; storage
reader and decoding/integrity costs are additional, not hidden by matcher complexity.

## Integrity, memory and output

A match is not emitted as verified source evidence before the retained reader's actual integrity condition
is satisfied. A whole-object hash or single AEAD envelope may require reading/authenticating the whole object
before release. Account that real input/buffer cost; do not describe a few displayed bytes as the total read.
If #330 does not provide authenticated incremental release, use one bounded object at a time and state that
limit. Do not invent Merkle pages or another encryption format just to make a streaming claim.

Match/output limits are distinct from scanned-input limits. The existing first-extra-match truncation rule
may be retained; reaching a ceiling does not prove exhaustive matches. A successfully completed scan with
matches and truncated presentation is distinguishable from an aborted scan. A complete negative requires
zero matches, all selected domain items verified and scanned, and no missing/unsupported/limit/cancel gap.
An empty selected inventory still validates the request and is reported as empty, never widened to all.

Interactive result paging may reuse the query window for retained match references, but cannot silently
turn an unfinished full scan into a complete one. No durable exact-job/checkpoint lifecycle is required.

## Regex and other predicates

A real regex request may use one bounded `regex-automata` configuration. Its domain, UTF-8/byte behavior,
newline and case options are explicit. Do not assume a regex can be evaluated independently on blocks;
anchors and cross-block matches require supported engine semantics. Refuse unsupported combinations rather
than claim complete absence. Structured-name resolution belongs to actual syntax/semantic facts and is not
inferred from a literal occurrence or BM25 result.

## Delete with consumers

Delete `proof::find_literal`, supplied scan plans/readback/inventory proofs, duplicate spine, canonical
plan/report digests, permits, durable resume/checkpoint and handle minting. Move only the useful matcher and
accounting beside the real query/source consumer if no substantial package remains. Keep causal tests,
not old API/byte spelling.

## Implementation checks, not a new gate system

After code and minimal changed-owner Clippy: compile once across several sources; cross-block overlaps;
no cross-source match; byte chunks splitting UTF-8; raw versus decoded UTF-16; CRLF/normalization distinction;
whole-object integrity failure; no-match with one unsupported item; first-extra-match truncation; pattern,
input and output ceilings; cancellation during a large source and after EOF. This document records required
behavior; it does not claim these checks or a streaming reader already ran.
