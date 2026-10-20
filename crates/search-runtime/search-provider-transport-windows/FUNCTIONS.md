# Windows pipe effect boundary

`NativeLocalConnection` is optional infrastructure for a selected persistent local pipe. The baseline
child-stdio client does not depend on it.

## Retained API, if selected

```text
open(admitted_endpoint, absolute_deadline) -> NativeLocalConnection
read_once(buffer, absolute_deadline/cancel) -> bytes | would-block | EOF
write_once(bytes, absolute_deadline/cancel) -> count | would-block
close/drop
```

Concrete names may stay while useful. Rules:

1. The caller/platform owner supplies an already admitted endpoint and real OS-owner/installation decision.
2. Open exactly once; no discovery, retry loop, alternate endpoint, TCP or token-file fallback.
3. Retain one non-clone handle; no arbitrary public `File`/raw-handle constructor or extraction.
4. Validate byte-pipe/local shape available from Windows. Pipe locality/name alone is not authentication.
5. Partial I/O advances only the reported bytes. `WouldBlock` and EOF are distinct.
6. All polling uses the caller's original absolute deadline/cancellation; no phase renews time.
7. `flush` is no peer acknowledgement and no durability/rollback evidence.
8. Drop closes only this handle and claims no remote effect rollback.
9. Errors are stable/content-free; raw pipe paths, handles and source/query bytes do not enter diagnostics.

Protocol framing, request correlation, cancellation, partial-output fail-stop and one terminal response belong
to `search-provider-protocol`/client. Restricted-client credentials/session policy belong to their selected
adapter.

## Deletion boundary

If #332 ships only child stdio, remove:

- this package;
- native client registration/credential/end-point resolver used only for it;
- daemon native pipe listener/opening/grant composition;
- old pairing/profile expectations and compatibility fixtures.

If retained, remove all APIs beyond the small byte-stream effect above. Do not make the optional pipe a
second protocol or product-readiness gate.
