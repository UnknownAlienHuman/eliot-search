# Historical port catalogue

This document formerly assigned one global `search-ports` trait to every planned capability and treated the
trait/implementation matrix as architectural closure. That model is retired.

Current dependency ownership is described in [DEPENDENCY_GRAPH.md](DEPENDENCY_GRAPH.md), the
[architecture manifesto](../architecture/README.md) and the
[implementation guide](../implementation/README.md).

## Current rule

Concrete effect owners expose the smallest API their real consumers use:

```text
source/object owner   → acquisition, immutable read/write, current source head
redb owner            → typed current-state transactions and reads
preparation owner     → materialize/unitize/project
Qdrant bridge         → concrete schema/mutation/query/readback
publication owner     → pending/current/cleanup transitions
query owner           → scope/context/evidence/window
protocol owner        → current bounded request/result frames
```

There is no mandatory global port registry, implementation-owner table, conformance fake catalogue or rule
that all operations carry the same receipt/mutation/context shape.

A narrow trait may live beside a consumer only when that consumer currently needs multiple implementations.
It is removed with the alternate implementation. Vendor types remain private where useful, but that does not
require a universal abstraction package.

## Historical disposition

The 23-port / 80-operation catalogue at public main commit
`c8e38b6fc9829f1cd38070773b1b3e5b21de18ad` remains source evidence for why the project expanded. It is not
an implementation specification and must not be regenerated as current architecture.

`search-ports`, generated `swarm/coverage/ports.toml` consumers and package maps are deleted after current
owner cutovers. Missing entries do not block product work; real connected behavior is the authority.
