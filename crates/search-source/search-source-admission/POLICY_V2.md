# POLICY_V2

## Purpose and authority boundary

`CompiledAdmissionPolicyV2` is an immutable compiled policy with a reviewed `globset` matcher, an
owner-computed identity, and a reusable byte-path rule selector. It is a data and classification
model. It grants no admission authority.

The matcher returns canonical rule indexes only. It produces no effective decision, no receipt,
no batch, no allow or deny dominance, and no native-path, source-identity, root or currentness
proof. Safety dominance, evaluation, receipts and batch are separate later #241 authority, and
#405 owns lower observation and batch budgets. There is no v2-to-v1 facade and no consumer
cutover.

The v1 policy, decision, receipt and batch bodies are unchanged (BYTE_COMPATIBLE). Adoption of v2
is a VERSIONED_REBUILD that happens only in a later explicit slice; nothing here relabels or
rebuilds a stored identity.

## Compile bounds

| Bound | Value |
| --- | --- |
| Rules | 128 |
| UTF-8 bytes per pattern | 1,024 |
| Total pattern bytes | 32,768 |
| Complete domain-plus-CBOR preimage | 131,072 |

`CompileLimitsV2` carries four caller ceilings (rules, pattern bytes, total pattern bytes, encoded
bytes). Compilation validates that every field is nonzero and within the absolute maximum,
then retains and binds the accepted limits into the policy and its fingerprint. Preimage ceiling
exhaustion returns the existing `ADMISSION_BUDGET_EXHAUSTED`; no partial policy is accepted.
Identifiers reuse the existing 256-byte check; patterns have the separate 1,024-byte ceiling.

Borrowed whole-input validation happens before owned copies or any donor call: caller checkpoint,
schema 2, unknown fields, revision and limit and rule count, then per-rule ID, pattern grammar and
aggregate, duplicate ID and precedence, and the safety override. Unknown fields return
`ADMISSION_POLICY_UNKNOWN_FIELD`; wrong schema returns `ADMISSION_POLICY_SCHEMA_UNSUPPORTED`;
malformed, conflicting or donor-compile failures return `ADMISSION_POLICY_CONFLICT`. An ALLOW
effect on Credential, PrivateKey or System returns `ADMISSION_POLICY_OVERRIDE_FORBIDDEN`.

## Bounded grammar, not the full globset dialect

The accepted grammar is a deliberately bounded portable subset: UTF-8 relative patterns with `/`
separators; no control, backslash, colon, leading or trailing slash, or empty, `.` and `..` raw
components. At most 128 raw components, 8 recursive `**` pairs, star runs of at most 2, 4
non-nested brace groups with 16 total nonempty arms, 16 closed classes with at most 128 interior
bytes excluding brackets, no nested class opener and no slash inside a class. Commas, braces and
stars inside a class are literal, and a comma outside braces is literal. No empty alternate,
unmatched bracket or brace, or nested brace. Empty alternates and unclosed classes are off.

These are cheap borrowed scans that bound work before the donor sees a pattern. globset alone
interprets precise glob syntax; this profile is not a second matcher. The ceilings are selected
source policy, not a measured timing or memory qualification.

`GlobBuilder` options are explicit and fixed: case_insensitive false, literal_separator true,
backslash_escape false, empty_alternates false, allow_unclosed_class false.

## Pure checkpoint envelope

Compile and match are pure. Each takes a caller-owned `&mut impl FnMut() -> Result<(),
CheckpointFailureV2>` and no clock, context, lock or background work. `CheckpointFailureV2` is
closed with `Cancelled` mapping to the existing `ADMISSION_CANCELLED` and `DeadlineExceeded`
mapping to the existing `ADMISSION_BUDGET_EXHAUSTED`. No new failure code, no invented
`DeadlineExpired` variant and no absolute `OperationContext` exist here.

The caller captures the original absolute deadline and the live cancellation probe and passes only
the closure. The owner neither constructs nor stores a probe, never renews the deadline between
phases, and checks before and after each synchronous `GlobBuilder` and `GlobSet` build and
match and before returning.

This is cooperative, not hard, preemption. A single donor call cannot be interrupted mid-flight,
so no worst-case execution time, no deadline proof and no zero-allocation claim is made. A
post-check failure drops the whole policy or result and clears the reusable buffer.

## Identity and match

Profile `eliot-admission-globset-portable@2`, domain `eliot/cbor/admission/policy/v2`, donor
version `globset/0.4.20`. Identity uses the shared `blake3_canonical_array` only, with no local
codec or crypto and no caller-supplied fingerprint. The complete preimage is 10 header rows then
one row per rule, bounded to 138 rows. The header binds schema, profile, locator profile, donor
version, the five options, the fixed caps and grammar ceilings, the four effective compile
limits, revision, the section array (profile, max file bytes, the three permissions) and the
rule count. Rule rows bind ID, precedence, source class, operator, operand, effect and the
computed reason. `PolicyFingerprintV2` has no public constructor.

`new_match_buffer` allocates `2 * glob_to_rule.len()` once, from the computed policy, and binds
the policy fingerprint. The two-insert suffix strategy plus unique PatternSet IDs and anchored
fixed literals make that a conservative pre-dedup bound. Buffer use with a different policy
fingerprint fails closed. The buffer is cleared before checks, before every match and after every
error. Donor indexes are mapped into canonical rule order; no caller-shaped index is accepted.

`Candidate::from_bytes` receives only the accepted `AdmissionRelativeLocator` bytes. It calls
donor `normalize_path`, whose rewrite is a no-op because backslash separators are already
excluded by the grammar. No native `Path`, lossy conversion or normalization of meaning happens
on the owner path. A missing locator returns `ADMISSION_OBSERVATION_INCOMPLETE`, never an empty
successful match. No fresh per-observation result vector is allocated.

Unequal fingerprints classify as `MIXED_SECURITY_BARRIER_AND_RECONCILE` and equal ones as
`NOOP`; equality is never treated as set-inclusion proof.

## Donor boundary and qualification

Donor is exactly `globset` 0.4.20 with one normal closure of aho-corasick 1.1.5, bstr 1.13.1,
memchr 2.8.3, regex-automata 0.4.18 and regex-syntax 0.8.11. Donor NFA and hybrid cache ceilings
of 10 MiB are source-fixed, not configurable here, and are not cancellation or timing guarantees.
Registry and cached archive checks, licenses, MSRVs, non-yank status and an empty OSV result
refreshed 2026-10-11T02:14Z are provenance evidence, not security, corpus or performance
qualification.

## Fixtures

Private fixture groups are AUTHORED_NOT_RUN under #352, #241 and #215: grammar limit
boundaries and class and alternate cases; duplicate, unknown, schema, zero and override
rejection; exact option, case, separator and raw-byte matching; canonical permutation, recompile
and per-field sensitivity; lower complete preimage and plus-one; cross-policy reuse, missing
locator and failure clearing; checkpoint before and after build and match; and conservative change
classification. Execution belongs to the integrated owners.
