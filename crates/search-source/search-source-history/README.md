# search-source-history — forwarding restore/history package scheduled for deletion

This package owns no source body, database or current source state. It forwards/implements old revision
registration, namespace cutover and full registry restoration over `search-source-records`.

## Verified source inventory

Production files total about **70.1 KiB** before tests:

```text
registry_restore.rs   28,379 B
revision.rs           35,816 B
namespace_adapter.rs   4,989 B
lib.rs                   930 B
```

Tests add about 27.4 KiB. The package exists because the old architecture separated schema records from
history/replay adapters, not because the current application needs a history subsystem.

## Replacement

Current source lifetime is:

```text
immutable ObjectRef / RevisionRef
+ one current SourceRecord head in typed redb
+ PublishedView manifest entries while current/pending/pinned
```

A source update compare/sets the current head. Old immutable revisions remain reachable only while current,
pending, cleanup or live views refer to them. Object GC deletes unreferenced artifacts later. No universal
revision-registration history, namespace ownership transition or full registry restore is required.

Move a genuinely useful `RevisionRef`/validation field to the object/source owner after #598's accepted
handoff. Move no operation replay or old codec.

## Delete after #270/#271 cutover

- registry restore and v4 original-input resolver;
- namespace owner/cutover adapter;
- historical revision registration/receipts/currentness;
- forwarding exports to `search-source-records`;
- package tests/goldens and workspace/dependency/status entries.

Unsupported old roots remain untouched. Normal restart reads typed current root/source/head rows and validates
their immutable object references; it never replays a universal source history.

See [search-source-records](../search-source-records/README.md),
[source registry](../search-source-registry/FUNCTIONS.md) and issue
[#270](https://github.com/UnknownAlienHuman/eliot-search/issues/270).
