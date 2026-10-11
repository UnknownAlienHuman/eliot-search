# search-source-admission

**Security support for C03/C06 — Source admission policy.**

**Status:** `SOURCE_PRESENT`; deny-by-default policy, decision and receipt source exists. #320 replaces copied SHA-256 with the shared digest implementation; effective policy migration and durable source-ingestion composition remain incomplete. See [central package status](../../../docs/product/PACKAGE_STATUS.toml). Current owners: #241 → #110.

Evaluate a versioned deny-by-default source-admission policy without reading source bodies or mutating registry state.

The permanent [effective v2 evaluator](EVALUATION_V2.md) resolves class-scoped compiled rules with
mandatory safety/completeness checks and emits one immutable internally bound decision. It uses the
accepted glob matcher and shared canonical digest producer, requires the caller's original checkpoint,
charges finite complete-preimage/output limits and clears scratch on every refusal. It grants no
receipt/native/root/access/currentness proof. V1 and accepted policy/observation identities remain exact;
consumer adoption, receipt/batch and integrated qualification stay separate under #241/#405/#270/#271/#215.

The five retained v1 section/policy/observation/receipt/batch preimages are
**BYTE_COMPATIBLE**: domain + NUL, big-endian integers, u64 byte-string lengths,
option tags and field order are unchanged. The sole existing legacy writer charges
the complete input against the fixed 1 MiB admission ceiling before copying each
field; a sticky overflow returns no digest. `search-contracts::sha256_exact_bytes`
executes accepted RustCrypto `sha2 0.10.9` without adding another prefix or codec.
The admission package contains no SHA compression or padding implementation.

`ValidatedAdmissionConfig` is produced only by `validate_section` and exposes
five read-only getters for its private profile, size and permission fields.
Callers edit `ConfigSectionInput` and validate a new section. They cannot
construct or mutate the validated fields. This validates data without granting
applied-effect or source-admission authority.

`section_digest` retains `Result<Blake3Digest32, AdmissionError>` and its bounded
writer. Oversize returns the content-free `ADMISSION_OBSERVATION_INVALID`.
The legacy return container holds SHA-256 bytes;
it does not certify BLAKE3 execution. Opaque bounded policy/observation inputs keep
their infallible signatures. No stored identity is relabelled or rebuilt here.
Caller-specific lower v1 `AdmissionLimits` behavior is unchanged by this absolute
ceiling; the separate v2 evaluator enforces its own complete operation ceilings.

The public Rust API deliberately replaces writable fields with immutable
methods. Validated values retain exact v1 SHA bytes (**BYTE_COMPATIBLE**),
validator/reload behavior and existing consumer source. Four private
constructor/getter/input-isolation/reload fixture groups are
**AUTHORED_NOT_RUN** under #352/#241/#215. The existing owner-only synthetic
oversize fixture checks the defensive codec ceiling; it supplies no public
validated-value bypass.

Legacy v1 rule patterns remain validated, normalized and fingerprinted diagnostic
bytes. V1 has no path predicate, so `evaluate`, `issue_receipt`, `verify_receipt`
and the whole `evaluate_batch` refuse any pattern-bearing policy with the closed
`ADMISSION_POLICY_SCHEMA_UNSUPPORTED` error before hashing or allocating results.
Pattern-bearing v1 authority is **LEGACY_DECODE_ONLY**. Policies without patterns
keep their existing operation and retained v1 bytes. The accepted v2 matcher and
relative-locator models feed the separate effective v2 evaluator; receipts and
consumer adoption remain under #241/#270/#271.

Five private refusal/parity/precedence fixtures are **AUTHORED_NOT_RUN** under
#352/#241/#215. Historical receipt construction exists only inside `cfg(test)`;
it supplies no alternate production issuer or public proof constructor.

Frozen vectors for all five domains and bounds/overflow fixtures are
**AUTHORED_NOT_RUN** under #352/#215. Production compilation and source guards do
not qualify ingestion, Qdrant, Windows installation or the complete product.

## Owns

The additive [`observation_v2`](OBSERVATION_V2.md) module validates bounded
portable root-relative UTF-8 bytes and binds them with existing validated
metadata into an immutable observation equality model. It preserves case and
Unicode bytes, uses the shared canonical CBOR/BLAKE3 implementation and exposes
no policy, decision or receipt authority. Its locator profile has fixed
4,096-byte, 255-byte-component and 128-component limits; its complete preimage
ceiling is 16 KiB. V1 types, APIs and preimages remain unchanged. Future consumer
adoption requires **VERSIONED_REBUILD**; this slice performs no cutover.

