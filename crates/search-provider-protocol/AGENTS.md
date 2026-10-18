# Local request and result protocol

Read the [architecture](../../docs/architecture/README.md),
[implementation guide](../../docs/implementation/README.md) and
[query cutover](../search-query/README.md#direct-operations-not-a-recipe-operating-system).

Own one bounded typed exchange used by the actual CLI and daemon. Keep frame sizing, request correlation,
cancellation and one terminal result. Do not build another host/session/capability framework.

Replace the eleven V1 recipes and parallel V2 endpoint with one current request/result family. Update
codec, dispatch, handler and client together; no V3 wrapper or old-byte compatibility layer.

The trusted-local path uses owned stdio or an owner-only pipe. It does not need paired hello, grants,
body MACs, capability credentials or registry revisions. A selected restricted-client profile owns real
credentials, replay checks and delegated scope separately.

Capabilities, if exposed, are a simple projection of constructed handlers and useful limits. They are not
request authority and need no canonical digest. Unsupported operations fail clearly.

Use typed Serde on complete bounded frames. Bound serialized output, queues and in-flight requests. Test
through the application. Historical recipe counts, provider ceremonies, waves and all-profile gates do
not block local integration.
