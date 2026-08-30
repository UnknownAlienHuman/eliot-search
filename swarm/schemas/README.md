# Swarm control-record schemas

These TOML files define field-level machine contracts for integration-owned assignment control records.
They are schemas, not record instances and not implementation authorization.

## Record sequence

```text
context_manifest_v1
→ assignment_ticket_v1
→ writer_lease_v1
→ lease_event_v1: ACKNOWLEDGED
→ package_submission_v1
→ independent_review_v1
→ package_handoff_v1
```

Revocation/cancellation/replacement uses append-only `lease_event_v1` and
`supersession_receipt_v1`; accepted records are never edited.

## Field schema format

Each file contains global invariants and ordered `[[field]]` entries:

```text
path             dotted logical field path
kind             closed scalar/record/list type
required         whether absence is invalid
canonical_order  field order in exact machine record
rules            bounded semantic requirements
```

Unknown load-bearing fields are rejected. Concrete record files use the order declared by their schema
and UTF-8/LF exact-byte canonicalization.

## Digest rule

V1 schemas retain the field name `signature.record_sha256`. It is **not** a self-hash of the complete
file. It means the signed payload SHA-256 over exact bytes before the `[signature]` table.

A consuming record/operation receipt separately stores:

```text
Git blob identity
exact_record_file_sha256 over the complete committed file
```

These are different types and values. Full details are normative in
`swarm/RECEIPT_CANONICALIZATION.md`. A validator must reject any implementation that attempts an
embedded fixed-point complete-file digest.

## Content floor

Control records may contain repository-relative paths and immutable refs/digests. They contain no source
body, unsaved buffer, raw query, credential/token, local absolute path, provider internals or dependency
implementation source. Raw evidence remains in separately governed immutable artifacts.

## Non-claims

- no context materializer or ticket issuer exists;
- no draft is converted to a record;
- no package has an issued ticket or active lease;
- no schema PASS is package, G0 or W0 evidence;
- package writers cannot edit schemas or integration records.
