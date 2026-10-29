# Source registry — typed current rows

Source/control owns current registrations and source heads; the object store owns bytes; PublishedView owns
served evidence. This package must not become another database, replay engine or history service.

<a id="current-records"></a>
<a id="current-operations"></a>
## Current-state mutation

[FUNCTIONS.md](FUNCTIONS.md#current-state-and-conditional-application) is the single definition of row mutation
and stale-work rejection. A content head alone is not the expected-state guard: use the captured non-reset row
and registration/settings state required there. Do not reconstruct record layouts or commit signatures from an
abbreviated README example.

The [connected operation](FUNCTIONS.md#connected-source-operation) defines the acquired-body handoff, object-before-
metadata order, uncertain commits and lost wake-ups. The [view boundary](FUNCTIONS.md#published-view-versus-source-state)
separates mutable desired state from the served snapshot. Source-head commit does not itself make data searchable.

## Remove with callers

Follow the [source/caller removal map](FUNCTIONS.md#remove-after-cutover). Move real consumers and useful current
values before deleting forwarding history/records packages. Plain ingestion does not wait for optional #598;
only actual unpublished object/key overlap needs coordination. Unsupported roots and sole retained copies stay intact.

## Completion

Use the [owner's checks](FUNCTIONS.md#checks-after-code) and the current
[delivery stage](../../../docs/implementation/README.md#delivery-sequence). No new cache, public repository trait
or compatibility layer is required to preserve this package.

<a id="verified-current-source"></a>
## Historical inventory

The [earlier inventory](https://github.com/UnknownAlienHuman/eliot-search/blob/4cb8dc4d61924ee0a1a2f3ce056b7086ac45c332/crates/search-source/search-source-registry/README.md#verified-current-source)
is a source-inspection aid, not a current count or replacement specification. Inspect actual callers before a code cut.
