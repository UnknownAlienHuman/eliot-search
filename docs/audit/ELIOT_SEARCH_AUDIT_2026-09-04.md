# ELIOT Search: аудит Rust/Qdrant

Дата: 4 сентября 2026 года, America/New_York. Исходный `main`: `d80ed9def94e75793c3078e40bd90cfd6a7818db`.

## Вердикт

Проект не пустой, но ещё не является собранным продуктом на Rust/Qdrant. Есть Rust-модули, модели переходов состояния, подготовка текста и несколько development-runtime. Основной разрыв — между этими модулями и реально запускаемым демоном. Наличие Cargo-пакета, feature или успешного теста модели не доказывает работающий backend.

Целевая архитектура остаётся прежней: Rust; один владелец data root; неизменяемые ревизии и производные артефакты в файловом CAS; redb только для технического control state; Qdrant как единственный индекс. DIRECT должен работать без Qdrant, но через те же проверенные ревизии и подготовленные данные.

## Что действительно подключено

| Узел | Состояние исходного кода | Последствие |
|---|---|---|
| Основной демон | `bins/eliot-searchd/Cargo.toml` направляет `eliot-searchd` в `src/entry.rs`, а не в `src/main.rs`. | Аудит только `main.rs` проверяет другой runtime. |
| DIRECT | `secure_direct_store.rs` проверяет сохранённые ревизии, но ищет через `development::scan_text`. Materializer/unitizer не включены в default feature. | Канонический preparation pipeline не подключён. |
| redb | `search-control-redb` прямо описывает in-memory reference model. В его Cargo.toml нет зависимости `redb`. | Название пакета не означает наличие дисковой БД и crash recovery. |
| Qdrant | `search-qdrant-bridge` прямо описывает in-memory model для будущего concrete adapter. | Это не рабочий сетевой data plane Qdrant. |
| Persistent roots | `source_roots.rs` содержит файловый каталог, но не объявлен в двух daemon entrypoint: `entry.rs` и `main.rs`. | Файл landed; интеграция в запускаемый демон не подтверждена. |
| Альтернативный индекс | `src/main.rs` собирает отдельный `eliot-search-snapshotd` с собственным `LexicalIndex`. | Такой эксперимент нельзя выдавать за согласованный Qdrant indexed baseline. |
| Preparation | `search-materializer` и `search-unitizer` содержат настоящий Rust-код ограниченной обработки UTF-8 и разбиения текста. | Их следует подключать и проверять, а не переписывать с нуля. |

### Дефекты вне уже внесённых исправлений

В `source_roots.rs::add` переменная `canonical` перемещается в `SourceRootEntry`, а затем используется повторно. При подключении модуля это станет ошибкой владения. `refresh` сравнивает только количество доступных roots: смена доступности двух разных каталогов с сохранением общего количества возвращает `false`. Нужны тесты самого каталога и его включение в реальный target; исправление отдельного некомпилируемого файла не закрывает landing.

В `app.rs::serve_stdio` ограничение длины команды проверяется после `BufRead::lines()`. Само чтение до перевода строки не ограничено. Требуется bounded reader до выделения неограниченной памяти, с явной обработкой превышения лимита, EOF и CRLF.

Windows-фасад DIRECT сначала вызывает plaintext indexing, затем sealing. Этого недостаточно для production-гарантии encrypted-at-rest: необходимо шифровать ревизию до её публикации и проверять восстановление после каждого durable шага. Полная проверка файловых гонок и Windows security ещё не выполнена.

Текущий DIRECT использует SHA-256 metadata, а shared preparation API принимает `Blake3Digest32`. Нельзя просто переименовать 32 байта SHA-256 в BLAKE3, выдать глобальный номер события за номер ревизии источника или создать фиктивный receipt для прохождения типов. Нужны проверяемые привязки source/revision/digest/receipt и их восстановление после перезапуска.

## Python

В исследованном дереве `tools/` Python используется прежде всего для валидаторов документации, coverage-графов, планирования заданий и qualification. Это не свидетельство Python-поискового ядра. При этом обвязка действительно разрослась: например, `coverage_graph_v2.py`, `validate-integration-bootstrap.py`, `package_maps_v2.py` и пакеты планировщиков.

Два удалённых workflow также генерировали Rust-код через Python и пытались сами коммитить его в `main`. Такой способ реализации продукта прекращён.

Перенос оставшихся обязательных проверок в Rust/Cargo нужен отдельно: сохранить негативные fixtures и реальные причины отказа, затем удалить заменённые Python entrypoint. Просто удалить все `.py` и объявить готовность нельзя. Приоритет — работающий Rust runtime, а не новая система отчётов о документации.

## Исправления и граница проверки

В `main` внесены исправления:

