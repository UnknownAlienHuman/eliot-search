# search-source-history

Transitional relocation of source-revision registration, namespace adapters and registry restore logic.
It owns no source body, database or current source state; the registry reexports its APIs.

The current application needs one immutable `RevisionRef` and one current source head. Retained-object I/O
belongs to `search-revision-store`; the short atomic head transaction belongs to `search-control-redb` and
the source registry/composition.

Move useful current revision/head types and codecs to those actual owners. Remove generic historical
operation replay, namespace cutover adapters and full registry restoration with their legacy consumers.
An old unsupported root is rejected before writes and left intact; normal startup does not replay a
universal source history.

Preserve #598's active unpublished work and consume its accepted retained revision/head result without
creating a competing implementation.
