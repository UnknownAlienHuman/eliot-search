# Indexed boundaries — navigation, not a second schema

The historical sparse/vector/validity contract is superseded. Do not implement it beside the current profile.
[Shared contracts](FUNCTIONS.md) owns cross-owner data; concrete indexed algorithms live at these owners:

| Question | Owner |
|---|---|
| Physical text index versus query scoring | [Lexical](../search-lexical/FUNCTIONS.md) |
| Generation-bound addresses and collision evidence | [Point identity](../search-index-qdrant/search-point-identity/FUNCTIONS.md) |
| Exact Qdrant schema, payload and RPC translation | [Bridge](../search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md) |
| Fixed build input, visibility, recovery and cleanup | [Publication](../search-index-qdrant/search-publication/FUNCTIONS.md) |
| Pinned evidence lookup | [Control](../search-control-redb/FUNCTIONS.md) |
| Empty/unknown selection and candidate/IDF filters | [Scope](../search-query/search-access/FUNCTIONS.md) |

Stage 2/3A has no required vectors, occurrence incarnations, sequence or validity interval. Points are immutable
within one never-reused physical collection. Conditional Stage 3B cannot introduce those fields before its
measured need and consumers exist. Neither profile physically deletes individual points.

A Qdrant hit is a nomination. Its address is not the full identity, and payload is not retained source evidence.
Known empty selection returns without RPC; unknown selection fails instead of widening. Do not reinstate the
old empty-is-error rule, provider-neutral field table or schema/qualification digests.

The [prior contract](https://github.com/UnknownAlienHuman/eliot-search/blob/44ee1661833c1139f1e0f1486f365a3a523fe814/crates/search-contracts/INDEXED_CONTRACT.md)
is historical evidence only. Physical/query profile rules and validation cases must not be copied here again.
