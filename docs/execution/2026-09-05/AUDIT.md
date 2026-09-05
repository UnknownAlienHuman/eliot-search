# ELIOT Search — аудит незавершённой реализации и план доведения

**Дата:** 5 сентября 2026 года. **Проверенный baseline:** `a5abdf7ef0cb9d691000759494fd8829b2ba0b60`.

## Вывод

ELIOT Search ещё не является законченным поисковым продуктом Rust + Qdrant. Здесь уже есть полезные Rust-реализации, файловые адаптеры, общий materializer/unitizer, точный поиск и отдельный настоящий redb journal. Главный остаток — не количество отсутствующих файлов, а отсутствие одной согласованной и исполненно проверенной цепочки владения, хранения, подготовки, индексирования и выдачи.

На проверенном SHA нет GitHub Actions runs. В среде аудита нет `cargo`/`rustc`; загрузка репозитория/toolchain через локальную сеть не прошла. Поэтому этот документ различает **подтверждённое чтением исходников**, **неподключённую реализацию** и **неисполненную проверку**. Он не объявляет компиляцию, Windows security, тесты или runtime успешными. Исторические зелёные package PR относятся к своим SHA и не подтверждают сегодняшнюю композицию.

Организационная причина отклонения: последовательные исправления попадали непосредственно в `main` без текущей исполняемой проверки, пока главный runtime оставался отдельным development-путём, а capability packages и orchestration registries развивались параллельно. Новый порядок — ограниченные задания, отдельные draft PR, явные зависимости, один writer на пакет и две сквозные контрольные точки: **durable DIRECT** и **живой Rust–Qdrant pipeline**.

Нормативная архитектура остаётся прежней: Rust; Qdrant — единственный поисковый индекс; redb — только техническое состояние; immutable CAS — данные и manifests; один владелец data root; клиенты не получают прямой доступ к хранилищам. Новые task packets не отменяют Part I и не являются выданными tickets/leases.

## 1. Что уже есть и не должно переписываться повторно

| Узел | Подтверждённое наличие | Чего наличие не доказывает |
|---|---|---|
| Основной daemon/CLI | `entry.rs`, постоянный DIRECT-сервис, loopback proxy, команды и process-local handles | Полный canonical provider/query contract, безопасную поставку и зелёную сборку |
| Исходные ревизии | SHA-256-привязки, immutable readback; Windows protection перед публикацией новых source events | Полный residency-key CAS и атомарный source catalog |
| Подготовка | Общие UTF-8 materializer/unitizer и KMP между units | Durable representations/profiles/manifests/receipts |
| redb | `PersistentControlJournal`: настоящие файловые транзакции, replay/readback, owner handoff | Использование этой БД основным daemon |
| Source roots | Регистрация восстанавливается под owner lock; missing root не трактуется как пустой | Управление через работающий сервис и доказанную currentness |
| Qdrant packages | Модели схем, фильтров, lifecycle и publication | Сетевой transport, настоящий процесс и выполненную квалификацию |
| Foundation packages | Contracts/domain/ports/config и отдельные исторические реализации | Принятые handoffs именно текущего дерева или автоматическое разрешение следующей wave |

В текущем baseline уже внесены защитные исправления: отказ plaintext в protected readback; ciphertext до source publication; no-clobber публикация защищённого объекта; отказ усыновления plaintext orphan; монотонность control snapshots; preflight при потере namespace/log; блокировка повреждённого proxy exchange. Эти исправления **не записываются заново как отсутствующие**. Но их собственные регрессионные тесты и взаимодействие с остальным runtime остаются неисполненными. Новые задания обязаны сохранить их и проверить, а не заменить очередной параллельной реализацией.

Источники: [основной manifest](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/bins/eliot-searchd/Cargo.toml), [DIRECT preparation](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/docs/runtime/DIRECT_PREPARATION.md), [redb](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/docs/runtime/CONTROL_REDB.md), [последние guards](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/docs/runtime/CATALOG_LOSS_AND_CHANNEL_FAILURE.md).

## 2. Исполнение, сборка и ownership

### A01 — Блокирующий: нет текущей исполняемой квалификации → T03, T04

`cargo check`, тесты, fmt, Clippy и rustdoc не выполнены для текущего baseline. Добавленные тестовые файлы нельзя считать пройденными. Особенно недостаточно Linux-проверки для Windows-only ветвей. Требуется точный head, lockfile, toolchain, список реально запущенных тестов и их exit codes; ноль тестов, все ignored или пропущенная Windows-проверка — не успех.

