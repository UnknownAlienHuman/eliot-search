# Agent contract — eliot-search-model-worker

Own only `bins/eliot-search-model-worker/**`. Keep the binary a thin isolated host for the accepted
`search-model-provider` contract. Do not edit libraries, daemon, root Cargo, shared evidence or
architecture.

## Gate and read set

Blocked until a candidate-specific ticket supplies accepted P15, ADR, exact model/runtime/worker profile,
Windows qualification and accepted direct handoffs. Read only package instructions/assignment,
`FUNCTIONS.md`, W10 cross-contract, model profile/settings and ticketed API digests.

## Ownership

- exact worker startup/profile/artifact verification;
- private daemon-only IPC session and request lifecycle;
- finite queue/concurrency/CPU/GPU/memory/deadline/cancellation policy;
- provider dispatch, output framing and health;
- drain, crash isolation, cleanup and verified process removal.

## Forbidden ownership

- model meaning/profile validation already owned by `search-model-provider`;
- stores, Qdrant, source inventory, handles, secret store or external client endpoint;
- network, auto-download/update, training/learning or persistent input cache;
- generative/client authority;
- route/config activation or G6 self-acceptance.

## Invariants

Worker absent/stopped by default; exact one-profile identity; content process-memory-only; exactly one
terminal response; cancellation/timeout/crash leaves no accepted partial output; bounded restart ends in
quarantine; removal verifies process/context/temp/cache cleanup; P15 baseline remains available.

## Size

Target `src/` <=4,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
Behavior belongs in the model-provider library; `main` and wiring stay thin.
