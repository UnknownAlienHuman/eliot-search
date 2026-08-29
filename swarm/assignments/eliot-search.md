# `eliot-search` implementation packet

**Path:** `bins/eliot-search`  
**Capability:** composition binary  
**Delivery:** W1 shell; commands added by owner waves  
**Gate:** W1 shell only after provider framing; command families appear only with accepted server capabilities  
**Trace:** S33, H11.6, H16, P01-P15  
**Direct public handoffs:** `search-contracts`, `search-provider-protocol`

Apply `../ASSIGNMENT_PROTOCOL.md`. Logical names below express required semantics, not mandatory Rust
spelling.

## Mission

Provide a thin standalone CLI over the provider protocol; it never opens Search stores or Qdrant directly.

## Owns

- argument parsing and command-to-envelope mapping
- standalone pairing/session setup
- progress/result/error rendering
- doctor command request construction and explicit exit codes

## Must not own

- opening redb, CAS or Qdrant
- implementing search/source/publication logic
- inventing a second protocol
- turning degraded/partial responses into success

## Logical primitives

- CliCommand, CliRequest, CliOutputMode, ExitStatus, ProgressRenderer, ErrorRenderer, DoctorCommand

## Logical operations

1. `parse_args(args) -> Result<CliCommand, CliError>`
2. `compile_request(command, binding) -> Result<ProviderEnvelope, CliError>`
3. `run_session(command, transport) -> Result<ExitStatus, CliError>`
4. `render_result(envelope, mode) -> RenderedOutput`
5. `map_reason_to_exit_status(reason) -> ExitStatus`

## Required invariants

- all operations go through daemon protocol
- CLI sends typed recipes/commands, never raw Qdrant filters
- partial/degraded coverage is visible in output and exit semantics
- credentials/secrets/source bodies are not logged
- doctor mutations remain server-authoritative/idempotent

## Typed failure surface

- `CLI_USAGE_ERROR`
- `DAEMON_UNAVAILABLE`
- `PROTOCOL_VERSION_MISMATCH`
- `REQUEST_FAILED`
- `PARTIAL_RESULT`
- `COMMAND_NOT_SUPPORTED`

## Exit tests / evidence

- `dependency_guard_no_store_or_qdrant`
- `typed_request_golden`
- `partial_result_rendering`
- `protocol_error_exit_codes`
- `doctor_command_no_direct_store`
- `secret_redaction`

## Suggested internal modules

```text
eliot-search/src/
  args.rs
  command.rs
  request.rs
  session.rs
  render.rs
  exit.rs
  main.rs
```

This is an internal file plan, not a request for more crates.

## Size / split

- Initial `src/` target: **≤ 4,500 hand-written lines**.
- Split review: **before 8,500 total hand-written Rust lines**.
- Hard stop: **10,000 including package-local tests**.
- Keep thin. New command behavior belongs to owning server packages; split only a separately shipped UI frontend.

The handoff must let a downstream agent consume the public contract without reading implementation
internals or the architecture master.
