<a id="qdrant-adapter-boundary"></a>
# Qdrant adapter boundary — navigation

The [bridge recipe](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) owns SDK mapping and the
local connection. Do not maintain a second schema, parameter table or process model on this page.

## Selected runtime boundary

Use the operator-started same-host server, one client and ordinary optional API-key input. No supervisor/lease
constructor, remote endpoint, hidden fallback or key lifecycle. Connection may precede the first collection;
[readiness cases](../../crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md#connection-versus-index-readiness)
distinguish no index, damaged current state and a verified empty generation.

## Physical view identity

[Publication](../../crates/search-index-qdrant/search-publication/FUNCTIONS.md) owns fixed builds, exact physical
collection/evidence binding, view installation and cleanup. No alias or process report supplies that authority.

## Scope and sharding

[Local selection](../../crates/search-query/search-access/FUNCTIONS.md) owns base population versus narrowing;
the bridge maps it to the selected backend. No separate grants, custom shard portfolio or local ranking backend.

## Current code debt

The [prior source map](https://github.com/UnknownAlienHuman/eliot-search/blob/7ed46f79f07a23f01f75f5e85a96248dc42e280b/docs/runtime/QDRANT_ADAPTER_BOUNDARY.md#current-code-debt)
contains revision-specific removals and constructor findings. Recheck actual callers, not its historical tense.

## Optional process and Edge boundaries

Use the [legacy-supervisor removal map](../../crates/search-index-qdrant/search-qdrant-supervisor/FUNCTIONS.md).
Managed process/Edge work does not block the configured endpoint or justify a backend abstraction.

## Verification

Use [minimal production checks](../implementation/WORKING_GUIDE.md#minimal-production-checks) and the bridge's
later connected cases. Old library-only checks are not a current-main build or a Qdrant 1.20 runtime result.
The [Windows note](QDRANT_NATIVE_WINDOWS.md) owns platform evidence/update routing, not another implementation.
