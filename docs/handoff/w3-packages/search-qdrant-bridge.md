# W3 package checkpoints — `search-qdrant-bridge`

**Write scope:** `crates/search-index-qdrant/search-qdrant-bridge/**`  
**Authority:** none; an issued ticket, active lease, acknowledged context and exact qualified server/client/schema set are still required.

Read only the materialized package context, exact accepted dependency handoffs, artifact/client/schema inputs, package assignment, `FUNCTIONS.md`, this packet and common rules in `../W3_MILESTONE_PACKETS.md`.

## QB0 — Client and schema handshake

Implement exact client/server/build/schema/profile compatibility, collection topology inspection and strict payload-index parity. Raw vendor clients and names stay private.

## QB1 — Bounded data operations

Implement vendor-neutral create/inspect/upsert/read/delete operations with finite batches, strong mutation ordering, exact acknowledgements and readback.

## QB2 — Query/filter semantics and recovery

Implement accepted base-filter and filtered-IDF request translation, strict unindexed-filter rejection, malformed/partial response handling, cancellation and unknown mutation outcomes.

## QB3 — Conformance and handoff candidate

Close client/server/schema goldens, missing-field/signed-epoch/strict-mode probes, vendor-type escape guards, resource bounds, line budget and package submission evidence.

## Exit rule

Each checkpoint records failing-first tests, exact commands/raw outcomes, unavailable checks, package-only diff, dependency/artifact/schema digests and line count. QB3 creates only a submission candidate; independent review and integration-owned handoff remain separate.
