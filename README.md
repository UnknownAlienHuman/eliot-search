# ELIOT Search

ELIOT Search is a standalone local search application for large codebases and document collections. It returns
ranked source-backed locations with verified excerpts, exact retained revision identity and explicit gaps. Qdrant
is derived retrieval state; immutable objects remain the evidence source.

## Read first

| Need | Document |
|---|---|
| Product purpose and invariants | [Architecture](docs/architecture/README.md) |
| Shortest implementation path | [Implementation spine](docs/implementation/README.md) |
| Current checkpoint and evidence | [R053 registry](docs/audit/ANALYSIS_REGISTRY.md#resume) |
| One concrete cut | Nearest `FUNCTIONS.md`, actual callers and owning issue |

[Documentation authority](docs/README.md) explains which layer wins. Historical waves, receipts and old issue text
are evidence only.

## Current truth

The product is **not implemented end to end**. R053 preserves the explicit Qdrant 1.20 text profile from R052 and
corrects the Windows finalization boundary after tracing the currently reachable native source-registration route.
It does not claim compiler or runtime success.

```text
1 Foundation
  root/plain settings → native-canary-backed final-manifest-last root → DataRootGuard

2 Walking skeleton
  one file → ObjectRef → retrieval_text
  → vectorless Qdrant 1.20 collection
  → explicit scoring Text index V1
  → InsertOnly points + deterministic TextQuery canary
  → PublishedView
  → TextQuery(k=1.2,b=0.75) → retained excerpt → CLI

3A correctness/reference
  restart + inventory + explicit filter/residency profile
  + complete fresh generations + release budgets

3B only where budgets fail
  source-local diff + stable occurrence identity + point additions/closures
```

The Cargo lock still contains `qdrant-client 1.19.0`; server/client/schema/query upgrade together in #263/#264.

## Exact lexical baseline

Physical Text index V1:

```text
field=retrieval_text
Code tokenizer
lowercase=true
ascii_folding=false
phrase_matching=false
stemmer=Disabled
stopwords/min/max/nested absent
BM25 scoring
enable_hnsw=false
memory=Pinned for Stage 2
```

Stage 2 creates no source/access filter indexes because one file is the whole population. Stage 3A adds only real
filter indexes and explicitly selects `Pinned|Cached|Cold` residency per measured corpus profile.

Query scoring is separate: baseline k=1.2, b=0.75, finite/range validated, no score threshold. Changing only k/b
does not rebuild the collection. Qdrant responses contain no vectors and only routing payload required for pinned
evidence lookup; excerpts always come from immutable objects.

## Windows root/object boundary

- Path-based `tempfile::persist_noclobber` is not trusted publication authority on Windows.
- The existing legacy `registration_native::rename` is also not a donor: it routes a relative name through the
  Win32 `SetFileInformationByHandle` wrapper and is reachable from legacy source-root commands.
- Foundation/object publication requires one owner-private direct native rename canary tied to the opened stage and
  retained destination parent, followed by exact reopen/readback.
- Unsupported native/API/filesystem profiles fail closed. There is no path-only fallback or general filesystem
  framework.

## Other boundaries

- Plain storage is baseline; #598 encryption is optional.
- `PublishedView` pins exact collection + physical profile + redb snapshot; Stage 2/3A has no sequence.
- Complete immutable collections and generation-bound point IDs precede point optimization.
- Whole-collection cleanup only in baseline.
- No aliases, custom-shard portfolio, Edge fallback, query scheduler/tickets or second backend.
- No redb writer/view/window lock spans backend/object/process/model I/O.

## Verification status

Only R031 has current compiler evidence: locked Rust 1.98 check/minimal Clippy for materializer + unitizer. Current
binaries, Foundation, native Windows finalizer, Qdrant 1.20, explicit text-index profile, walking skeleton, restart,
relevance, scale and end-to-end behavior remain unrun. See the
[registry](docs/audit/ANALYSIS_REGISTRY.md#evidence-status).

[QUICKSTART.md](QUICKSTART.md) remains an unverified diagnostic route until #264 works.

## License

See [LICENSE](LICENSE).
