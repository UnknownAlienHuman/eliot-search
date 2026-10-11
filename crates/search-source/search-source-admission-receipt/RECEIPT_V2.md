# Admission receipt v2

`receipt_v2` issues an immutable receipt over a producer-derived `AdmissionDecisionV2` and verifies it against an exact current immutable policy/observation fence. It performs no I/O or durable mutation. A verified receipt retains its outcome: Deny, ReviewRequired and Unsupported never become Allow. This is admission-policy provenance only, without native containment, root/source identity, access or currentness proof.

## Permanent API

`issue_receipt_v2(policy, observation, decision, limits, probe)` checks the exact revision, policy fingerprint and observation fingerprint of the sole evaluator's private immutable decision. `verify_receipt_v2(receipt, current_policy, observation, limits, probe)` checks the stored trusted decision and recomputes receipt identity. The receipt privately embeds that decision; `decision()` exposes it readonly rather than duplicating the result shape. `schema_version()` and `fingerprint()` are readonly; `VerifiedAdmissionReceiptV2::receipt()` is the wrapper's sole getter. Receipt and verified wrapper have no public constructor, supplied digest, mutable view, accepted flag, codec or v1 fallback. Future untrusted decoding cannot mint this provenance by digest equality.

Deny/safety logic stays in the permanent evaluator. Verification needs neither a second safety evaluator nor fresh reevaluation: the exact immutable input identities and private producer-derived decision are required. A policy section/profile/compile-limit change changes the policy fingerprint and makes the old receipt stale.

## Four finite operation ceilings

| Caller ceiling | Inclusive range | What is charged |
| --- | --- | --- |
| max_encoded_bytes | 1..4096 | complete receipt fingerprint domain + NUL + canonical CBOR |
| max_observation_encoded_bytes | 1..16384 | complete existing observation preimage |
| max_reasons | 1..23 | embedded decision reason count before variable copies |
| max_matched_rule_ids | 1..128 | embedded decision ID count before variable copies |

Defaults use fixed maxima. Invalid raw ceilings return ObservationInvalid; actual count/complete-byte exhaustion returns BudgetExhausted. This does not re-charge the decision preimage or bound caller-side input construction. Decision production has its own accepted lower ceilings. Observation/canonical row copies remain bounded but may precede sink-byte charging; this is not a zero-allocation claim.

Both calls use the original caller checkpoint before raw validation, around observation revalidation/hash work and after cloning before return. No clock, renewed deadline or hard preemption exists. Entry cancellation wins over invalid limits. Verification validates raw limits, then schema; current policy revision/fingerprint stale precedes observation mismatch. Issuance uses ReceiptMismatch for a decision/input fence mismatch; verification uses ReceiptStale for a changed policy, ReceiptMismatch for observation/receipt equality, and PolicySchemaUnsupported for unsupported schema. Counts and complete receipt hashing precede the variable receipt/decision clone. Any failed checkpoint returns no value, including after the local clone.

## Equality profile

Schema2, profile `eliot-admission-receipt@2`, domain `eliot/cbor/admission/receipt/v2`; one private shared canonical CBOR/BLAKE3 producer. Five rows:

0. `[2, profile, 4096, 16384, 23, 128]`;
1. policy revision;
2. policy fingerprint bytes32;
3. observation fingerprint bytes32;
4. decision fingerprint bytes32.

The accepted eight-row decision fingerprint binds outcome, ordered reasons, canonical ordered IDs, sensitivity and both input identities transitively. Caller operation ceilings are excluded, so sufficient lower ceilings preserve identity. The complete receipt preimage is 174..182 bytes: domain/NUL32, containing array1, header38, revision1..9 and three34-byte digest byte strings. The fixed4096 ceiling is finite and independently chosen; the existing decision ceiling remains65536 and observation16384.

Existing v1 and accepted policy/observation/decision types, APIs and preimages remain BYTE_COMPATIBLE. Future stored adoption is VERSIONED_REBUILD under #270/#271. No current consumer is cut over and no persisted receipt codec is supplied here.

## Permanent package owner and evidence boundary

#424 relocates this full implementation and its eight causal fixture groups to 
`search-source-admission-receipt::receipt_v2` over public immutable admission models.
The old admission receipt_v2 module is deleted without a forwarding reexport or dependency cycle.
Rust import migration is explicit; the schema/profile/domain/preimages and v1 remain unchanged.
The initial whole-owner Rust review ceiling is1200 lines, including all fixtures.
No prior receipt-only9200 allowance transfers. Fixtures remain AUTHORED_NOT_RUN;
production gates/source review and product/native/resource qualification #215 are separate.
