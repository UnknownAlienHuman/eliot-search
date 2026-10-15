# Original-input archives v2

Both archives use the shared `eliot.cbor.length-first.v1` encoder/parser.
They retain metadata and root-relative locator bytes, never source bodies.
They do not share a fingerprint preimage with the policy or receipt owners.

The policy archive is a closed seven-field canonical map:
`archive_profile = eliot.source-original-policy@2`, `schema = 2`,
`profile = eliot-admission-globset-portable@2`, original `revision`,
the complete five-field normalized `section`, all four original compile
`limits`, and the ordered `rules`. Each rule retains ID, precedence, source
class, predicate/operator/operand and effect. Restoration uses the caller's
actual platform/capabilities and the sole section validator/policy compiler.
The complete regenerated map must equal the original bytes, including unknown
field refusal; no digest supplied by storage constructs the compiled model.

The source archive is exactly this seven-element canonical array:

```text
[2, "eliot.source-original-admission@2",
 [policy_schema, policy_profile, policy_revision, policy_fingerprint],
 [locator_class, location_class, source_kind, source_class, byte_size_or_null,
  generated_or_null, vendor_or_null, binary_or_null, system_or_null,
  sensitivity_or_null, detector_or_null, profile_or_null, unavailable_fields],
 relative_locator_bytes_or_null, observation_fingerprint,
 [receipt_schema,
  [outcome, ordered_reasons, ordered_matched_ids, sensitivity,
   policy_revision, policy_fingerprint, observation_fingerprint, decision_fingerprint],
  receipt_fingerprint]]
```

Restoration reconstitutes the observation, evaluates it and issues a receipt
through the existing v2 owners. It compares the entire newly encoded array
with the exact canonical stored bytes. It never decodes supplied fingerprints
into a trusted decision/receipt. All four outcomes remain exact. Verification
against a current policy is a separate receipt-owner call and may return stale
while historical reproduction remains valid.

Caller cancellation/deadline checkpoints reach parsing, bounded copying,
compilation, evaluation, issuance and the terminal return. Both archive objects
have opaque Debug output and share the complete original policy via Arc;
source archives contain only policy references. The parser ceilings are 256 KiB
per archive. These are not durable-value limits: the registry must check actual
complete state/operation envelopes against its 64 KiB control-value limit.
Policy-size measurement, durable placement and the actual Source v2 registry
consumer remain separate unfinished #569 work; these pure fixtures do not
qualify Native, real-redb restart, scale or an installed product.

## Bounded physical layout

An actually accepted 128-rule policy with 256-byte rule IDs and 32,768 aggregate
pattern bytes retains as 82,255 bytes. `storage_proof.rs` reproduces that case;
it exceeds one control value without exceeding the admission owner's limits.
This is a measured accepted case, not a global maximum-size claim.

`OriginalInputsLayoutV2` splits an owner-built archive into 16,384-byte bodies.
Its canonical reference is `[2, "eliot.original-input-fragments@2", kind,
archive_digest, complete_length, part_count]`, where kind is `policy` or
`admission`. The whole exact archive uses shared raw BLAKE3 with domain
`eliot/raw/original-policy-archive/v2` or
`eliot/raw/original-admission-archive/v2`, respectively.
The canonical technical key is `[layout_profile, kind, archive_digest, index]`;
the value is `[reference, index, body]`. These keys never identify a source.

Reconstitution checks the complete bounded part set, exact keys/references,
unique indices, exact lengths, whole-archive digest and canonical forms, then
calls the actual policy or original-admission restorer. A digest or layout
reference alone cannot construct a trusted receipt or currentness grant.
The I/O owner must supply the parts from one coherent journal observation,
enforce immutable conditional writes and preserve unknown outcomes.
This layout does not add a store, mutation ledger, publication phase or native
authority. Actual same-journal write/readback integration remains unfinished.
