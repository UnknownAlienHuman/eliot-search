# `eliot-search-model-worker` implementation packet

**Path:** `bins/eliot-search-model-worker`  
**Capability:** isolated optional model process  
**Delivery:** W10 / P16  
**Gate:** BLOCKED until exact accepted P15 + model ADR/profile/artifact ticket  
**Direct public handoffs:** `search-contracts`, `search-ports`, `search-provider-protocol`, `search-model-provider`

Read only package/root instructions, this assignment, `FUNCTIONS.md`, W10 cross-contract, model profile/
settings and exact accepted handoffs in the ticket.

## Mission

Host one exact qualified local model profile over private daemon-only IPC with finite resources,
cancellation, content minimization, crash isolation and verified cleanup.

## Owns

- startup/artifact/profile and inherited containment verification;
- private worker protocol/session/request lifecycle;
- finite queue/concurrency/CPU/GPU/memory/deadline/cancel enforcement;
- accepted provider dispatch and bounded output framing;
- health/pressure/drain/shutdown and process/content-cache cleanup.

## Must not own

- model semantics/profile validation owned by `search-model-provider`;
- artifact/provider selection in scaffold;
- stores, Qdrant, source inventory, handles, secret store or external clients;
- route/config/capability activation or Product Pulse/G6 verdict;
- network/download/update, training/learning, persistent input cache or generative authority.

## Required operations

See `FUNCTIONS.md`: validate startup/containment, load exact provider, open private session, admit/serve
encode and rerank, cancel/terminal, health/pressure, drain, shutdown/remove and crash/retry classification.

## Exit evidence

Feature/binary absent baseline; exact artifact reopen; private auth/replay/frame/in-flight; finite resource
matrix; encode/rerank guards; cancellation/late callback/one terminal; no content in process surfaces;
no network/training/cache; crash isolation; bounded restart/quarantine; process/temp/cache cleanup; P15
baseline removal fixture.

## Size

Target `src/` <=4,500 lines; split review before 8,500 total; hard stop 10,000 including local tests.
Keep `main` thin; model behavior stays in the library/provider implementation selected by ADR.
