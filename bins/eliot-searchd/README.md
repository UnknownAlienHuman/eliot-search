# eliot-searchd

**Composition binary — sole owner of Search stores, local Qdrant process and provider server.**

**Status:** partial DIRECT composition exists. Primary redb control, canonical durable preparation,
real Qdrant and full provider integration remain unfinished and unqualified.

## Owns

- data-root owner acquisition and progressive startup
- concrete redb/OS-secret/Qdrant adapter construction
- vendor-neutral port wiring
- bounded request supervision and provider server
- shutdown/readiness/degradation reporting

## Must not own

- capability logic
- shared clients for CLI/workers/adapters
- reverse adapter edges into query/lifecycle packages
- hidden fallback or client canonical writes

- **Delivery wave:** W1 shell, integrated progressively through W9
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)

## DIRECT proxy child bounds

`--serve-loopback-data-root` retains one process owner and one pipe worker. Startup/READY has a
30-second budget; each command has one 120-second budget across pipe writing, reply reading, client
forwarding and shutdown exit confirmation. Normal exit/forced cleanup has a 5-second budget. The
worker queue and each operation-specific reply channel have capacity one. No command is replayed.

Frames retain the 64 KiB ceiling. A reply also has a 64 MiB total-byte ceiling including newlines;
shutdown output is retained within 128 KiB until the exact child exits successfully and its pipe
worker joins. A STOPPED frame alone cannot acknowledge shutdown. Timeout or ambiguous output closes
the socket, terminates the child and prevents later admission. Ordinary complete errors stay reusable.

Cleanup uses bounded polling, not an unconditional `Child::wait` or thread join. An OS failure to
reap a child or close inherited pipe handles remains failed/unknown cleanup, never clean owner
release. The proxy exits rather than silently retaining a serving worker. This is not process-tree
containment or proof against an uninterruptible OS syscall; native owner/job qualification remains.

