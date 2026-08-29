# Agent contract — search-model-provider

Own only `crates/search-model-provider/**`. Do not edit workers, daemon, Cargo root, shared contracts,
qualification evidence, configuration registry or architecture. Missing load-bearing fields use the
contract-change process.

## Gate and read set

This W10/P16 package is blocked until an integration ticket supplies the exact accepted P15 receipt,
candidate ADR, selected profile/artifacts, G6 evidence plan and accepted direct handoffs.

Read only root/package instructions, package assignment, `FUNCTIONS.md`, the W10 cross-contract,
`model-profile.toml`, W10 settings and exact accepted dependency/API digests listed by the ticket.

## Ownership

- immutable provider-neutral model profile identity;
- document/query dense or multivector input/output contracts;
- bounded rerank subset transform;
- output/profile validation and content-free receipts;
- profile-change/migration classification;
- instrumentation and removal-plan validation seams.

## Forbidden ownership

- worker process, model/runtime/vendor implementation or artifact selection in scaffold;
- Qdrant/redb/CAS/source/handle/client access;
- query scope, access, evidence, exact proof or client disposition;
- generative answers, network, download/update, training/learning or persistent content cache;
- G6 benefit verdict or self-acceptance;
- fallback to another provider.

## Required invariants

- provider output is nomination/ranking only;
- rerank output is a subset of its bounded input;
- inaccessible/stale/denied/purged/shadowed content cannot influence output;
- exact profile identity covers all tokenizer/runtime/vector/rerank/resource behavior;
- dense/multivector require new collection generation;
- cancellation/timeout/crash produces no successful partial output;
- P15 baseline works when provider is absent or fails;
- unsaved/source/query content is process-memory-only and absent from diagnostics.

## Size

Target `src/` <=6,500 lines; split review before 8,500 total; hard stop at 10,000 including local tests.
No crate split without a real dependency/replacement/security/test boundary and ADR.
