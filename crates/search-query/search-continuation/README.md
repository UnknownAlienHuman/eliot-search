# search-continuation

Replace the shared and daemon continuation/handle stores with one short-lived window in the actual
query/session owner. It keeps an ordered finite result set and the published view needed to read its evidence.
It is not a durable query, corpus-completeness certificate or separate service.

[FUNCTIONS.md](FUNCTIONS.md) owns admission, memory accounting, idle/active expiry, page progress, cancellation
and expansion. [Protocol](../../search-provider-protocol/FUNCTIONS.md) owns the transport. Do not copy another
lifecycle algorithm into this README or treat the old catalog as a completed implementation.

Move useful direct slicing and prepared-output commit with their callers; delete duplicate catalogs, handles,
source fences and receipt/replan machinery. [AGENTS.md](AGENTS.md) describes the integration boundary.
