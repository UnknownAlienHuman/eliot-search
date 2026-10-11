# Effective admission evaluation v2

`evaluation_v2::evaluate_v2` consumes one immutable compiled policy and one immutable observation.
It performs no I/O or durable mutation. It produces one immutable decision binding policy revision,
policy fingerprint and observation fingerprint, or returns a closed refusal with empty match scratch.
This is policy meaning only: no receipt, source identity, native root, access or currentness proof.

## Permanent contract

The caller supplies policy-bound `MatchBufferV2`, five raw `EvaluationLimitsV2` ceilings and its original
deadline/cancellation checkpoint. Each call clears scratch before the entry checkpoint, validates all
ceilings, charges the full canonical rule count, revalidates the complete observation preimage, invokes
the existing byte-path matcher and walks every class-scoped canonical rule. Checkpoints surround
synchronous hashing and matching and precede successful return. Every error clears scratch. There is
no owned clock, renewed deadline, hard preemption or zero-allocation claim.

| Ceiling | Inclusive range |
| --- | --- |
| max_rule_evaluations | 1..128 |
| max_matched_rule_ids | 1..128 |
| max_reasons | 1..23 |
| max_encoded_bytes | 1..65,536 complete decision preimage |
| max_observation_encoded_bytes | 1..16,384 complete observation preimage |

Defaults use fixed maxima. Invalid raw ceilings return `ObservationInvalid`; actual count/complete-byte
exhaustion returns `BudgetExhausted`. Missing relative locator refuses the whole call with
`ObservationIncomplete`, including nonpath/empty policies. Cross-policy scratch returns `PolicyConflict`.
Only shared `OversizePayload` at `digest_preimage`/`canonical_sink` maps to decision byte exhaustion;
other shared failures return `ObservationInvalid`. The existing checkpoint mapping remains the owner.

## Effective meaning

Deny dominates unsupported, then review, then explicit Allow; otherwise `DenyByDefault`. Every
predicate is scoped to its declared source class. Path predicates use only the existing internally
regenerated classifier indexes. No implicit baseline Allow, v1 fallback or second matcher exists.

Credential/private-key classes and sensitivity, system class or positive system flag, cache/build,
remote location, SecretCandidate, zero size and over-section size deny. Generated/vendor/binary class
or positive flag enforces the corresponding immutable section permission. A true permission removes
that baseline denial and grants nothing without an explicit matching Allow. Confidential also requires
explicit Allow. Missing size, detector, profile, sensitivity or flags produces existing review reasons;
unsupported SourceKind produces unsupported, rather than treating absent metadata as unsupported.

Distinct baseline/rule reason contributions consume `max_reasons` before winner filtering. Duplicate
reasons use no extra slot. The decision retains winning-category reasons in closed enum order and all
applicable IDs in canonical precedence/stable-ID order. ID capacity is charged before every clone;
no output is silently truncated. Private fields and readonly getters have no public decision constructor,
supplied digest, accepted flag or mutable input view. Debug omits IDs, patterns, metadata and locator text.

## Equality profile

Schema 2, profile `eliot-admission-effective@2`, domain `eliot/cbor/admission/decision/v2` use one private
shared canonical CBOR/BLAKE3 producer. Exactly eight rows:

0. `[2, profile, 128, 128, 23, 65536, 16384]`;
1. policy revision;
2. policy fingerprint bytes;
3. observation fingerprint bytes;
4. outcome tag;
5. ordered winning reason tags;
6. canonical ordered matched IDs;
7. sensitivity tag.

Caller evaluation ceilings are operation limits, excluded from identity. Every sufficient ceiling
preserves identity. Existing compiled policy compile-limit rows remain bound and unchanged. V1 and
existing v2 policy/observation bytes remain **BYTE_COMPATIBLE**. Future consumer adoption under #270/#271
requires **VERSIONED_REBUILD**; this slice neither migrates nor relabels stored state.

## Evidence boundary

Private causal fixtures are **AUTHORED_NOT_RUN**, owned by #215 integrated qualification. Production
check/Clippy/static guards and exact-head reviews establish source delivery only. Daemon lint debt
remains D/#387, NOT_PASS at unchanged normalized diagnostics. Receipt/batch/raw/cumulative bounds
remain #241/#405; runtime #333 and native/live/installed/scale qualification #215 remain incomplete.
