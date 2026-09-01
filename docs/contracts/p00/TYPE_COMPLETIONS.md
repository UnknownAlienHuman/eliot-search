# P00 named-type completions

This file closes named helper types already used by the field-level P00 schemas but not explicitly
registered in `TYPE_REGISTRY.md`. It does not change Architecture 8.4 Part I. Part I and the existing
field-level schemas remain authoritative.

A writer must not replace any type below with an unbounded `String`, `Vec`, map, JSON object, runtime
handle or vendor type.

## `RecipeIdV1`

Closed provider-wire enum with exactly eleven values:

```text
locate@1
find_text@1
inspect_entity@1
compare_implementations@1
explore_entity@1
corpus_profile@1
corpus_delta@1
provenance@1
compile_exact_scan@1
execute_exact_scan@1
expand_handle@1
```

Unknown, unversioned, aliased or vendor-specific values fail contract validation. Shape owner:
`search-contracts`; pure compatibility meaning: `search-domain`; request compilation:
`search-query-planner`.

## `RecipeBodyV1`

Closed tagged union keyed by `RecipeIdV1`. Its variants are the eleven exact bodies in `RECIPES.md`.
Exactly one variant is present and the variant tag must equal the enclosing recipe ID. A generic JSON
object, map or opaque vendor request is forbidden.

Shape owner: `search-contracts`; request validation and compilation owner: `search-query-planner`.

## `ComparisonAxis`

Closed enum:

```text
interface
validation
errors
side_effects
tests
callers
documentation
```

It is descriptive only and grants no normative-verdict authority. Shape owner: `search-contracts`;
meaning owner: `search-domain`; comparison-state owner: `search-comparator`.

## `ProtocolRange`

```yaml
ProtocolRange:
  minimum: ProtocolVersion
  maximum: ProtocolVersion
```

Rules:

- `minimum <= maximum` under lexicographic `(major, minor)` ordering;
- no wildcard, floating latest version or open-ended maximum;
- negotiation selects the highest mutually supported version;
- no overlapping version means `PROTOCOL_VERSION_MISMATCH`;
- a major version is never silently coerced to another major;
- unknown load-bearing fields fail closed.

Shape owner: `search-contracts`; compatibility meaning owner: `search-domain`; session negotiation owner:
`search-provider-protocol`.

## `PackageOpaque`

`PackageOpaque` is a type class for process-local capability references used by `search-ports`. It is
not a provider-wire or durable schema.

Required properties:

```text
non-serializable
non-canonicalizable
non-comparable across process incarnations
not constructible by a consumer
owner-package scoped
redacted Debug/Display/error surfaces
cannot be converted to path, socket, channel, executor, store or vendor handle by shared APIs
```

A concrete package implements its own opaque type behind the shared port signature. `search-ports` owns
the type-class contract; the capability package owns the concrete value and lifetime.

## Registry accounting

These five symbols are included in `swarm/coverage/schemas-primitives.toml`. Together with the 115 named
types already present in `TYPE_REGISTRY.md`, they close the P00 primitive/helper type surface consumed by
all record schemas.

## Required tests

- exact `RecipeIdV1` eleven-value fixture;
- recipe/body mismatch and unknown recipe rejection;
- exact `ComparisonAxis` fixture and unknown-axis rejection;
- protocol range ordering, overlap, highest-common-version and major-mismatch tests;
- compile/serialization guards proving `PackageOpaque` cannot cross provider or durable boundaries;
- no local alias may duplicate one of these types with weaker bounds or semantics.