Six private causal fixture groups and an independently encoded synthetic golden
are **AUTHORED_NOT_RUN** under #352/#241/#215. Source checks do not establish
matcher behavior, native containment or product qualification.

#414 adds `new_with_encoded_limit` and readonly `validate_encoded_limit` for caller-selected
complete domain + NUL + CBOR ceilings from 1 through 16,384 bytes. The shared producer keeps
identity bytes exact; the operation ceiling is not retained or hashed as another field.
Invalid raw ceilings refuse as ObservationInvalid; shared preimage/sink byte exhaustion
refuses as BudgetExhausted in the new APIs. Existing `new` retains its fixed ceiling and
all-error ObservationInvalid behavior. Revalidation recomputes and checks the private identity
without mutation. Bounded row values may still be copied before serialized-byte charging.
Private boundary/parity/error/readonly fixtures are **AUTHORED_NOT_RUN**, Q/#215. Other lower
limits, receipt/batch and integration remain #405/#241/#270/#271. Effective evaluation
and immutable decision identity are supplied by #416.

#418 caps the raw `unavailable_fields` list at the existing eight-name vocabulary before iteration
or cloning in `validate_observation`. Oversized lists return `ObservationInvalid`, including an
oversized list with an unknown marker. Earlier input checks retain their priority; in-cap validation
and all accepted v1/v2 identity bytes remain exact (**BYTE_COMPATIBLE**). This bounds the validator's
work and copy, without bounding caller-side input construction. Private causal fixtures are
**AUTHORED_NOT_RUN**, Q/#215; receipt/batch/cumulative bounds remain #241/#405 and actual ingestion
adoption remains #270/#271. No runtime or resource qualification follows from this source guard.

- canonical policy normalization and fingerprinting
- path/metadata/format/sensitivity observation evaluation
- deterministic decisions, reasons and receipts
- default exclusion fixtures

## Must not own

- filesystem/Git reads
- root registration, identity or membership state
- post-admission access authority
- silent allow-on-unknown behavior

- **Delivery wave:** W2 / P03
- **Soft source-line target:** 4,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
## Compiled v2 path classification (#412)

The additive [`policy_v2`](POLICY_V2.md) module compiles an immutable bounded
policy with one reviewed `globset` matcher, an owner-computed shared
CBOR/BLAKE3 fingerprint and reusable policy-bound match storage.
`compile_policy_v2` accepts raw rules and validated section data;
`match_path_rules` selects canonical rule indexes from accepted relative bytes.
It supplies no admission decision, receipt, batch or native authority.

Both calls require a borrowed caller checkpoint for the original absolute
deadline and live cancellation. Closed `Cancelled`/`DeadlineExceeded` failures
map to existing cancelled/budget-exhausted errors. No owner clock, context,
lock or hard preemption is introduced. Match failures clear all results.

V1 bodies and preimages remain **BYTE_COMPATIBLE**. Future v2 adoption requires
**VERSIONED_REBUILD**; this slice performs no consumer cutover or v2-to-v1
facade. Effective evaluation is supplied by [evaluation_v2](EVALUATION_V2.md);
receipts/batch remain #241/#405. Private causal
fixtures are **AUTHORED_NOT_RUN** under #352/#241/#215. Donor provenance and
source gates do not qualify native, runtime, scale or installed behavior.

## Immutable v2 receipt issuance and verification (#420)

The permanent [receipt_v2](RECEIPT_V2.md) API issues and verifies a private immutable receipt
embedding the sole effective evaluator's decision. It checks the exact current policy/observation
fence, four finite caller-selected count/complete-preimage ceilings and the original checkpoint,
including after the final clone. One shared five-row CBOR/BLAKE3 profile binds the decision
transitively; no duplicate safety evaluator, supplied digest, public codec or v1 fallback exists.
Verified Deny/Review/Unsupported remains that outcome. Debug omits source/locator/IDs/profile data.
Existing v1 and accepted policy/observation/decision bytes remain BYTE_COMPATIBLE; future consumer
adoption is VERSIONED_REBUILD under #270/#271. Batch/cumulative budgets stay #241/#405, runtime
#333 and qualification #215. Private causal fixtures are AUTHORED_NOT_RUN; source gates do not
compile or execute their graph or establish native/live-Qdrant/installed/scale/product exposure.
The pre-edit receipt-only size review caps aggregate owner Rust at9200 physical; hard10000 remains.
