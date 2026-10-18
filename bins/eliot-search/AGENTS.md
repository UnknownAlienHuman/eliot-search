# Agent contract — eliot-search

The CLI owns invocation, connection assembly and presentation in `bins/eliot-search/`.
Use [root instructions](../../AGENTS.md) and the
[implementation guide](../../docs/implementation/README.md) for named immediate-consumer cutovers.

The [implementation guide](../../docs/implementation/README.md#4-simplify-and-connect-the-query) connects
trusted child-stdio/OS-owned pipes separately from optional restricted-client pairing. Old grant/proof fields are not mandatory local
bootstrap; remove obsolete requirements with their real callers, never fake permissive credentials.

## Ownership and reuse

- Capture arguments/configuration once; preserve non-UTF-8 paths and build a closed invocation.
- Assemble one owned local connection. Persistent endpoints need actual OS/root identity; restricted
  clients additionally use their real installation/binding authority.
- Reuse the existing client/framing/correlation/cancellation code. Keep one session implementation.
- Build explicit scoped requests; render bounded human or JSON/NDJSON results. Grants belong to an
  implemented restricted-client profile, not all same-user reads.
- Reuse the existing Windows transport when a persistent pipe is selected. Use the concrete credential
  owner only for a restricted integration that needs secrets; do not require it for child-stdio startup.

The actual dependency graph is in [Cargo.toml](Cargo.toml): contracts, ports, config, provider protocol
and typed client, plus target-specific transport/secret adapters. Reuse useful code and replace its
obsolete constructors with their callers. Add direct serialization dependencies where they remove the
manual grammars; no second client engine is needed.

## Boundaries

Never open redb, revision CAS or Qdrant, implement query/access policy, mint or widen a grant,
decode opaque handles, expose hidden payloads, or use pipe reachability as authentication.
Do not enable `legacy-loopback-harness` for product startup or add TCP/endpoint-file/token-file fallback.
Native input/listener/command linkage and deletion candidates are in [FUNCTIONS.md](FUNCTIONS.md);
[#332](https://github.com/UnknownAlienHuman/eliot-search/issues/332) owns the remaining cutover.
Preserve supported DIRECT commands while replacing their named connection/dispatch edges.

Use the root production and behavior checks. Describe removed mechanisms and remaining
callers; splitting files or adding forwarding crates to satisfy a line target is not progress.
