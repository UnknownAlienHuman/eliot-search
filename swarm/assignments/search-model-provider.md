# `search-model-provider` implementation packet

**Path:** `crates/search-model-provider`  
**Capability:** C12 optional dense/multivector/rerank boundary  
**Delivery:** W10 / P16  
**Gate:** BLOCKED until exact accepted P15 + candidate ADR + integration ticket  
**Direct public handoffs:** `search-contracts`, `search-domain`, `search-ports`

Apply `../ASSIGNMENT_PROTOCOL.md`. Read only this assignment, package/root instructions,
`FUNCTIONS.md`, the W10 cross-contract, model profile/settings and exact accepted dependency/API digests
listed in `swarm/w10-optional-depth.toml` and the ticket.

## Mission

Implement provider-neutral profile, input/output validation and bounded encode/rerank semantics for one
exact optional model candidate without selecting a vendor/runtime, launching a worker, touching stores or
creating authority.

## Owns

- immutable complete `ModelProfileDescriptor` and domain-separated digest;
- document/query dense or multivector input preparation contract;
- model-vector shape/finite/layout validation and content-free receipts;
- bounded rerank subset transform and failure policy;
- profile capability/change/migration classification;
- instrumentation and benefit/removal receipt validation seams.

## Must not own

- provider/runtime/model/tokenizer artifact selection in scaffold;
- worker lifecycle, IPC, Windows containment or restart;
- Qdrant/redb/CAS/source/handle/client access;
- Search planning, access, source evidence, exact proof or client disposition;
- generative answers, network/download/update, training/learning or persistent input cache;
- Product Pulse threshold/verdict, shared qualification evidence or G6 acceptance;
- fallback to another model/profile.

## Required operations

See package `FUNCTIONS.md`:

1. profile validation/digest/qualification/capability;
2. document/query batch validation and exact input preparation;
3. document/query encode and vector-output validation;
4. rerank request/output/subset/failure semantics;
5. capability/profile-change and generation classification;
6. instrumentation, benefit-receipt and removal-receipt validation.

## Required invariants

- provider output is candidate nomination/ranking only;
- rerank cannot add a candidate, widen scope or claim completeness;
- exact authorized source/readback remains mandatory;
- access/currentness/shadow/purge apply before model influence;
- dense/multivector create new collection generation;
- rerank-only has no persistent-vector migration but still needs G6/removal;
- no successful partial output after cancellation/timeout/crash;
- no content persistence or content-bearing diagnostics;
- accepted P15 behavior remains independent.

## Exit evidence

Canonical profile/digest goldens; document/query/rerank fixtures; multivector ordering; finite/shape and
subset negatives; access noninterference; bounded resources/cancellation; no network/training/cache;
content-minimization; generation classification; fake worker port; removal/P15 regression seam;
dependency/vendor-type guard.

## Size

Target `src/` <=6,500 lines; split review before 8,500 total; hard stop 10,000 including local tests.
