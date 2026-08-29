# W3 implementation packet index

W3 creates the first indexed lexical product, but this directory and the package `FUNCTIONS.md` files
do not authorize implementation. `swarm/launch-state.toml` remains authoritative.

## Package packets

| Package | Functions | Configuration / qualification |
|---|---|---|
| `search-lexical` | `crates/search-lexical/FUNCTIONS.md` | `config/sections/lexical.md`, `qualification/qdrant/W3_QUALIFICATION.md` |
| `search-point-identity` | `crates/search-index-qdrant/search-point-identity/FUNCTIONS.md` | canonical point-key fixtures |
| `search-projection-planner` | `crates/search-index-qdrant/search-projection-planner/FUNCTIONS.md` | collection schema / exact manifest |
| `search-qdrant-supervisor` | `crates/search-index-qdrant/search-qdrant-supervisor/FUNCTIONS.md` | `config/sections/qdrant_process.md`, qualification artifact/process |
| `search-qdrant-bridge` | `crates/search-index-qdrant/search-qdrant-bridge/FUNCTIONS.md` | `config/sections/qdrant_data.md`, probe/schema suite |
| `search-publication` | `crates/search-index-qdrant/search-publication/FUNCTIONS.md` | publication failpoint matrix |
| `search-epoch-pins` | `crates/search-index-qdrant/search-epoch-pins/FUNCTIONS.md` | route/epoch pin fixtures |
| `search-index-reclaimer` | `crates/search-index-qdrant/search-index-reclaimer/FUNCTIONS.md` | `config/sections/index_reclaim.md`, exact-delete fixtures |

## Dependency order

```text
accepted W2 unit/revision contracts
  ├─ search-point-identity
  ├─ search-lexical
  └─ qualified Qdrant supervisor + bridge
       ↓
search-projection-planner
       ↓
search-epoch-pins
       ↓
search-publication
       ↓
search-index-reclaimer
```

The integration owner may parallelize only independent nodes whose direct handoffs are accepted by exact
commit/API digest.

## Hard stop conditions

- any missing base-filter payload index;
- missing-field or signed-epoch fixture mismatch;
- filtered IDF unavailable or access-noninterference failure;
- server/client/artifact identity not exact;
- plaintext secret side channel;
- point collision overwrite path;
- broad-filter close/delete on a correctness path;
- mutation success without exact acknowledgement/readback;
- publication visibility outside the guarded control commit;
- reclaim while any route/epoch pin is active;
- configuration/registry/Cargo dependency drift.

Any stop condition leaves indexed mode unavailable. It does not permit a local workaround.
