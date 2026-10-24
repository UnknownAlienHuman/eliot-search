# Configuration

Configuration supplies values to actual application owners. The typed replacement is selected but not yet
implemented: the current daemon still uses its legacy decoder, in-code registry and receipt-gated startup.

## Implementation entrypoints

| Question | Single owner |
|---|---|
| What the present code consumes | [Config caller and field map](../../crates/search-config/FUNCTIONS.md#observed-callers-and-startup-refusal) |
| Partial sections, precedence, lists and validation | [Typed layers](../../crates/search-config/FUNCTIONS.md#sparse-layers-and-validation) |
| OS paths, TOML encoding and useful errors | [Path and diagnostic policy](../../crates/search-config/FUNCTIONS.md#paths-and-diagnostics) |
| File capture and startup | [Capture recipe](../../crates/search-config/FUNCTIONS.md#capture-implementation) |
| Live change, restart and published profiles | [Runtime-change semantics](../../crates/search-config/FUNCTIONS.md#runtime-change-semantics) |
| Dependencies and verified API limits | [Donor evidence](../audit/DONOR_VERIFICATION_REGISTER_2026-10-09.md#typed-configuration-donor-boundaries) |

[#333](https://github.com/UnknownAlienHuman/eliot-search/issues/333) changes the concrete settings and their
actual consumers together. A runnable example ships with that change. The old example contained only legacy
parser headers and did not configure a usable Search application.

The unused [wave/section catalogues](https://github.com/UnknownAlienHuman/eliot-search/tree/afc3bfca60cd0a350c3cf822ccda48f5a13e2028/config)
and [versioned guides](https://github.com/UnknownAlienHuman/eliot-search/tree/afc3bfca60cd0a350c3cf822ccda48f5a13e2028/docs/config)
are preserved in Git history. They are not current file schemas, generated API inputs or requirements to
recreate a section registry. Concrete source, storage, query, publication and maintenance rules remain with
their [implementation owners](../implementation/README.md#owners).
