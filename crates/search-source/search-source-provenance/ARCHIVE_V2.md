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
