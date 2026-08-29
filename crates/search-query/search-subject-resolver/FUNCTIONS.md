# Function contract — `search-subject-resolver`

**Status:** W6/P11 logical contract; no resolver implementation exists yet.

The resolver identifies a subject under one explicit source/workspace/reference context. It returns one
resolved subject only when the available evidence uniquely supports it; material ambiguity is a normal
bounded output, not an error to hide or a ranking tie to break arbitrarily.

## State and authority

The package owns no durable catalogue and performs no source/index I/O. It consumes bounded,
authorized, source-backed candidate observations produced by accepted direct/exact/structural/lexical
legs and returns a deterministic resolution product.

It owns:

- request normalization and explicit context validation;
- resolution ladder and match-basis classification;
- equivalence/duplicate collapse inside one subject hypothesis;
- bounded ambiguity construction;
- source-view/fence-bound resolution receipt and drift revalidation.

It does not own candidate retrieval, source readback, handle authorization, repository lineage
independence, comparison or normative choice.

## `normalize_subject_request`

```text
normalize_subject_request(request, bounds)
    -> Result<NormalizedSubjectRequest, SubjectError>
```

Normalizes only contract-declared fields: explicit source handle/editor cursor, qualified symbol key,
name, signature observation, entity-kind constraint, modality, current/reference scope and requested
source/workspace view.

Normalization is deterministic and locale-independent. It does not invent a scope, remove a material
qualifier, coerce one entity kind into another or treat display paths/names as identity.

Empty or contradictory requests fail with a typed reason. Equal canonical requests yield equal request
digests.

## `validate_resolution_context`

```text
validate_resolution_context(request, source_view, workspace_view, plan_fence, grant)
    -> Result<ResolutionContext, SubjectError>
```

Requires one coherent authorized `SourceView`, optional `WorkspaceViewRevision`, source-owner
generations, access/live-deny/purge generations, currentness requirement and plan fingerprint.

A candidate from a different view/fence cannot be mixed into the resolution set. Empty authorized scope
is explicit. Possession of a handle or cursor still requires current authorization and owner-generation
validation by the producing package.

## Resolution-ladder fail-closed rule

The ladder is strictly ordered:

```text
explicit reference
> qualified key
> exact name
> signature and entity kind
> structural/lexical candidates
```

A lower rung is evaluated only after every applicable higher rung completed with a decisive
non-resolution result. **A higher-priority incomplete step blocks lower-priority `RESOLVED` output.**
Cancellation, timeout, budget exhaustion, truncation, unreadable evidence, observation gap or stale
context at a higher rung yields `INCOMPLETE` or material ambiguity; it is never treated as “no match” for
fall-through.

## `resolve_explicit_reference`

```text
resolve_explicit_reference(request, explicit_candidates, context)
    -> ResolutionStep
```

Handles an explicitly validated source handle or authenticated editor cursor. It succeeds only when the
reference identifies one exact subject occurrence in the requested context. Expired, revoked, stale,
foreign-binding or multiply-targeted references return explicit failure/ambiguity; the resolver does not
fall through as though the explicit request never existed.

Match basis: `explicit_handle` or `editor_cursor`.

## `resolve_qualified_key`

```text
resolve_qualified_key(request, candidates, context, limits)
    -> ResolutionStep
```

Matches exact normalized qualified symbol/entity keys under compatible modality/entity kind and source
view. Multiple materially different definitions remain ambiguous even when the qualified text is equal.
Aliases/renames collapse only when source-identity/lineage/structural receipts prove one subject.

Match basis: `qualified_key`.

## `resolve_exact_name`

```text
resolve_exact_name(request, candidates, context, limits)
    -> ResolutionStep
```

Uses exact normalized name within the explicit current/reference scope. Same-name candidates do not
collapse without stronger equivalence evidence. Repository, module, entity kind, signature and
configuration predicates remain visible dimensions.

Match basis: `exact_normalized_name`.

## `resolve_signature_and_kind`

```text
resolve_signature_and_kind(request, candidates, context, limits)
    -> ResolutionStep
```

Compares closed, bounded signature/entity-kind observations and compatible structural identity. It may
recognize a renamed analogue when the accepted evidence proves compatibility, but must preserve
uncertainty when overloads, cfg variants, generated/macro forms or incomplete parser assurance remain.

Match basis: `signature_kind`.

## `resolve_structural_lexical_candidates`

```text
resolve_structural_lexical_candidates(request, candidates, context, limits)
    -> ResolutionStep
```

Last-resort bounded nomination over validated structural and lexical candidates. Similarity/rank is a
match basis, not unique-subject proof. A material top set returns ambiguity; the resolver never selects
one because it ranked first by a small or vendor-specific score difference.

