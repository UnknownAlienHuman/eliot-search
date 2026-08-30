# Receipt and digest canonicalization

The swarm uses digests to freeze assignments, accepted APIs and wave receipts. No tool may parse and
reserialize an accepted document before verifying its exact committed identity.

## Two non-circular record digests

A signed control record has two different digests:

```text
signed_payload_sha256
  SHA-256 over the exact UTF-8/LF bytes from the first byte of the file through the LF immediately before
  the `[signature]` table header. This value may be embedded in `signature.record_sha256` for v1 schemas.

exact_record_file_sha256
  SHA-256 over every exact committed file byte, including the signature table and embedded payload
  digest. This value is never embedded in the same file; it is recorded by the consuming record,
  operation receipt, Git blob/path manifest or supersession record.
```

The v1 field name `signature.record_sha256` therefore means **signed payload digest**, not a self-hash of
the complete file. The schema rule `exact_record_digest_rule` resolves to this definition. Implementations
must expose the two values as different types and must not compare one to the other.

The signature binds:

```text
record kind + schema version + signed_payload_sha256 + actor identity + immutable context
```

Independent readback always recomputes both the signed payload digest and the external exact file digest.

## Text files

For integration-owned ticket and receipt files:

- encoding: UTF-8;
- byte-order mark: forbidden;
- line ending: LF (`0A`);
- final line feed: required;
- trailing spaces: forbidden;
- exact file digest: SHA-256 over every exact committed byte;
- signed payload digest: SHA-256 over the exact pre-signature byte range defined above;
- field/list order: significant because exact bytes are authoritative;
- comments: part of both applicable digests and therefore forbidden in machine record instances.

The external receipt/consumer records repository path, Git blob identity and exact file SHA-256. Review
recomputes SHA-256 from committed blob bytes and verifies the embedded payload digest/signature.

## Assignment and instruction digests

Assignment and instruction digests use exact-file bytes. A formatting-only edit changes the digest and
supersedes tickets that bind the old version. Writers do not normalize or repair supplied files locally.

## Context artifacts

Writer context artifacts follow the separate canonical bundle format in
`docs/handoff/TICKET_ISSUANCE_OPERATIONS.md`.

For each source, the context manifest records:

```text
exact source Git blob identity
SHA-256 and length of exact source bytes
SHA-256 and length of materialized UTF-8/LF payload
```

This permits source verification without pretending normalized context bytes equal committed source
bytes.

## API/schema digest

A package API/schema digest is computed over an integration-reviewed manifest listing, in deterministic
lexicographic path order:

```text
relative_path<TAB>sha256(exact_file_bytes)<LF>
```

The manifest includes every public contract source, generated schema and conformance fixture declared
by the package handoff. It excludes implementation-private files unless they affect serialized or
observable public semantics.

The API digest is SHA-256 over the exact manifest bytes. The manifest itself is retained as an immutable
handoff artifact.

## Raw evidence

Raw command output is retained without content normalization. Its evidence record includes exact bytes
digest, command, environment and artifact identities. Redaction is permitted only before the evidence is
accepted and must be declared; secrets and source bodies must not be introduced into orchestration
metadata.

## Forbidden practices

- embedding the exact complete-file SHA-256 inside that same file;
- treating `signature.record_sha256` as a fixed-point self-hash;
- using parsed TOML/JSON/YAML object reserialization as accepted identity;
- silently converting CRLF/LF before exact-source verification;
- comparing materialized context digest to exact source-blob digest;
- omitting generated public schema files from the API manifest;
- changing an accepted receipt in place;
- claiming two different byte sequences have the same accepted receipt identity because their parsed
  values appear equivalent.