Официальная страница `MetadataExt`, просмотренная в версии документации **Rust 1.98.1 (2026-09-01)**, помечает `volume_serial_number` и `file_index` как nightly-only `windows_by_handle`. Репозиторий закрепляет 1.98.0; отдельное исполнение именно этого компилятора здесь отсутствует. Вызовы найдены в девяти daemon-файлах: `development.rs`, `direct_store.rs`, `sealed_store.rs`, `sealed_file_reader.rs`, `sealed_transaction.rs`, `sealed_owner_epoch.rs`, `sealed_root_identity.rs`, `service_state.rs`, `src/bin/eliot-search-sealed-direct.rs`. Исправить только два основных файла недостаточно при `--all-targets`.

Нужен стабильный наблюдатель по открытому Windows handle. Нельзя «исправить» сборку удалением проверки идентичности, подстановкой нулей, `RUSTC_BOOTSTRAP`, nightly или исключением требуемых targets из доказательств.

Источники: [Rust MetadataExt](https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html), [development.rs](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/bins/eliot-searchd/src/development.rs), [manual check](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/.github/workflows/manual-workspace-check.yml), [exact-head runs API](https://api.github.com/repos/UnknownAlienHuman/eliot-search/actions/runs?head_sha=a5abdf7ef0cb9d691000759494fd8829b2ba0b60&per_page=2).

### A02 — Блокирующий для swarm: registries противоречат фактическому состоянию → T01

`swarm/launch-state.toml` всё ещё объявляет P00/W0, разрешает только `search-contracts`, оставляет domain/ports conditional и остальные пакеты blocked. В draft_control указаны нулевые issued tickets, leases, submissions, reviews и accepted handoffs. Это не означает, что весь код отсутствует; это означает, что из этих файлов нельзя получить достоверную очередь допуска текущей реализации.

Требуется сверить Cargo members/targets/dependencies, текущие public APIs, точные contexts и фактически существующие evidence. Нельзя просто выставить все gates в PASS. Bootstrap T01–T04 должен позволить исправлять известный build blocker, не требуя его уже успешной проверки как условия начала самого исправления. Полная M0-квалификация наступает только после T04.

[Источник: launch-state](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/swarm/launch-state.toml).

### A03 — Высокий: старые issues создают конкурирующие задания → T01

Есть повторные задания на contracts/domain/ports/config/redb, некоторые с плоскими путями `crates/search-source-admission/**`, тогда как фактический пакет находится в `crates/search-source/search-source-admission/**`. Старые #70/#85 описывают redb повторно; #83 предписывает иной cipher, #84 — обязательную нормализацию, которая несовместима с текущей точной raw-coordinate ветвью без отдельного профиля и maps. Исторические PR #62/#66/#67/#94/#95/#96 подтверждают, что часть «реализовать scaffold» уже не соответствует дереву.

Нужна crosswalk-таблица: выполнено и подтверждено, superseded duplicate, остаточная интеграция, конфликт контракта. Закрытие дубликата не должно означать закрытие недоделанного runtime. В этом аудите старые issues автоматически не закрывались.

### A04 — Высокий: composition root превратился в набор реализаций и экспериментов → T02

Daemon `AGENTS.md` задаёт чистую композиционную роль и предел размера. В каталоге сосуществуют основная DIRECT-реализация, snapshot/BM25, несколько сервисов, sealed experiments, собственные handles/continuations и storage logic. Точный текущий handwritten LOC не посчитан; нарушение численного лимита без подсчёта не утверждается. Но конфликт роли и альтернативные entrypoint подтверждены исходниками.

Нельзя лечить это forwarding-only crates или массовым переносом без ownership-карты. T01 устанавливает reachable graph и точный move map; T02 изолирует unsupported targets и переносит логику к существующим capability owners, сохраняя тесты и поведение. Крупный перенос внутри PR разбивается на проверяемые механические commits до любых semantic edits.

### A05 — Блокирующий: несколько протоколов владения одним root → T02, T08

Основной daemon использует `.eliot-search-owner.lock`, snapshot daemon — `runtime/owner.lock`. Разные файлы не обеспечивают взаимного исключения. Кроме того, primary owner хранит PID/время и не подключает полный monotone incarnation/owner-epoch протокол, существующий в отдельных kernels. Требуются единая реально удерживаемая блокировка, точная идентичность процесса и физического root, reverse-order shutdown и отказ при неопределённом takeover; срок истечения записи сам по себе не разрешает захват.

[Источники: primary owner](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/bins/eliot-searchd/src/development.rs), [snapshot owner](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/bins/eliot-searchd/src/main.rs).

## 3. Сохранность, источники и durable DIRECT

### A06 — Блокирующий: append-only каталог не транзакционен → T05, T10, T11

`append_drafts` сначала дописывает несколько событий, делает sync, затем перечитывает весь журнал. Сбой после возможной записи может оставить disk/memory разными. Постоянный сервис обычно выдаёт ошибку команды и продолжает принимать команды. Защита proxy exchange относится к потоку ответа, а не к атомарности storage.

T05 обязателен как временный fail-closed барьер: неопределённый write/readback блокирует весь затронутый runtime и инвалидирует результаты до точного recovery, без заявления rollback. T10 сначала проверяет старые namespace/events/roots/manifests и объекты без переключения. T11 делает единственный атомарный cutover на redb. Не допускаются dual-write authority, пустая БД вместо миграции или удаление старых данных до проверенного завершения.

[Источник: append_drafts](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/bins/eliot-searchd/src/direct_store.rs).

### A07 — Высокий: catalog-loss guard добавлен, но нужна общая recovery-модель → T04, T05, T10, T11, T37

Последний preflight правильно не создаёт пустой каталог при наличии остаточных данных. Он проверяет наличие/тип, не подлинность всей истории. Внутренние plaintext development constructors не изменены. Требуется испытать потерю каждого компонента, torn tails, устаревший in-memory state и любые supported maintenance entrypoint. GC должен потреблять достоверный current reference set и pins, а не только наличие двух файлов. Полностью стёртое состояние без следов невозможно распознать как старую установку одним presence check — этот предел не надо скрывать.

### A08 — Блокирующий: pre-open path checks не равны final-handle containment → T03, T07

Проверка symlink/reparse и canonical path до открытия оставляет интервал подмены. Metadata на одном handle до/после чтения доказывает лишь часть стабильности этого объекта, но не весь путь его допуска. Нужны final-handle identity, разрешённый root/ancestry, Windows sharing/ACL и no-execute policy. Проверяются rename, replacement, hardlink, reparse и изменение содержимого; результат нестабильного чтения не сохраняется как принятая ревизия.

`search-safe-reader` уже содержит полезную проверочную семантику, но сам не выполняет filesystem I/O. Основной reader обязан использовать корректный adapter, а не выдавать prefilled observations за измеренные факты.

[Источник: reader kernel](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/crates/search-source/search-safe-reader/src/lib.rs).

### A09 — Высокий: redb готов как отдельный adapter, не как primary control plane → T09–T12

Нужны семантические codecs control records, finite deadline/cancellation composition, реальные identities и правильная обработка unknown commit. Record class без проверки payload не доказывает content-free storage. Сейчас транзакция загружает весь snapshot и создаёт новый `BTreeMap`, после commit сравнивает весь набор: это конкретная структура затрат, но величина задержки/RSS не измерена. Исправление должно сохранить атомарный readback, а не просто убрать проверку ради скорости.

[Источник: PersistentControlJournal](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/crates/search-control-redb/src/persistent.rs).

### A10 — Высокий: config/feature presence не управляет реальной готовностью → T12

Primary использует локальные constants и собственный Health. Наличие Cargo feature либо зависимости не подтверждает подключённую capability. Требуется эффективный config snapshot после всех live/barrier/restart/rebuild obligations и readiness, выведенная из реального состояния adapters. Неподключённый indexed backend не должен объявляться доступным через шаблонную структуру.

### A11 — Высокий: canonical admission, identity и registry обходятся development-каталогом → T13

Стабильные источники, path history, memberships, policy barriers и ревизии уже имеют отдельные модели. Их надо соединить с фактическими observations и transactional source state. Нельзя делать source identity из одного пути, считать общий event counter ревизией каждого источника или повторно создать источник при возврате A→B→A. Старые immutable handles/байты должны сохранить точную интерпретацию после explicit migration.

### A12 — Высокий: полный residency-key CAS отсутствует в основной композиции → T14

Нормативный объект связывается со scope/access/confidentiality/encryption-key/retention/erasure domains и versioned digest. Текущая source/revision/SHA-256 раскладка не подменяет эту структуру. Требуются typed domain binding, authenticated envelope, no-clobber durable write, exact readback, одинаковое содержание в разных security domains без незаконного физического объединения и явная миграция старого формата. Не вводить новый cipher по старому issue без принятого решения. SHA-256 и BLAKE3 остаются разными алгоритмами.

### A13 — Высокий: подготовка каждый раз пересчитывается; durable spine отсутствует → T15–T17

Raw UTF-8 materializer сохраняет исходные bytes и различает CR/LF. Unitizer проверяет lines и границы Unicode. Literal KMP умеет cross-unit/overlapping matches. Это нужно сохранить. Следующий слой — фиксированные representation profiles, maps и единые durable unit manifests, связанные с exact admitted revision. Нормализованный/transcoded профиль не должен тихо изменить raw-coordinate semantics. Поиск не должен делать скрытые control/CAS writes ради ленивой подготовки; подготовка публикуется ingestion-путём.

**Контрольная точка T17:** через основной бинарник создать источник, сохранить/подготовить, найти cross-unit match, изменить/удалить live файл, прочитать точную старую ревизию согласно handle contract, перезапустить процесс и повторить. Это ещё не Qdrant acceptance.

## 4. Provider, доступ и результаты

### A14 — Высокий: framing bounded по памяти, но не всегда по времени → T06

Primary line reader может продолжать вычитывать бесконечный oversized frame до newline/EOF. Child startup/stdout/finish не ограничены общим operation deadline; socket timeout не ограничивает зависшего ребёнка. Добавленный poison-channel guard закрывает перенос остатка ответа, но не заменяет time budget/cancellation и реальные socket/process tests. Нельзя повторять мутацию после timeout автоматически.

### A15 — Высокий: текущий token-file протокол не равен принятому provider binding → T18, T19

Loopback и client challenge полезны, но сами по себе не создают purpose-bound OS-secret lease, mutual provider identity, canonical session/request/version sequences и capability protocol. Нужны один согласованный server/client path, точное framing, replay rejection, bounded diagnostics и отказ при missing/revoked binding. Не добавлять второй молчаливый transport fallback. Local process ≠ авторизованный клиент.

### A16 — Блокирующий для многокорпусной выдачи: нет live grant pipeline → T20

Проверка источника и authentication соединения не заменяют `grant → AccessCompiler → допустимые candidate/IDF/count/facet/trace legs → emission barrier`. Restrictive update немедленно инвалидирует затронутые rank legs и результаты. Неавторизованные документы не должны влиять на статистику и ранжирование разрешённых документов. Инвариант проверяется до получения top-k, а не удалением запрещённых строк в конце.

### A17 — Высокий: handles и continuation ещё development-local → T21

Текущий `ResultHandleCatalog` хранит TTL и source fence, но не полный client/binding/grant context; session nonce производится из namespace/PID/time. Документация честно называет такие токены locators, не production bearer credentials; поэтому аудит не заявляет доказанный обход доступа через «угадываемый токен». Требуется canonical token generation/binding, live reauthorization каждого expansion, deny/purge dominance и точная политика session/durable handles. Частичное покрытие и expiration не превращаются в полный результат.

[Источник: result_handles](https://github.com/UnknownAlienHuman/eliot-search/blob/a5abdf7ef0cb9d691000759494fd8829b2ba0b60/bins/eliot-searchd/src/result_handles.rs).

## 5. Живой Qdrant вместо модели

### A18 — Блокирующий: bridge и supervisor не являются живыми adapters → T22–T24

`search-qdrant-bridge` прямо описывает in-memory semantics, supervisor — эффекты, которые выполняет внешний adapter. Нужны exact artifact/version/platform/client set, квалификационный probe и настоящий owned process/transport. Применяется pinned dependency, не автоматический upgrade/download. Неверный процесс на loopback-порту не считается правильным Qdrant. Сбой indexed backend возвращает его недоступность; DIRECT может работать отдельно, но не притворяется indexed success.

### A19 — Блокирующий: candidate filter не гарантирует независимый IDF → T22, T24, T25

Официальные Qdrant docs указывают default IDF по всему queried shard. В per-tenant инструкции описан отдельный `idf` search parameter с payload filter, доступный начиная с **1.19.0**. Это не повод автоматически обновить сервер: необходимо выполнить probe точного выбранного artifact и проверить идентичность eligibility для retrieval и scoring corpus. Запрещённые документы не должны менять score/order/count/trace разрешённого запроса. Если нужное поведение не прошло квалификацию, leg недоступен; локальный самописный BM25 индекс не является разрешённым обходом.

Источники: [Qdrant text search](https://qdrant.tech/documentation/search/text-search/full-text-search/), [per-tenant IDF](https://qdrant.tech/documentation/tutorials/multiple-partitions/). Проверены 5 сентября 2026; это требования к probe, не PASS текущему репозиторию.

### A20 — Высокий: lexical/projection/publication не соединены durable-путём → T25–T29

Lexical encoder должен производить named sparse vectors и корректный ScoringDocumentId, а не владеть postings. Projection manifest связывает один point ровно с одним membership. Upsert и exact count/readback предшествуют control commit. VisibleEpoch публикуется только после повторной live-проверки restrictive guards; отменённый epoch не переиспользуется. Route cutover, old-reader pins и reclaim должны работать при настоящих process failures, а не только в reference model.

### A21 — Блокирующий release milestone: нет сквозного executed proof → T30

Тест через canonical client и primary daemon должен пройти `source → revision → preparation → Qdrant projection → validated result → exact expansion → restart`. Затем — corruption, Qdrant stop/restart, interrupted publication, deny, rebuild и source-loss scenarios. Нужны точные серверные бинарники и code heads; mock-only tests оставляются unit tests. T30 не разрешает скрыть нерешённые currentness/retention требования полным product-ready флагом.

## 6. Currentness, Git, overlays и расширенные baseline recipes

### A22 — Высокий: roots нельзя полноценно управлять через живой daemon → T31

Регистрация/синхронизация отдельной командой reacquire-ит root lock, который уже удерживает сервис. Требуются root RPC под существующим owner, сохранение регистрации и gap-aware reconciliation/cursors. Watcher — hint, не источник истины. Недоступный root не пустой; завершение обхода нескольких roots не доказывает current workspace при unresolved gap.

### A23 — Высокий: Git identity kernels не выполняют exact Git acquisition → T32

Нужно читать exact admitted object/worktree state без hooks, filters, shell, credential prompts и сети. Packed/loose capabilities должны быть честно объявлены; unsupported object storage нельзя заменить current-path bytes. Repo lineage/fork/mirror/submodule определяется evidence, не URL или одинаковым HEAD. Декомпрессия и object size ограничены.

### A24 — Высокий: ephemeral IDE overlays не подключены → T33

Unsaved text поступает только от authenticated IDE snapshot, не inferred watcher. Он не попадает в redb/CAS/Qdrant/backup/telemetry/provider cache без явного save/admission. Нужны overlay replacement/removal, TTL/bounds и немедленная потеря видимости после session/access invalidation. Индекс либо stale base и overlay не должны удваивать membership/результаты.

### A25 — Высокий: literal success не равен exact negative proof → T34

Полный scan переданных chunks не доказывает полный authoritative corpus. Нужен frozen denominator, источник каждой ревизии и причина каждого пропуска. Top-k никогда не уменьшает denominator. Changed/missing/unreadable/cancelled остаются partial/unknown, а не «не найдено». Проверять это через публичную операцию, не только чистую функцию.

### A26 — Высокий: structural/resolution/comparison recipes не реализованы в основном запросе → T35, T36

Code enrichment требует pinned parser/grammar, exact structural anchors и явно ограниченной semantic assurance. Compare/resolution должны использовать такие anchors и source validation, сохранять ambiguity и различать descriptive comparison от нормативного заключения. Нельзя объявить готовность baseline recipes по наличию enums или empty success adapters.

## 7. Lifecycle, эксплуатация и чистая поставка

### A27 — Высокий: retire + orphan GC не заменяют retention/purge → T37, T38

Обычная retention учитывает current/historical references, handles, pins и незавершённые operations. Security purge сначала ставит немедленный deny barrier, затем очищает все затронутые representations/points/handles и фиксирует exact outcome. Сбой midway не снимает deny. Logical deletion не обещает physical media erasure. Новые CAS/manifests должны быть включены в root-of-reachability, иначе расширение pipeline создаст новый destructive GC bug.

### A28 — Высокий: restore, rotation и ownership cutover не завершены → T39

Копия root или backup не получает authority по прежнему пути. Restore начинается с pending revalidation; identity/schema/key/profile incompatibility не исправляется угадыванием. Требуется сохранить handles согласно контракту, проверить потерю/смену ключа и невозможность двух authoritative owners namespace. Ошибка cutover не должна оставлять обе стороны активными.

### A29 — Высокий: нет измеренной resource/leakage-квалификации → T40

Поиск пока выполняет полные проходы и preparation; expensive snapshot/identity hashing повторяется. Нужны реальные bounded corpus fixtures и p50/p95/RSS/disk growth. Лимит bytes одного файла не равен лимиту всей работы запроса. Проверить secrets, source text и path sentinels во всех errors/Debug/telemetry. Например, `RelativePathToken::Debug` выводит значение целиком: это кандидат на disclosure review, не доказательство утечки во внешнем endpoint без анализа вызова.

### A30 — Средний для runtime, высокий для pure-Rust workflow: Python остался в tooling → T41

Python используется преимущественно валидаторами документации/registry/планирования; PowerShell wrappers вызывают `.py`. Это не Python-поисковое ядро. Нужно перенести только обязательные проверки в Rust/Cargo и доказать parity на negative fixtures, затем убрать заменённые entrypoint и Python prerequisite. Не тратить время на ещё одну систему метаотчётов вместо backend. CI остаётся `workflow_dispatch` only и не пишет product code.

### A31 — Высокий для честности capability: worker binaries пустые → T42

`eliot-search-model-worker` и `eliot-search-doc-worker` содержат `fn main() {}` и могут завершаться успешно, ничего не сделав. Для baseline их надо исключить/отключить и возвращать unavailable, а не обязательно реализовывать все модели/OCR сейчас. Optional leaf adapters не импортируют authority или storage клиентов внутрь Search. Отдельная future optional qualification требует measured benefit и accepted gate.

### A32 — Высокий: отсутствует одна воспроизводимая поставка → T43

`QUICKSTART.md` описывает старый snapshot command surface под именами primary binaries. Нужен один установленный CLI → daemon → Qdrant path на Windows с validated config/OS secrets, отдельным data root, обновлением, диагностикой и безопасным удалением. Release archive не должен включать внутренние agent scaffolds, сторонний lexical runtime и пустые workers. Документированный quickstart выполняется на чистой машине/профиле и после рестарта, а не проверяется глазами.

## 8. Порядок завершения

**M0 — T01–T04:** reconciled ownership, isolated experiments, stable Windows API, выполненная build/test baseline.

**M1 — T05–T08:** fail-closed errors, bounded protocol, admitted final handles и один durable owner.

**M2 — T09–T12:** подготовленная миграция, единый redb control owner и effective config/readiness.

**M3 — T13–T17:** canonical admission/CAS/preparation и проверенный durable DIRECT через основной бинарник.

**M4 — T18–T21:** canonical authenticated provider, live grants и handles/continuations.

**M5 — T22–T30:** квалифицированный Qdrant, transport/lexical/projections/publication/query/rebuild и сквозной product-spine test.

**M6 — T31–T36:** live currentness/Git/overlays, exact proof и structural/comparison recipes.

**M7 — T37–T43:** retention/purge/restore, эксплуатационные измерения, Rust tooling, честный optional boundary и Windows release.

DAG, а не один порядковый номер, определяет готовность. T22 может квалифицировать artifact после M0 параллельно с storage; T41 может переносить tooling после M0, если не конфликтуют package locks. Задачи с общим daemon/package write scope исполняются последовательно. Запуск всех 43 writers одновременно запрещён.

## 9. Что значит закрытая задача

Task PR изначально содержит только задание. Implementation считается завершённым только после кода, failing-then-passing discriminating fixtures, exact-head команд с exit codes, независимого review и подтверждения post-merge состояния. Required live/native test, который не запускался, блокирует соответствующий gate. Свежая документация, feature flag, checksum исходного файла или model test не заменяет эту проверку.

Новый план не выдаёт product acceptance и не гарантирует отсутствия неизвестных багов. Он покрывает обнаруженные дефекты и проверенные интеграционные разрывы, задаёт место для новых findings и запрещает закрывать задачу только потому, что агент создал нужный файл. Остаточная неизвестность локализуется тестами в конкретном PR, а не скрывается очередным общим «почти готово».