- Workspace workflow возвращён к `workflow_dispatch`, read-only permissions и checkout точного SHA; удалены два автоматических workflow-генератора кода. CI больше не используется для коммитов этих патчей.
- В owner-epoch encoder исправлено использование захвата переменной внутри `format!(concat!(...))`.
- Исправлен parser имени эпохи: writer использует 20-значное число с ведущими нулями, а прежний reader их запрещал. Это ломало повторное открытие записанной истории.
- Добавлен отказ до записи за пределом максимальной читаемой истории эпох.
- Ручная unsafe-очистка памяти в общем коде `sealed_store` заменена на safe API `zeroize` версии 1.9.0, уже присутствовавшей в lockfile. Буфер также очищается при отказе конструктора; расшифрованные данные получают zeroizing guard до дальнейшей проверки. Исключение для unsafe ограничено существующей Windows ABI-границей этого модуля.
- Основной daemon/CLI явно выбираются через `default-run`; альтернативные snapshot-бинарники не удалены и не признаны production baseline.

Добавлены девять Rust regression tests для codec/filename/capacity и sensitive-buffer поведения. **Эти тесты и свежий `cargo check` в данном проходе не выполнены.** Изменения не означают зелёную сборку или принятие Windows security.

Последняя прочитанная полноценная проверка — Actions run `33937679768`, commit `78c70d0783f1449dc664ec0903f42a39f45c46a5`: FAIL. В сохранённом Linux-выводе есть ошибки форматирования, workspace check/tests/clippy/doc и выбора бинарника для запуска. Это исторический результат, а не результат проверки новых коммитов.

Runs с названиями про redb нельзя считать доказательством реализации: run `33938159506` упал на `Require clean qualified baseline`; шаги применения кода и проверки были пропущены.

## Минимальная последовательность завершения

### 1. Восстановить проверяемую сборку

Один ручной `cargo +1.98.0 check --workspace --all-targets --all-features --locked` на точном SHA. Вывод compiler diagnostics должен оставаться в обычном логе Actions. Не переписывать исходники и lockfile в workflow. Затем отдельные regression tests, format/clippy и Windows-проверка. Успех одного Linux check не закрывает Windows и runtime.

### 2. Соединить одного owner, durable control и source roots

Реальный redb adapter вместо in-memory модели; явные транзакции, поколения, idempotency и восстановление неоднозначного commit. Каталог roots должен открываться из primary daemon, сохранять разрешённые локаторы и состояние недоступных источников. Путь — локатор, не идентичность источника.

Приёмка: добавить root → остановить процесс → открыть без повторной передачи root → получить тот же каталог; проверить второй owner, исчезновение/возврат каталога, повреждение состояния и сбой между durable commit и публикацией snapshot. Недоступность не превращается в «источник пуст».

### 3. Подключить единый immutable preparation pipeline к DIRECT

Проверенная immutable revision → materializer → unitizer → сохраняемые manifest/receipt → DIRECT/result handles. Убрать production-зависимость от development scanner. Закрепить идентичность и алгоритмы digest, нумерацию ревизий и разрешение receipt; не подставлять фиктивные значения.

Приёмка: после изменения или удаления исходного файла старый handle читает точные сохранённые байты; новый запрос не смешивает ревизии. Проверить UTF-8, CRLF, совпадение через границу unit, ограничения результата, повреждённый digest, перезапуск и crash recovery. Неполное покрытие не выдаётся за доказательство отсутствия.

### 4. Подключить настоящий Qdrant data plane

Реальные Rust transport/supervisor, закреплённый совместимый Qdrant artifact, создание схемы и payload indexes, upsert с точным readback, фильтрация и публикация epoch. In-memory model оставить тестовым oracle, не fallback-backend. Snapshot/BM25 эксперимент вывести из обычного product packaging либо явно изолировать как неподдерживаемый эксперимент.

Приёмка с настоящим Qdrant: write → readback → query → source validation; неверные namespace/epoch не возвращают кандидатов; отказ, timeout и restart не создают ложной готовности. DIRECT продолжает работать при недоступном индексе.

### 5. Закрыть локальную поставку

Один воспроизводимый Rust/Cargo build и понятный запуск daemon + Qdrant на целевой Windows-системе без Python в product path. Затем end-to-end smoke на чистом data root и после перезапуска. Только после этого можно называть базовое ядро готовым к живому тестированию.

## Основные источники в репозитории

`AGENTS.md`; `docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md`; корневые `Cargo.toml` и `Cargo.lock`; `bins/eliot-searchd/{Cargo.toml,src/entry.rs,src/main.rs,src/app.rs,src/direct_store.rs,src/secure_direct_store.rs,src/source_roots.rs,src/sealed_owner_epoch.rs,src/sealed_store.rs}`; `crates/search-control-redb/{Cargo.toml,src/lib.rs}`; `crates/search-index-qdrant/search-qdrant-bridge/src/lib.rs`; `crates/search-prep/{search-materializer,search-unitizer}/src/lib.rs`; `qualification/CURRENT_FAILURES.txt`; история Actions указанных runs. Удалённые workflow сохранены в Git history исходного commit.
