# search-model-provider

**C12 — Optional model provider contract.**

**Status:** package boundary and agent contract only; behavior is intentionally unimplemented.

Define the isolated optional dense, rerank and multivector provider boundary; no model is selected in the scaffold.

## Owns

- versioned model profile descriptors
- worker request/response contracts
- health, cancellation and resource accounting
- dense/rerank result validation
- uninstall/fallback-to-baseline proof seam

## Must not own

- baseline dependency on a model
- canonical decisions or generative answers
- implicit downloads or network calls
- starting before P15 acceptance and an ADR

- **Delivery wave:** W10 / P16 after accepted P15
- **Soft source-line target:** 6,500
- **Agent instructions:** [AGENTS.md](AGENTS.md)
