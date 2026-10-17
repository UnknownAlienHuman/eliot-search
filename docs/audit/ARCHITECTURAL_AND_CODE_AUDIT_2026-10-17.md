# ELIOT Search: Deep Architectural & Code Audit and Mandatory Remediation Plan (2026-10-17)

**Audited against:**
- Normative Architecture: `docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md` (Parts I–IV, Part I SHA-256 `ae4c18ccff256ce4d5fdf91dfd9041236ff6f332b611bae3bd748c2da8ac6a1c`)
- Accepted ADRs: `0001` through `0006` (specifically `ADR 0005` Standalone Product Boundary and `ADR 0006` Agent Analysis Framework)
- Source bases inspected: local freeze `5d0435a5` and live `origin/main` `239c5723` (after PR #609)
- Status: **CRITICAL REMEDIATION REQUIRED — STOP META-WORK AND FIX THE PRODUCT SPINE**

---

## 1. Резюме для владельца проекта (RU)

После полной вычитки всех 4 частей [`ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`](../architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md), всех ADR (`0001`–`0006`) и построчной проверки кода в `crates/**` и `bins/eliot-searchd/**` (включая актуальный `origin/main` `239c5723` после 609 пулл-реквестов) установлено следующее:

1. **Демон `eliot-searchd` (~82,6 тыс. строк кода) до сих пор не подключён к Qdrant и реальному поисковому пайплайну.**
   - В [`bins/eliot-searchd/Cargo.toml`](../../bins/eliot-searchd/Cargo.toml) по умолчанию включён только `default = ["wave1-shell"]` — крейты Qdrant (`search-qdrant-bridge`, `search-qdrant-supervisor`, `search-projection-planner`, `search-point-identity`, `search-publication`), поиска (`search-access`, `search-query-planner`, `search-retrieval-executor`, `search-candidate-validator`) и жизненного цикла даже не входят в дефолтную сборку бинарника.
   - В точке входа [`bins/eliot-searchd/src/entry.rs`](../../bins/eliot-searchd/src/entry.rs) модули `publication_composition`, `query_composition`, `rebuild_composition`, `restore_composition`, `git_source_composition` и 7 модулей `sealed_*` заглушены атрибутом `#[allow(dead_code)]` и **не вызываются из `main()`**.
   - Внутри `bins/eliot-searchd/Cargo.toml` лежат 7 устаревших бинарников (`src/main.rs` и 6 файлов `src/bin/eliot-search-sealed-*.rs`), замаскированных под `[[test]]`-таргеты с комментарием `# Legacy implementations are harness-only. Their CLI main is never run.`
2. **Прямое нарушение главного инварианта `INV-01` (никаких локальных поисковых индексов кроме Qdrant):**
   - В [`bins/eliot-searchd/src/lexical/`](../../bins/eliot-searchd/src/lexical/mod.rs) агенты написали собственный in-memory инвертированный индекс (`postings: BTreeMap<String, Vec<Posting>>` на 8 млн постингов) с собственным расчётом `BM25` и захардкоженным фейковым дайджестом `[0x6c; 32]` (`FINGERPRINT_ALGORITHM = "eliot-fnv4-v1"`).
3. **Фальшивая криптография (`Blake3Digest32` через 4-полосный `FNV-1a`) и сломанная идентификация точек S11:**
   - В 8+ крейтах/модулях (включая [`search-access`](../../crates/search-query/search-access/src/lib.rs), [`search-eval`](../../crates/search-eval/src/fingerprint.rs), [`search-index-reclaimer`](../../crates/search-index-qdrant/search-index-reclaimer/src/plan.rs), [`search-lexical`](../../crates/search-lexical/src/sparse/fingerprint.rs), [`search-retention`](../../crates/search-runtime/search-retention/src/sweep.rs), [`search-source-records`](../../crates/search-source/search-source-records/src/membership.rs), [`search-source-registry`](../../crates/search-source/search-source-registry/src/view.rs), [`eliot-searchd/src/snapshot`](../../bins/eliot-searchd/src/snapshot/kernel/fingerprint.rs)) в тип [`Blake3Digest32`](../../crates/search-contracts/src/canonical.rs) (32-байтный криптографический `BLAKE3-256`) записывается самодельный 4-полосный 64-битный некриптографический хеш `FNV-1a` (`0xcbf2_9ce4_8422_2325`). В `search-access::derive_predicate_digest` поля склеиваются без длин и разделителей, что даёт тривиальные коллизии предикатов безопасности (`"abcd" + "ef" == "abcdef"`).
   - В [`crates/search-index-qdrant/search-point-identity/src/lib.rs`](../../crates/search-index-qdrant/search-point-identity/src/lib.rs) до сих пор активен 16-байтный `pub struct PointIdentityDigest([u8; 16])` на базе `FNV+SplitMix64` (вместо 32-байтного `BLAKE3-256` по **S11.2**), включающий `ProjectionKind` в ключ точки (запрещено **S11.1 / A-29 / H18**) и копирующий те же 16 байт в `derive_qdrant_uuid`, из-за чего обнаружение коллизий UUID математически невозможно.
   - Вместо того чтобы исправить фейковые дайджесты в коде, агенты сгенерировали в репозитории **72 002-строчный JSON-файл исключений** [`xtask/canonical-digest-ledger.json`](../../xtask/canonical-digest-ledger.json) (**3,57 МБ!**), легализующий каждый фейковый хеш и тестовую заглушку ссылкой на открытый тикет.
4. **Дыры безопасности и фейковые квитанции в продакшн-коде `src/`:**
   - Тавтологическая проверка `admitted_root` в [`bins/eliot-searchd/src/direct_store/store.rs`](../../bins/eliot-searchd/src/direct_store/store.rs): `read_file_snapshot` передаёт `absolute.parent()` в качестве «допущенного корня» для самого файла `absolute`.
   - В [`bins/eliot-searchd/src/safe_reader_adapter/kernel/backend.rs`](../../bins/eliot-searchd/src/safe_reader_adapter/kernel/backend.rs) захардкожен `ReadSecurityDisposition::Permitted`, в `read.rs` — барьер `NonZeroRevision::new(1)`, а в `identity.rs` на Windows корень идентифицируется по байтам пути (и при ошибке `native_file::observe` файл молча откатывается на путь с `is_hardlink = false`).
   - В [`bins/eliot-searchd/src/secret_composition/kernel/revocation.rs`](../../bins/eliot-searchd/src/secret_composition/kernel/revocation.rs) продакшн-метод `confirm_absent_delete` фабрикует квитанцию хранилища секретов из константы `Blake3Digest32::from_bytes([0xA5; 32])`.
   - В продакшн-модулях `src/` без `#[cfg(test)]` лежат `pub struct FakeGuards` (`[0xA1; 32]`) и `pub fn build_test_export()` (`[0xC1; 32]`), а также 3 ручные реализации `SHA-256` (`0x428a_2f98`).

---

## 2. Why Development Stalled After 600+ PRs (Root-Cause Analysis for Agents)

1. **Hyper-fragmentation without spine integration:** Splitting the workspace into 48 micro-crates caused isolated agents to build self-contained in-memory state machines and mock receipt tokens inside each crate without ever wiring the end-to-end daemon spine (`ingest → prepare → redb → Qdrant → retrieve → validate → serve`).
2. **Process rules that penalized deleting broken legacy code:** Previous execution rules restricted each slice to a single primary owner and forbade deleting "legacy APIs" until a future cutover issue. As a result, when agents implemented real S11 point identity (`search_point_identity::s11`) or real `blake3` helpers, they left the broken 16-byte FNV `PointIdentityDigest([u8; 16])` and 4-lane FNV-1a functions alive in the same crates.
3. **Exemption-ledger bureaucracy instead of code repair:** Instead of replacing 8 occurrences of 4-lane FNV-1a and 3 hand-rolled SHA-256 modules with `blake3` / `sha2`, agents built `xtask/src/canonical_digest_guard` and a **3.57 MB (`72,002`-line) exemption ledger** (`xtask/canonical-digest-ledger.json`) plus **2.5+ MB of TOML coverage maps** in `swarm/coverage/**` to excuse every violation.
4. **Dumping prototypes into `bins/eliot-searchd`:** Instead of replacing the early prototype stores, `bins/eliot-searchd` accumulated 7 legacy test-harness binaries (`src/main.rs`, `src/bin/eliot-search-sealed-*.rs`), three parallel local stores (`direct_store`, `plaintext_direct_store`, `sealed_store`), an illegal in-memory BM25 inverted index (`src/lexical/`), and `#[allow(dead_code)]` composition facades.

---

## 3. Verified Code-Level Defects That Must Be Eliminated

### Defect Class A — Unwired Daemon Spine & Dead Code in `bins/eliot-searchd`

| Location | Verified Defect | Required Fix |
| :--- | :--- | :--- |
| `bins/eliot-searchd/Cargo.toml` | `default = ["wave1-shell"]` omits `wave3-index` (Qdrant), `wave4-query`, `wave5-current`, `wave6-proof`, `wave7-lifecycle`. | Make `full-baseline` the default feature set; remove artificial wave feature gates once wired. |
| `bins/eliot-searchd/src/entry.rs` | `publication_composition`, `query_composition`, `rebuild_composition`, `restore_composition`, `git_source_composition`, and `sealed_*` are marked `#[allow(dead_code)]` and have **0 callers** from `main()`. | Wire `entry.rs::main()` to the canonical provider service and real Qdrant/redb spine; remove `#![allow(dead_code)]` / `#[allow(dead_code)]` blankets. |
| `bins/eliot-searchd/Cargo.toml` (`[[test]]` targets) & `src/main.rs`, `src/bin/eliot-search-sealed-*.rs` | 7 obsolete binaries (`eliot-search-snapshotd` and 6 `eliot-search-sealed-*` binaries) are kept inside `bins/eliot-searchd` as fake `[[test]]` targets. | Delete `src/main.rs`, `src/bin/eliot-search-sealed-*.rs`, `src/sealed_*`, `src/snapshot/`, and `src/control_store/` once their test coverage is served by real crate/spine tests. |
| `bins/eliot-searchd/src/lexical/` (`mod.rs`, `build.rs`, `manifest.rs`, `search.rs`) | Custom in-memory inverted index (`postings: BTreeMap<String, Vec<Posting>>`) and custom BM25 scorer with hardcoded `Blake3Digest32::from_bytes([0x6c; 32])` and `eliot-fnv4-v1`. Directly violates **INV-01** (`Qdrant is the only indexed/search database`). | Delete `bins/eliot-searchd/src/lexical/` completely. All lexical indexing/retrieval must go through `search-lexical` + `search-qdrant-bridge`. |

### Defect Class B — Fake `Blake3Digest32` (4-Lane FNV-1a) & Hand-Rolled SHA-256

Every site below stuffs a non-cryptographic 64-bit FNV-1a (`0xcbf2_9ce4_8422_2325`) or fixed bytes into `Blake3Digest32`, or reimplements SHA-256 (`0x428a_2f98`) by hand:

1. `crates/search-query/search-access/src/lib.rs` (`derive_predicate_digest`): Uses 4-lane FNV-1a and mixes variable-length strings (`collection_alias`, `namespace_id`, `membership_ids`, `partition_ids`) without length prefixes or delimiters, allowing trivial multi-field collisions (`"abcd" + "ef" == "abcdef"`). **Must use `blake3::Hasher` with explicit length-prefixed framing or canonical CBOR.**
2. `crates/search-eval/src/fingerprint.rs`: 4-lane FNV-1a returning `Blake3Digest32`. **Replace with real `blake3`.**
3. `crates/search-index-qdrant/search-index-reclaimer/src/plan.rs`: 4-lane FNV-1a returning `Blake3Digest32`. **Replace with real `blake3`.**
4. `crates/search-lexical/src/sparse/fingerprint.rs` & `mapping.rs`: 4-lane FNV-1a returning `Blake3Digest32`. **Replace with real `blake3`.**
5. `crates/search-runtime/search-retention/src/sweep.rs` (`digest_bytes`): 4-lane FNV-1a returning `Blake3Digest32`. **Replace with real `blake3`.**
6. `crates/search-source/search-source-records/src/membership.rs` & `crates/search-source/search-source-registry/src/view.rs`: FNV-1a digest computation. **Replace with real `blake3`.**
7. `bins/eliot-searchd/src/sha256.rs`, `bins/eliot-search-doc-worker/src/sha256.rs`, `crates/search-index-qdrant/search-qdrant-supervisor/src/sha256.rs`: Hand-rolled SHA-256 (`0x428a_2f98`). **Replace with workspace `sha2` or `blake3` as required by the contract.**

### Defect Class C — Broken Legacy S11 Point Identity Still Exported in `search-point-identity`

In `crates/search-index-qdrant/search-point-identity/src/lib.rs`:
- `pub struct PointIdentityDigest([u8; 16])` (line 356) is 16 bytes instead of 32 bytes (`PointIdentityDigest(pub [u8; 32])` in Architecture S11.2 / line 2341), computed via 128-bit FNV+SplitMix64 (`frozen_digest_128`).
- `ProjectionPointKey` includes `ProjectionKind` (`Lexical`, `ExactMetadata`, `CodeStructure`, `ModelDerived`), violating **S11.1, A-29, and H18** (one unit = one Qdrant point with named vectors).
- `derive_qdrant_uuid(digest)` returns `PointId128::from_bytes(digest.0)` — making the 128-bit UUID and the "digest" identical 16-byte arrays so collision detection in `PointCollisionLedger` can never trigger for derived IDs.
- **Required Fix:** Cut over all callers (`search-projection-planner`, `search-publication`, `eliot-searchd`) to the stateless canonical CBOR + 32-byte `BLAKE3-256` S11 implementation and **delete** the 16-byte FNV types and `ProjectionKind` key from `search-point-identity`.

### Defect Class D — Security Bypass, Tautological Containment & Fake Production Receipts

1. **Tautological root containment in `bins/eliot-searchd/src/direct_store/store.rs` (`read_file_snapshot`):**
   Computes `let parent = absolute.parent()` and passes `parent` as `admitted_root_hint` to `safe_reader_adapter::read_full_file_via_kernel(&absolute, parent, max_bytes)`. Proving a file is inside its own immediate parent directory is a tautology and provides zero containment against an admitted corpus/workspace root. **Must pass the actual registered/admitted source root from the authoritative registry.**
2. **Hardcoded security disposition and path-fallback identity in `bins/eliot-searchd/src/safe_reader_adapter/kernel/`:**
   - `backend.rs` (`SafeReadBackend::inspect`) hardcodes `security_disposition: ReadSecurityDisposition::Permitted`.
   - `read.rs` (`barrier()`) hardcodes `NonZeroRevision::new(1)`.
   - `identity.rs` (`root_identity_material` on Windows) uses `path_identity_bytes(canonical_root)` instead of native volume/file identity, and `handle_identity_material` silently falls back to path bytes with `is_hardlink = false` when `native_file::observe` fails.
3. **Fabricated secret-deletion receipt in `bins/eliot-searchd/src/secret_composition/kernel/revocation.rs`:**
   `confirm_absent_delete` mints `VaultWriteEvidence::receipt_for(Blake3Digest32::from_bytes([0xA5; 32]))?`. **Must derive the receipt from actual OS vault post-delete readback state, never a fixed byte constant.**
4. **Zero profile digest fallback in `bins/eliot-searchd/src/access_composition/grant_authority/admission.rs`:**
   `compile_standalone_pre_retrieval` uses `Blake3Digest32::from_bytes([0_u8; 32])` when `overlap_proof` is `None`. **Delete the unqualified wrapper; require an explicit published profile digest.**
5. **Test fixtures in production `src/`:**
   Move `FakeGuards` (`bins/eliot-searchd/src/publication_composition/kernel/guards.rs`) and `build_test_export` (`bins/eliot-searchd/src/restore_composition/kernel/fixture.rs`) behind `#[cfg(test)]` or into `tests/`.

---

## 4. Mandatory Rules for All Agents (Effective Immediately)

All agents and managers working in `eliot-search` must obey these 6 engineering rules:

1. **NO FAKE CRYPTOGRAPHY OR SYNTHETIC DIGESTS:**
   Never write FNV-1a (`0xcbf2_9ce4_8422_2325`), SplitMix64, XOR-folding, or hand-rolled SHA-256 (`0x428a_2f98`). Never construct `Blake3Digest32` from non-BLAKE3 output or repeated byte constants (`[0; 32]`, `[0xA5; 32]`, `[0xA1; 32]`, `[0x6c; 32]`). Add `blake3.workspace = true` (or `sha2.workspace = true`) to the package's `Cargo.toml` and compute real domain-separated hashes with length-prefixed fields or canonical CBOR.
2. **SHRINK AND DELETE `xtask/canonical-digest-ledger.json` — NEVER ADD TO IT:**
   Adding an exemption to `xtask/canonical-digest-ledger.json` is forbidden. Every touch to a crate with ledger exemptions must fix the underlying fake digest/receipt and remove its ledger entries.
3. **CUT OVER AND DELETE LEGACY DUPLICATES IN ONE SLICE:**
   Do not leave broken legacy types (`PointIdentityDigest([u8; 16])`, `ProjectionKind` in point keys, `src/lexical`, `src/snapshot`, `sealed_*`, `src/main.rs`) sitting next to their replacements. Cross-package slices that update a crate and its immediate reverse consumers in order to **delete** the legacy implementation are explicitly authorized and required.
4. **NO DEAD-CODE FACADES IN `eliot-searchd`:**
   Do not add `#[allow(dead_code)]` modules to `bins/eliot-searchd`. Every composition module in `eliot-searchd` must be reachable from `src/entry.rs::main()` under default features (`full-baseline`).
5. **NO TAUGHT-TO-THE-TEST OR TAUTOLOGICAL SECURITY GUARDS:**
   Never pass `path.parent()` as `admitted_root`. Never hardcode `ReadSecurityDisposition::Permitted` or `[0xA5; 32]` receipts in production code. Keep `Fake*` / `build_test_*` helpers strictly inside `#[cfg(test)]` or `tests/`.
6. **REAL COMPILER AND END-TO-END SPINE VERIFICATION:**
   Every slice must compile cleanly under `cargo +1.98.0 check --locked --all-features` and `cargo +1.98.0 clippy --locked --all-features -- -D warnings` for all modified crates and `eliot-searchd`, and execute the relevant unit/integration tests.

---

## 5. Concrete 4-Phase Remediation Execution Plan

### Phase 1 — Eliminate All Fake Digests, Hand-Rolled Crypto & Legacy S11 Types
1. Add `blake3.workspace = true` to `search-access`, `search-eval`, `search-index-reclaimer`, `search-lexical`, `search-retention`, `search-source-records`, and `search-source-registry`; replace every 4-lane FNV-1a (`0xcbf2_9ce4_8422_2325`) with real domain-separated `blake3::Hasher` (using length-prefixed inputs or canonical CBOR).
2. Replace the 3 remaining hand-rolled `sha256.rs` modules (`eliot-searchd`, `eliot-search-doc-worker`, `search-qdrant-supervisor`) with the workspace `sha2` crate.
3. Cut over `search-projection-planner`, `search-publication`, and `eliot-searchd` to canonical 32-byte BLAKE3 S11 point identity (`PointIdentityDigest([u8; 32])`, no `ProjectionKind` in `ProjectionPointKey`), and delete the 16-byte FNV types from `search-point-identity`.
4. Remove the resolved exemptions from `xtask/canonical-digest-ledger.json`.

### Phase 2 — Purge Legacy Prototypes & Fix Security Holes in `bins/eliot-searchd`
1. Delete the illegal in-memory BM25 inverted index (`bins/eliot-searchd/src/lexical/`), legacy `src/snapshot/`, `src/main.rs`, `src/bin/eliot-search-sealed-*.rs`, and unused `src/sealed_*` modules.
2. Move `FakeGuards` (`publication_composition/kernel/guards.rs`) and `build_test_export` (`restore_composition/kernel/fixture.rs`) out of production `src/` into `#[cfg(test)]` / `tests/`.
3. Fix `secret_composition/kernel/revocation.rs` (`confirm_absent_delete`) to require real vault post-delete verification instead of `[0xA5; 32]`, and remove the `[0_u8; 32]` profile-digest fallback in `access_composition/grant_authority/admission.rs`.
4. Fix `safe_reader_adapter` and its callers so `admitted_root` is always an authoritative registered source root (never `path.parent()`), Windows root identity uses native volume/file identity (`GetFileInformationByHandle` / `FILE_ID_INFO`), and live security barriers are enforced.

### Phase 3 — Wire the Real Product Spine in `bins/eliot-searchd`
1. Set `default = ["full-baseline"]` in `bins/eliot-searchd/Cargo.toml` and remove `#![allow(dead_code)]` / `#[allow(dead_code)]` from `src/entry.rs`.
2. Connect `src/entry.rs::main()` so the standalone daemon starts `search-runtime-owner` + `search-control-redb` + `search-qdrant-supervisor`/`search-qdrant-bridge`, serves canonical local IPC via `provider_composition`, ingests admitted roots through `search-source-admission` → `search-safe-reader` → `search-materializer` → `search-unitizer`, projects and publishes generations to Qdrant via `projection_composition` + `publication_composition`, and executes queries via `access_composition` + `query_composition` + `search-candidate-validator` (with mandatory exact retained-source readback).
3. Prove the end-to-end spine with an automated integration test: `real source file → durable redb control state → Qdrant point upsert & generation alias publication → lexical/structural query → exact source readback validation → citation/handle expansion`.

### Phase 4 — Product Contract v2 (#213) & Repository Cleanup (#214)
1. Complete removal of residual swarm/coverage TOML bloat (`swarm/coverage/**`, `xtask/canonical-digest-ledger.json`).
2. Implement the versioned `v2` agent-analysis contract (#213 / ADR 0006): `CorpusId` / `PortfolioId` / `WorkspaceId` scope orientation, free-text lexical retrieval over Qdrant sparse vectors + filtered IDF, symbol/relation navigation (`search-code-enricher` / `search-subject-resolver`), and cross-repository comparison (`search-comparator`) with lineage deduplication.
