# `eliot-search-doc-worker` implementation packet

**Path:** `bins/eliot-search-doc-worker`  
**Capability:** isolated optional document materializer  
**Delivery:** W10 / P17  
**Gate:** BLOCKED until exact accepted P15 + provider ADR/profile/artifact ticket  
**Direct public handoffs:** `search-contracts`, `search-ports`, `search-provider-protocol`, `search-materializer`

Read only package/root instructions, this assignment, `FUNCTIONS.md`, W10 cross-contract, document
profile/settings and exact accepted handoffs in the ticket.

## Mission

Host one exact qualified document provider in a private no-execute/no-network process with bounded
container/resources, validated coordinates/loss maps, malformed-input isolation and verified cleanup.

## Owns

- provider/profile/artifact and inherited sandbox verification;
- private daemon-only IPC;
- bounded MIME/container/member/page/object/image/decompression inspection;
- no-execute/no-network/remote-resource/path containment enforcement;
- provider dispatch and output/coordinate/loss-map validation seam;
- cancellation, crash isolation, temp cleanup and process removal.

## Must not own

- source acquisition, revision store, materialization meaning, Qdrant/publication or clients;
- provider selection in scaffold;
- scripts/macros/OLE/hooks/filters/shell/child process/remote resources;
- path/reparse escape or unbounded archives/pages/objects/images/output;
- Product Pulse threshold/verdict, shared qualification evidence or G6 acceptance;
- Python/Node runtime without explicit ADR and exact qualification.

## Required operations

See `FUNCTIONS.md`: validate profile/sandbox, load provider, private session, request admission, safe input
inspection, materialize/output validation, cancel/terminal/cleanup, health/drain/shutdown and crash/retry.

## Exit evidence

Binary absent baseline; exact Windows artifact/license; private IPC; no store/index access; exact retained
input; script/network/remote/path denial; bomb and malformed/fuzz corpus; coordinate/loss-map/assurance
goldens; finite resources/cancellation; no unverified temp output; content-minimized process surfaces;
worker cleanup and accepted P15 regression after removal.

## Size

Target `src/` <=5,000 lines; split review before 8,500 total; hard stop 10,000 including local tests.
