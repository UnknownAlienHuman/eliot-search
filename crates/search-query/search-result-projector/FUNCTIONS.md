# search-result-projector — current API and cutover

## Current API

`project_candidate_set(envelope, inputs, budget)` currently:

- consumes a complete `Vec<CandidateProjectionInput>`;
- re-sorts it by raw score;
- receives already verified source bytes and an already minted `SearchSourceHandle`;
- applies candidate/source-byte limits only after that earlier work;
- returns the old handle-oriented `SearchCandidateSet`;
- copies caller coverage while growing one omission record per rejected input.

`verify_handle_target_binding` compares three supplied IDs. It neither resolves the handle nor proves that
the public metadata belongs to the candidate.

Source: [lib.rs](src/lib.rs).

## Required change

1. Retrieval/fusion establishes final order.
2. Query composition selects one bounded page before retained-source I/O.
3. Candidate validation resolves `ViewManifestEntry` and verifies actual retained bytes/ranges.
4. Projection receives `VerifiedEvidenceItem` values in final order and formats `EvidenceCard` values.
5. The query window owns the cursor, view guard and optional `(window_id, item_index)` locator.
6. Bound encoded output and sampled gap details; report total omitted counts without storing one record per
   omitted candidate.

Remove `CandidatePublicMetadata`, mandatory `SearchSourceHandle`, raw-score sorting and handle-binding
helpers with their consumers. If no nontrivial transformation remains, fold the code into the query owner
and delete this package rather than maintain another lifecycle boundary.
