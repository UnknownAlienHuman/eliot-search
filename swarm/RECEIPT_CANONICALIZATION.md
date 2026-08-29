# Receipt and digest canonicalization

The swarm uses digests to freeze assignments, accepted APIs and wave receipts. A digest always covers
exact committed bytes; no tool may parse and reserialize a document before hashing it.

## Text files

For integration-owned ticket and receipt files:

- encoding: UTF-8;
- byte-order mark: forbidden;
- line ending: LF (`0A`);
- final line feed: required;
- trailing spaces: forbidden;
- digest: SHA-256 over the exact file bytes committed to Git;
- field/list order: significant because exact bytes are authoritative;
- comments: part of the digest and therefore discouraged in machine receipts.

The receipt records its repository path, Git blob identity and SHA-256. Review recomputes SHA-256 from
the committed blob bytes.

## Assignment and instruction digests

Assignment and instruction digests use the same exact-byte rule. A formatting-only edit changes the
digest and supersedes tickets that bind the old version. Writers do not normalize or repair supplied
files locally.

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
digest, command, environment and artifact identities. Redaction is permitted only before the evidence
is accepted and must be declared; secrets and source bodies must not be introduced into orchestration
metadata.

## Forbidden practices

- hashing parsed TOML/JSON/YAML objects after tool-dependent reserialization;
- silently converting CRLF/LF before verification;
- omitting generated public schema files from the API manifest;
- changing an accepted receipt in place;
- claiming two different byte sequences have the same accepted receipt identity because their parsed
  values appear equivalent.
