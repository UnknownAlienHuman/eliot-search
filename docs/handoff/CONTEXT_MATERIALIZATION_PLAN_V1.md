# Context materialization plan v1

This is the executable bridge from a verified `context_artifact_candidate_v1` to a prospective
`context_manifest_v1`. It is deliberately non-authoritative.

## Inputs

- canonical candidate metadata and exact `ELIOT_SWARM_CONTEXT_1` bundle;
- one explicit context ID;
- recorded UTC creation time;
- distinct materializer and reviewer identities;
- one immutable artifact reference matching the bundle bytes;
- an external artifact-readback declaration matching that reference;
- explicit `OptionalV1` materializer/reviewer signature refs.

Selection JSON is an ordinary local input under `artifacts/context-materialization-inputs/`. It is not a
control record or signature receipt.

## Two phases

### Payload phase

With artifact/readback and actor selection but both signature refs `ABSENT`, the planner renders the exact
pre-signature TOML bytes and computes:

```text
materialize_context_v1 operation ID
signed_payload_sha256
```

The operation ID excludes signature artifacts because those artifacts can only be produced after the
signed payload digest exists. Actor identities, created-at time, candidate, bundle, artifact reference and
readback identity remain operation-ID inputs. The payload itself embeds the operation ID, so later
signature refs are bound to the same operation.

Decision:

```text
READY_FOR_DUAL_SIGNATURE_COLLECTION
```

### Complete proposal phase

Two `PRESENT` signature refs must bind the exact signed-payload digest and their selected actor. The
planner appends the canonical `[signature]` table, computes the complete-file SHA-256 and derives the only
prospective control path:

```text
swarm/context-manifests/<package>/<exact_record_file_sha256>.toml
```

Decision:

```text
READY_FOR_INTEGRATION_OWNER_READBACK_AND_COMMIT
```

Even this decision authorizes nothing. The integration owner must verify the artifact store, approval
artifacts, current control-plane zero/conflict state and exact committed Git readback in the real
`materialize_context_v1` operation.

## Accepted handoffs

The planner opens each exact handoff block embedded in the bundle, verifies record identity fields and
constructs `OrderedAcceptedPackageHandoff`. Its `evidence_digest` uses
`accepted_evidence_digest_v1`; the immutable handoff ref remains authority.

## Output boundary

Output is limited to ignored files under `artifacts/context-materialization-plans/`:

- `plan.json`;
- `context-manifest.payload.toml` after payload selection;
- `context-manifest.prospective.toml` after both signatures.

`control_record_mutations` is always empty and all authority fields remain false. The planner never writes
under `swarm/context-manifests/`, never issues a ticket/lease and never accepts a package, gate or wave.