Match basis: `structural` or `lexical`. Optional semantic basis is unavailable until separately admitted.

## `collapse_equivalent_occurrences`

```text
collapse_equivalent_occurrences(candidates, equivalence_receipts, limits)
    -> Result<SubjectHypothesisSet, SubjectError>
```

Collapses only exact duplicates/aliases/renames proven to represent one subject under accepted source
identity, owner generation, repository lineage and structural receipts. Fork/mirror independence and
cross-repository analogue collapse belong to the comparator and are not performed here.

Conflicting or stale equivalence receipts keep hypotheses separate.

## `assemble_resolution`

```text
assemble_resolution(request, context, ladder_steps, limits)
    -> Result<SubjectResolution, SubjectError>
```

Closed output variants:

```text
RESOLVED
AMBIGUOUS
NOT_FOUND
SCOPE_EMPTY
INCOMPLETE
```

`RESOLVED` requires one materially unique hypothesis at the strongest completed ladder basis and no
unresolved higher-priority step. `AMBIGUOUS` contains a bounded deterministic set plus truncation/
coverage metadata. `NOT_FOUND` is not an absence proof beyond the executed resolution scope.

Stable ordering:

```text
ladder basis priority
> entity-kind compatibility
> assurance class
> current/reference portfolio priority
> source identity
> native coordinate
> candidate identity
```

Raw vendor scores never directly decide the final subject.

## `build_ambiguity_set`

```text
build_ambiguity_set(hypotheses, context, limits)
    -> SubjectAmbiguitySet
```

Each entry includes authorized display metadata, exact source handle/reference, match basis, entity kind,
signature/configuration observations, assurance/freshness, differentiating dimensions and reason codes.

The set exposes omitted count/reasons when bounded. Truncation can never be relabeled as complete
ambiguity enumeration.

## `issue_resolution_receipt`

```text
issue_resolution_receipt(resolution, request, context, evidence_receipts)
    -> Result<ResolutionReceipt, SubjectError>
```

Binds request digest, plan/source/workspace view, source-owner/access/security generations, candidate
set digest, ladder profile ID, output variant, selected/ambiguity identities and evidence receipts.

It contains no raw Qdrant score/filter, secret, inaccessible candidate metadata or source body.

## `revalidate_resolution`

```text
revalidate_resolution(receipt, current_fence)
    -> ResolutionRevalidation
```

Owner generation, source/workspace view, access/live-deny/purge, overlay shadow, observation freshness,
profile or candidate-evidence drift invalidates or requires re-resolution. A restrictive security
change always overrides the prior receipt. The resolver cannot patch a stale receipt in place.

## Cancellation, deadlines and retry

All operations are pure over captured bounded inputs. Cancellation/budget exhaustion during a ladder
step yields `INCOMPLETE` with executed/omitted steps and cannot produce `RESOLVED` unless every
higher-priority relevant step completed. Repeating equal inputs is deterministic and safe. There is no
durable mutation or unknown commit outcome.

## Typed failures and reasons

- `SUBJECT_REQUEST_INVALID`
- `SUBJECT_SCOPE_EMPTY`
- `SUBJECT_NOT_FOUND`
- `AMBIGUOUS_SUBJECT`
- `SUBJECT_CONTEXT_STALE`
- `SUBJECT_OWNER_GENERATION_CHANGED`
- `SUBJECT_ACCESS_REVOKED`
- `SUBJECT_OBSERVATION_GAP`
- `SUBJECT_EQUIVALENCE_UNPROVEN`
- `SUBJECT_EVIDENCE_INCOMPLETE`
- `SUBJECT_BUDGET_EXHAUSTED`
- `SUBJECT_CANCELLED`
- `SUBJECT_AMBIGUITY_TRUNCATED`

## Required tests / qualification evidence

- explicit valid handle/cursor wins; stale/revoked explicit reference does not silently fall through;
- qualified key precedes name; exact name precedes signature/structural/lexical;
- higher-priority incomplete evidence blocks every lower-priority resolved success;
- same-name materially different definitions return ambiguity;
- renamed true subject collapses only with accepted equivalence receipt;
- overload/entity-kind/signature/cfg variants remain distinct when material;
- structural/lexical top-rank difference alone cannot force resolution;
- bounded ambiguity exposes truncation and differentiating fields;
- all candidates bind one coherent source/workspace/security fence;
- view/owner/access/purge/shadow/currentness/profile drift invalidates receipt;
- cancellation of a higher-priority step cannot produce lower-priority resolved success;
- deterministic ordering and receipt bytes for equal inputs;
- no normative choice, Qdrant/vendor type, source body or inaccessible metadata crosses the API.
