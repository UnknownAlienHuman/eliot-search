# Concrete Windows provider byte transport

Own only one non-Clone local client pipe handle and synchronous byte I/O.
The selected name comes from NativeEndpointNameV1. No arbitrary File/handle,
path, host, discovery, fallback, listener, parser, credential or grant API.

Only src/windows.rs may allow unsafe for checked synchronous Kernel32 calls.
Callers do not inherit that exception. The private handle has no clones,
extraction API, background operations or outstanding overlapped I/O.

Connection/type/locality are not authentication. Pairing and policy remain in
the provider/client and daemon owners. Deadline checks observe the caller's
original Instant; no native hard real-time guarantee or async I/O claim.

Focused fixtures belong here. Native/installed qualification remains #332/#215.
