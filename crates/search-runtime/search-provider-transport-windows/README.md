# Windows provider transport — delete with the old provider client

This package is one small synchronous **client-side** named-pipe adapter. It opens an endpoint described by
the old provider protocol, exposes `Read + Write`, translates a few pipe errors and uses `PIPE_NOWAIT`
polling. Production source is about 8.4 KiB before tests.

Repository callers are the old typed provider client, its example and the unconnected CLI native-pairing
path. There is no server/listener implementation here and no caller on the selected child-stdio path.

The current architecture uses owned child stdin/stdout first. Therefore remove this package and its workspace
member with #280/#332 and the old provider client. Do not preserve it merely because a future persistent pipe
might exist.

If a persistent Windows daemon later has a measured need, implement the smallest transport at that time:

```text
actual owner-only named-pipe server/listener
+ real ACL/peer-owner validation
+ one owned client stream
→ current u32-length + Serde protocol
```

That implementation may reuse the narrow Win32 error/handle lessons, but it must not depend on
`NativeEndpointNameV1`, pairing keys, grants, capability digests, endpoint files or legacy session types.
Transport reachability alone does not grant source scope; the daemon still resolves every request against its
pinned view and optional restricted-session policy.

No TCP/token-file fallback, discovery framework, reconnect pool or generic transport plugin is required.
