# P00 context artifact candidate builder v1

**Status:** executable, deterministic and non-authoritative. It builds the exact proposed writer-visible
artifact bytes for one P00 package. It does not store an immutable artifact, create a context manifest,
issue a ticket or lease, authorize implementation, accept a package, satisfy G0/W0 or advance launch
state.

Machine registry:
[`../../swarm/context-artifact-builder-v1.toml`](../../swarm/context-artifact-builder-v1.toml).
Candidate schema:
[`../../swarm/context-artifact-candidate-schema-v1.toml`](../../swarm/context-artifact-candidate-schema-v1.toml).

## 1. Purpose and boundary

The ticket-issuance planner answers whether an exact package/commit is structurally eligible for context
materialization. This builder performs the next safe step: it reads the same immutable Git tree and
constructs the exact bounded context bytes plus a content-minimized candidate metadata file.

The output is intentionally incomplete as a `context_manifest_v1`. These fields remain unresolved:

```text
identity.context_id
identity.operation_id
artifact.ref
verification.readback_verified
signature.created_at
signature.materializer_identity
signature.reviewer_identity
signature.record_sha256
signature.materializer_signature_ref
signature.reviewer_signature_ref
record path / Git commit / Git blob / exact complete-file digest
```

Those values require a selected artifact-store profile, durable store readback, distinct materializer and
reviewer identities, dual signature artifacts, a committed immutable manifest and exact post-commit
readback. Local files under `artifacts/` cannot satisfy those requirements.

## 2. Inputs

A build requires:

```text
package                  search-contracts | search-domain | search-ports
base_commit              full algorithm-tagged immutable Git commit
accepted_handoff paths   exact committed package_handoff_v1 records required by the draft
output root              artifacts/context-artifact-candidates or a descendant
```

Every repository input is loaded with Git object reads from `base_commit`. The working tree, index,
branch name, wall clock and random state do not affect candidate bytes or identities.

Before building, the tool reuses the schema-v2 planner checks for:

- package/function/stage/launch parity;
- exact non-claimable ticket/context draft pair;
- manifest-owned 16/24 source ceilings, six selector ceiling and one handoff-slot ceiling;
- exact manifest-closed `search-contracts` source pack;
- regular committed UTF-8 source blobs and forbidden-path fencing;
- exactly-one closed selector resolution;
- accepted prerequisite handoff identity/signature/supersession checks;
- current-package control-record conflicts and an existing W0 receipt;
- control-schema versions and manual/read-only/credential-free workflows.

Any failed preflight stops before output.

## 3. Source materialization

Each declared source is read by exact Git blob. The candidate records both identities:

```text
exact committed bytes:
  Git blob ID
  SHA-256
  byte length

materialized payload:
  strict UTF-8
  CRLF -> LF
  lone CR -> LF
  no Unicode normalization
  no forced terminal newline
  SHA-256
  byte length
```

NUL is rejected. Source order is exactly the context-draft order. Architecture files, binaries,
implementation `src/**` trees and issued control roots remain forbidden.

## 4. Registry fragments

The closed selector grammar remains:

```text
swarm/crates.toml::package[name=<package>]
swarm/function-packets.toml::foundation[package=<package>]
swarm/stages.toml::stage[id=W0]
swarm/launch-state.toml::authorized_packages[<package>]
swarm/launch-state.toml::conditional_packages[<package>]
swarm/launch-state.toml::conditional_activation.<package>
```

A selector must resolve exactly once. The selected semantic value is rendered as canonical JSON:

```text
UTF-8
LF terminated
lexicographically sorted object keys
compact separators
semantic array order preserved
no null, float, datetime or non-string map key
```

The candidate records the registry Git blob, exact registry-file SHA-256, selector, match count, fragment
SHA-256 and fragment length.

## 5. Accepted handoffs

`search-domain` and `search-ports` require the exact accepted `search-contracts` handoff declared by their
drafts. The builder verifies it with the planner and embeds its exact canonical UTF-8/LF record bytes as a
length-framed block. A branch, API sketch, implementation source tree, bad signature, wrong path,
missing final commit or superseded handoff fails closed.

`search-contracts` accepts no prerequisite handoff and rejects extras.

## 6. `ELIOT_SWARM_CONTEXT_1` framing

The artifact is a deterministic text container with byte-length framing:

```text
ELIOT_SWARM_CONTEXT_1
<canonical JSON preamble>

--- repository-path: <path> ---
<canonical JSON block metadata including content_bytes/content_sha256>
<exact content_bytes materialized source bytes>
<one framing LF>

--- registry-selector: <path>::<selector> ---
<canonical JSON block metadata including content_bytes/content_sha256>
<exact content_bytes canonical fragment bytes>
<one framing LF>

--- accepted-handoff: <package> ---
<canonical JSON block metadata including content_bytes/content_sha256>
<exact content_bytes handoff record bytes>
<one framing LF>

--- end-context-artifact ---
```

Length framing makes source text containing header-like lines unambiguous. The builder parses the complete
result again, verifies every block length/digest/header and requires exact semantic round-trip before it
writes output.

Block order is:

```text
all sources in declared order
all registry fragments in declared order
all accepted handoffs sorted by package
```

## 7. Candidate identities

Three SHA-256 values have separate meanings:

```text
artifact_candidate.sha256
  SHA-256 of exact ELIOT_SWARM_CONTEXT_1 bytes

candidate_id
  SHA-256("eliot-search/context-artifact-candidate/v1\0" || exact artifact bytes)

candidate_sha256
  SHA-256(
    "eliot-search/context-artifact-candidate-metadata/v1\0"
    || canonical candidate JSON with only candidate_sha256 omitted
  )
```

None is a `context_id`, `materialize_context_v1` operation ID, immutable artifact ref, signed-payload
digest or complete committed manifest-file digest.

## 8. Output and idempotency

The only output root is:

```text
artifacts/context-artifact-candidates/
```

For one candidate:

```text
<root>/<package>/<candidate_id>.context
<root>/<package>/<candidate_id>.json
```

Both extensions are ignored by Git. Equal existing bytes are an idempotent success. A pre-existing path
with different bytes is `CANDIDATE_OUTPUT_CONFLICT`; it is never overwritten. Symlinked output components
and paths outside the artifact root are rejected. Writes use same-directory temporary files, atomic
replacement and exact local readback.

Local readback proves only local candidate-file integrity. It is not artifact-store or committed-control-
record readback.

## 9. Candidate metadata

The adjacent JSON includes:

- immutable repository/base/draft identities;
- exact and UTF8/LF source identities;
- exact selector source and fragment identities;
- accepted handoff inputs;
- bundle path, digest, size and format;
- complete planner preflight checks;
- required unavailable checks;
- a non-schema-instance `context_manifest_v1` projection;
- the exact unresolved field list;
- zero control-record mutations and all authority flags false.

Successful candidates have `reason_codes = []`.

## 10. Commands

```powershell
$format = git rev-parse --show-object-format
$base = "${format}:$(git rev-parse HEAD)"

pwsh -NoProfile -File tools/build-context-artifact-candidate.ps1 `
  -Package search-contracts `
  -BaseCommit $base `
  -PrintResult

pwsh -NoProfile -File tools/validate-context-artifact-candidate.ps1 -Json
python qualification/context-artifact/test_context_artifact_candidate_v1.py
```

A ready candidate means only that exact writer-context bytes have been constructed and checked. It does
not permit implementation to begin.
