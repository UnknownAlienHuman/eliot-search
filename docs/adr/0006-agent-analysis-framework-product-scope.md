# ADR 0006 — Agent-oriented analysis and retrieval

- **Decision date:** 2026-10-03; reconciled 2026-10-23.
- **Status:** product mission retained; 8.4 implementation prescriptions superseded by the
  [current architecture](../architecture/README.md).
- **Boundary:** [ADR 0005](0005-standalone-search-product-and-controller-boundary.md).

## Decision retained

Search helps agents and people orient themselves, locate evidence, inspect ambiguity, compare selected
implementations and verify exact predicates across large explicit code/document scopes. Qdrant remains
the only indexed retrieval engine. Search owns retained-source meaning and useful results around it.

A ranked hit is a candidate, not proof of identity, absence, equivalence or consensus. Results identify
their exact evidence and limits. Progressive reading should make a useful first step possible without
loading the entire corpus. A model and agent controller are not required.

The [product behavior](../product/AGENT_ANALYSIS_FRAMEWORK.md) is the single current home for these
workflows and observable results. The [query owner](../../crates/search-query/README.md) records the actual
producer/consumer seams and gaps. Compilation, existing schemas and package names do not establish delivery.

## Implementation prescriptions retired

An explicit portfolio selects sources from the published view. It does not require a separate
`ReferencePortfolioRevision`, immutable-membership authority, plan digest, recipe host or grant platform.
Existing pre-release #213 schemas may be replaced with their real consumers; their existence is not a
reason to preserve a disconnected protocol.

The former decision text is preserved at
[`5af8cde5`](https://github.com/UnknownAlienHuman/eliot-search/blob/5af8cde5416fa33e3f1a8f3c19367ed520eac9a4/docs/adr/0006-agent-analysis-framework-product-scope.md).
It explains the original intent, not a parallel specification. Useful requirements now have current owners;
obsolete mechanisms and historical delivery queues do not override them.

## Consequences

Keep navigation, comparison, document coordinates and large-corpus behavior visible beyond the first lexical
milestone. Add optional syntax, model, document and MCP integrations only with a concrete consumer in the same
evidence path. Protect retained user data when replacing internal formats. Scheduling, task history and finish
authority remain outside Search.
