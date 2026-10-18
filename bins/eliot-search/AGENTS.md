# Agent contract — eliot-search

The CLI owns invocation, connection assembly and presentation in `bins/eliot-search/`.
Use [root instructions](../../AGENTS.md) and the
[execution protocol](../../docs/product/EXECUTION_PROTOCOL.md) for named immediate-consumer cutovers.

## Ownership and reuse

- Capture arguments/configuration once; preserve non-UTF-8 paths and build a closed invocation.
- Assemble one local connection from independently trusted installation/binding coordinates.
- Use `search-provider-client` for pairing/session/framing/correlation/cancellation semantics.
- Build canonical recipe/grant requests; render bounded human or JSON/NDJSON output and truthful exits.
- Reuse the existing Windows transport and purpose-bound Credential Manager owners through their
  narrow APIs. The CLI does not implement secret storage or choose arbitrary credential targets.

The actual dependency graph is in [Cargo.toml](Cargo.toml): contracts, ports, config, provider protocol
and typed client, plus the existing target-specific transport/secret adapters. Reuse those owners;
new external dependencies/pins require the existing manager review. No second client engine.

## Boundaries

Never open redb, revision CAS or Qdrant, implement query/access policy, mint or widen a grant,
decode opaque handles, expose hidden payloads, or use pipe reachability as authentication.
Do not enable `legacy-loopback-harness` for product startup or add TCP/endpoint-file/token-file fallback.
Native input/listener/command linkage and deletion candidates are in [FUNCTIONS.md](FUNCTIONS.md);
[#332](https://github.com/UnknownAlienHuman/eliot-search/issues/332) owns the remaining cutover.
Preserve supported DIRECT commands while replacing their named connection/dispatch edges.

Use the root production checks and deferred evidence rules. Count removed mechanisms and remaining
callers; splitting files or adding forwarding crates to satisfy a line target is not progress.
