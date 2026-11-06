# Preparation: exact retained UTF-8 to one owned product

The current public transform is [materialize_utf8](src/utf8/materialize.rs). It consumes verified retained
bytes into one unchanged UTF-8 String with an attached exact line inventory. The actual P1 consumer is
the daemon's prepare-source operation through [search-unitizer](../search-unitizer/FUNCTIONS.md).
Source reads and object writes remain with the shared object owner; CONTROL owns conditional installation.

## First connected format

The selected transform preserves every UTF-8 byte, including BOM, CRLF, CR, LF and final unterminated
text. It does not normalize or execute input. Empty input succeeds with zero lines. NUL or the existing
bounded C0-control density policy returns BinaryContent; malformed UTF-8 returns InvalidUtf8.
These distinct content refusals can become current Gaps bound to the exact input and selected settings.

The same owned MaterializedText goes to unitization. Its private fields prevent attaching a caller-built
line inventory to other text. There is no source reread, copied per-unit text or normalization buffer.

## Enforce the execution limit before acquisition

The daemon must check the retained ObjectRef's declared length against the operation ceiling before
the shared reader allocates or hashes. The reader verifies that exact object; mutable source paths are
not preparation inputs. This transform checks actual input/output lengths before UTF-8 conversion and
line allocation. Nonzero line ceilings and allocation byte arithmetic are validated before growth.

The original absolute deadline and cancellation flag pass through the transform and unitizer. Binary
classification and line scans check them during bounded chunks, and line allocation uses fallible
bounded growth. Standard UTF-8 validation is a bounded library pass checked before and after it; it
does not receive a reset deadline. Cancellation/deadline are transient operation failures, not permanent
unsupported input.

## Separate reuse, installation and retry decisions

Output identity contains exact UTF-8/line-algorithm versions and preferred/hard chunk settings, together
with source namespace/identity/registration and the exact retained input ObjectRef. Fail-only execution
ceilings are excluded. Equal bytes in another source do not authorize reuse of that source's binding.

The daemon captures source row/registration/input, selected settings and their non-reset revision, plus
the expected current outcome value/revision. It reads/transforms/unitizes/writes outside a writer, then
compares all captured state in one short CONTROL transaction. A stale task never regains installation
rights after ABA/remove-add. A failed stale attempt cannot downgrade a usable current result.

A new operation may reuse still-correct pure output under a newly captured guard. A relaxed execution
ceiling can permit a retry; unchanged permanent failures must not busy-loop. Queue loss recovery belongs
to later bounded reconciliation, not a new preparation job service.

## Prepared artifact acceptance

[The complete artifact codec](../search-unitizer/src/prepared.rs) stores actual unchanged representation
bytes and ordered descriptors once. Its input ObjectRef is encoded; its own subsequently returned
ObjectRef is not. The shared store publishes/read-backs these bytes before CONTROL records Ready.

Saved acceptance verifies the Prepared ObjectRef first, then framing, expected source/input/registration/
settings, representation length/digest and bounded valid ranges. It consumes saved data directly instead
of rerunning materialization or unitization. Missing/incompatible/corrupt data remains unavailable.
An explicit prepare operation may rebuild from exact retained input under the ordinary current guard;
query/evidence reads must not silently substitute live source bytes or secretly reprepare.

## Successful empty is not failed extraction

Complete supported empty text yields a real prepared object with zero units. Binary/invalid UTF-8 and
selected fail-only limit refusals remain explicit bound Gaps. Invalid settings/limits, corruption,
allocation/overflow and cancellation/deadline retain their actual operation errors. No sentinel unit/vector
or success-shaped partial output is created.

<a id="inspected-source-useful-work-and-actual-defects"></a>
## Removed source routes

The receipt-bound materialize/RetainedRevision/MaterializationReceipt route and dormant product,
request/profile, decode/normalize/maps, Markdown/JATS, provider and legacy preparation-store families
are physically removed with their exclusive fixtures. Their former source inspections are historical,
not reasons to preserve compatibility wrappers. Pure exact UTF-8 and line behavior cases remain.

<a id="markdown-donor-correction"></a>
Other format donors are reconsidered only with a selected real retained-input/evidence consumer.

## Verification

Use the minimal locked owner/real-consumer check and strict Clippy after James's lock-resolution handoff.
The focused behavior cases cover original-allocation ownership, exact Unicode/line coordinates, successful
empty, distinct content refusals and execution failure. Actual daemon/object/CONTROL runtime qualification
is recorded by its designated integrator/verifier; this recipe alone makes no runtime-pass claim.