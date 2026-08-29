# search-model-provider

**C12 — Optional model profile and scoring boundary.**

**Status:** complete W10/P16 function contract; no model/runtime/provider selected or implemented.

Owns versioned dense/multivector/rerank profiles, bounded input/output validation, provider-neutral
worker requests, content-free receipts, migration classification and removal validation.

Does not own worker lifecycle, stores/index, query/access authority, source evidence, generative answers,
network/download/update/training/cache behavior or G6 acceptance.

- **Delivery:** W10/P16 only after exact accepted P15 + ADR + qualification.
- **Default:** absent/disabled.
- **Soft source target:** 6,500 lines.
- **Functions:** [FUNCTIONS.md](FUNCTIONS.md)
- **Agent instructions:** [AGENTS.md](AGENTS.md)
- **Cross-contract:** [`../../docs/optional/W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md`](../../docs/optional/W10_OPTIONAL_DEPTH_CONTRACTS_1.0.md)
