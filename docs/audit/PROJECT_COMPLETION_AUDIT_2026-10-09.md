# ELIOT Search — аудит всего пути до завершения проекта

**Проверенный исходный снимок:** `1a6d17d073c5f6d1fe50f21082439f1965d24ef8`. **Координатор:** [#97](https://github.com/UnknownAlienHuman/eliot-search/pull/97). **Полная карта:** [PROJECT_COMPLETION.md](../product/PROJECT_COMPLETION.md). Эта редакция охватывает не только Wave 2, но и оставшуюся интеграцию, все группы профилей, lifecycle, квалификацию и выпуск.

## Итог проверки

Подготовка проекта не должна состоять из очередного списка пожеланий. В общей карте определены 115 именованных этапов и 283 связи между ними. Суффиксы `.core`, `.schema`, `.live`, `.build`, `.candidate`, `.run`, `.review`, `.publish` — фазы существующих задач, а не новые issues и не runtime-сущности. Для каждого этапа записаны входы, владелец исходников и проверяемый результат.

Статическая проверка модели зависимостей выполнена Python `graphlib.TopologicalSorter`: 115 уникальных узлов, 283 ребра, 0 отсутствующих узлов, 0 self-dependencies, 0 циклов. Отдельно проверено отсутствие прямой зависимости source/runtime-этапов от итогового release verdict. Это проверка согласованности плана, не компиляция и не доказательство отсутствия ошибок в продукте.

## Исправленные причины остановок

### 1. Главный coordinator продолжал предлагать устаревшую архитектуру

Body #97 ещё содержал создание `search-canonical`, production Ciborium, старые source-donor PR как merge bases и несколько параллельных менеджеров. Он заменён целиком, а не дополнен очередным конфликтующим комментарием. Единственный canonical owner — существующий `search-contracts`; #200/#207/#209/#210 — только исторические доноры конкретных файлов; менеджер один.

### 2. Завершённые foundation-задачи могли запускаться повторно

#319/#237, #322/#253, #323/#258 и #326/#250 отмечены как доставленные результаты с ограниченной областью доказательств. Unicode research не выдан за production tokenizer. #324 остаётся повторным запуском настоящего validator, #327 — отдельным dependency/advisory действием. Следующий source owner остаётся #256; активный worktree не сбрасывается из-за документационного merge.

### 3. Не было опубликованной общей точки входа для новых интеграционных владельцев

#330 revision store, #331 preparation persistence, #332 native client edge и #333 config runtime уже имеют конкретные исходники, инструкции, ссылки и критерии завершения. Теперь они включены в публикуемую полную карту, а старые programme PR направляют туда, а не требуют заново реализовать пакеты. Ссылка на отсутствующий `docs/product/PROJECT_COMPLETION.md` закрыта этим документационным cutover.

### 4. Работоспособность смешивалась с релизной квалификацией

Configured, Operational и ReleaseQualified разделены. Продукт-кандидат обязан запускаться, чтобы его можно было испытать. Startup не зависит от будущего benchmark/attestation и не импортирует evaluator. При этом наличие dependency, feature или credential не означает operational readiness: нужны реальные constructed owners и readback. #333/#277 и programme #109 описывают эту границу.

### 5. Packaging и qualification блокировали друг друга

Сначала реализуются форматы, verifier, runner и package tooling; затем строится неподтверждённый candidate; до результатов фиксируется план; далее запускаются установленные проверки; затем независимая верификация и явная публикация тех же bytes. В #137/#140 теперь нет требования уже иметь финальный acceptance до создания кандидата. Предварительные модели/проверки не заменяют реальные installed runs.

### 6. Extraction клиента не имел выполнимой границы завершения

#235.core извлекает один typed engine, сохраняя неизменной существующую внешнюю connection assembly до #332. #332 даёт реальный native endpoint и удаляет старую assembly. Запрещено продвигать TCP/token/line-protocol в новый canonical client, но также запрещено удалять единственную используемую сборку до появления замены. #116 и #235 согласованы с этой последовательностью.

### 7. Удаление legacy API могло ломать не проверяемых потребителей

#256 добавляет новый stateless S11; #259/#262 переводят потребителей; #329 удаляет старую registry/FNV/callers/exports до #264. Проверка включает непосредственных reverse consumers. Старые экспорты не принимаются новой generation и не становятся fallback. Временная совместимость имеет точного владельца удаления.

### 8. Preparation всё ещё скрывает ошибку профиля

Повторно прочитан `bins/eliot-searchd/src/preparation_store/kernel/batch.rs` на указанном снимке. `preparation_checkpoint()` использует `canonical_materializer_digest().unwrap_or([0; 32])` и такой же fallback для unitizer. Ошибка превращается в правдоподобный checkpoint. Это закреплено за #331: fallible profile acquisition, verified bundle, actual persistence/readback и удаление zero/invalid success paths. Сам исходник в этом документационном проходе не исправлен.

### 9. Legacy SHA и canonical-domain SHA — разные формулы

`SHA256(old_preimage)` нельзя заменить на `SHA256(domain || NUL || old_preimage)` с обещанием сохранить digest. #238 допускает только ограниченный exact-byte helper внутри уже существующего shared owner и явную v1/v2 migration. Новые crypto/codec владельцы не создаются.

### 10. Optional означало «когда-нибудь придумать задачу»

#216 содержит конкретные поздние фазы HTML, LaTeX, embedded-text PDF, scholarly PDF, Office, legacy formats и OCR с одним worker/preparation boundary. В карте присутствуют также code-fact producers, scholarly connectors, LSP/MCP и их отдельное acceptance. Not-shipped означает незавершённую/исключённую возможность, а не выполненную задачу. Базовый релиз и завершение всего выбранного backlog — разные утверждения.

## Все вертикали теперь имеют вход и выход

| Вертикаль | Рабочие владельцы | Результат, без которого она не завершена |
|---|---|---|
| Root/control/source | #266–#272, #239, #241, #129 | Один root/control owner, admission до bytes, coherent immutable SourceView и historical scopes |
| Secrets/storage/preparation | #307–#310, #315/#316, #330/#331, #257 | Реальные protected immutable artifacts, exact key references, verified maps/UnitSet и redb readback |
| Indexed publication | #256–#264, #329, #119/#120 | Новая согласованная generation, durable recovery, одинаковая retrieval/IDF population, actual source readback |
| Access/handles/continuations | #287/#288/#274, #282/#275, #283–#285/#276, #300 | Ограниченные opaque capabilities, live reauthorization, recoverable pins и range-bound expansion |
| Query/navigation | #278–#281, #290/#291/#293/#294, #296–#298/#232 | Одна real query chain, exact ranking, сохранённые 11 recipes плюс evidence/orientation |
| Native/config | #194, #235/#332, #238/#333/#277 | Durable bindings, one native pipe/client/parser, applied config, noncircular readiness |
| Code/docs/research | #223–#231/#236/#254, #216 phases | Профили и доноры связаны с настоящими source/UnitSet/coordinates, а не отдельными каталогами |
| Lifecycle | #300–#305, #243–#245, #134.release | Полные roots/reference graphs, restrictive purge, backup/restore/migration без resurrection/last-copy loss |
| Tooling/release | #233/#234/#240/#242/#215/#137/#140, #214/#220 | Exact installed candidate, raw evidence, independent review, operator guide и human publish |
| Optional agent leaves | #130/#219/#139 | Тонкие adapters над тем же client; отдельные selected-profile claims и A1 |

## Что брать у доноров

Полная таблица первичных источников находится в карте. Узкие crates обслуживают грамматику, matching, Unicode и AST; SCIP/SARIF/LSP/MCP — межсистемные форматы; Qdrant — единственный physical index; Tika/GROBID/Docling/OCR — isolated profile providers; Lucene/Nix/Vespa/Aider/Zotero/Onyx — конкретные state-machine/workflow принципы, не дополнительные серверы внутри Search.

Не добавляются новые RAG frameworks, второй CAS/control/index/graph, generic receipt framework, runtime downloader или ещё один controller. Exact donor versions/features/checksums/advisories проверяются в момент конкретного dependency cutover и фиксируются; выбранный механизм не равен разрешению копировать moving HEAD. Нельзя честно утверждать, что у всех доноров взято абсолютно всё лучшее или все будущие версии проверены.

## Правила для менеджера

Один текущий issue и первый dependency-ready этап. Субагенты исследуют узкие вопросы по исходникам/документации и возвращают пути, факты, proposed patch и blockers. Менеджер пишет, интегрирует и отвечает за scope/dependencies/lockfile. Не запускать новый большой аудит перед каждым изменением. Не создавать ещё один issue, когда отсутствующая функция уже принадлежит существующему owner.

При изменении API сначала перечислить actual reverse consumers. Код → scoped locked check → strict Clippy → необходимый focused proof → независимый review → merge → обновление только затронутых строк. Не удалять старый путь, пока новый не может заменить реального потребителя; не сохранять его навсегда после cutover.

Broad тесты отложены до завершения соответствующего implementation closure, но до релиза выполняются обязательно. Ни compilation, ни shape-validator, ни security-review badge не заменяют native/installed evidence.

## Граница данного результата

Подготовлены карта всего проекта, согласованные programme bodies, entrypoints и критерии release. Проверены модель DAG и конкретные актуальные source/PR/issue границы. Исходный Rust, manifests, Cargo.lock, workflows и тестовые ожидания не меняются. Никакого нового Cargo/Clippy/Windows/Qdrant PASS этот аудит не выдаёт.

Предыдущие F01–F154 остаются техническими obligations в master/specialized audits. Это не новый полный построчный прогон каждого файла и не доказательство отсутствия багов. Полная подготовка backlog означает, что путь и требования прописаны до выпуска, а не что код уже реализован или допущен к эксплуатации.