# Доказательства живых проверок

Каждый файл `<изменение>.md` описывает одну проверку в запущенной 1С: дату, базу, команды, что проверено и результат — со ссылками на сырые дампы и снимки в `files/<область>/`. Инструменты, которыми это снято, — в [`scripts/1c-e2e/live/`](../../scripts/1c-e2e/live/README.md). Реестр «возможность → статус → доказательство» — [`../agent-readiness.md`](../agent-readiness.md).

Правило процесса (`todo.md`): изменение OpenSpec нельзя архивировать с пунктом «проверено в 1С», пока нет `docs/evidence/<изменение>.md`; `python scripts/check_evidence.py [--strict]` перечисляет архивные изменения без такого файла.

## Указатель

| Область | Файл | Статус |
|---|---|---|
| Привязка полей к реквизитам объекта | [bind-controls-to-object-attributes](bind-controls-to-object-attributes.md) | подтверждено, кроме `Наименование` справочника «Синхронизация плана счетов» в первом прогоне (привязка не сработала; позже работает) |
| Таблицы и колонки табличных частей | [bind-tables-to-tabular-sections](bind-tables-to-tabular-sections.md) | подтверждено; типы колонок в дампе не видны |
| Связи без потерь | [lossless-datapath-steps](lossless-datapath-steps.md) | подтверждено: дамп до и после совпадает побайтно; состав правки не записан |
| Колонки-флажки | [boolean-check-box-columns](boolean-check-box-columns.md) | подтверждено (отличие только в ширине) |
| Кнопки командной панели | [edit-command-bar-buttons](edit-command-bar-buttons.md) | подтверждено наличие, текст и действие; нажатие не проверялось |
| `new-form` | [add-new-form-command](add-new-form-command.md) | формы открываются; отчёт — см. следующую строку |
| Шаблон отчёта `new-form --kind report` | [report-form-template](report-form-template.md) | подтверждено в «Бух.»: форма открывается, `ОтчетОбъект`, вызов модуля объекта; дамп равен форме Конфигуратора |
| Панель со страницами | [add-panel-with-pages](add-panel-with-pages.md) | **только этап A**; этап B в 1С **не проверялся** (V1) |
| Страницы и `ShowTabs` существующей панели настоящего отчёта | [functional-report](functional-report.md) | подтверждено платформой (сборка `.erf` + выгрузка) для переименования, добавления, удаления, перестановки и `ShowTabs` 0 ↔ 1; в живой 1С открыты итоговая форма и перестановка/`ShowTabs` 0 → 1 (V1, V2) |
| Реквизиты формы и таблица значений | [add-form-attributes-and-value-tables](add-form-attributes-and-value-tables.md) | таблица значений — да; простые реквизиты формы — **нет артефакта** (V6) |
| Кнопки выбора у ссылочного поля | [input-field-selection-buttons](input-field-selection-buttons.md) | **не закрыто**, только состояние «до» (V5) |
| Элементы в формах потока версий 25 и 26 | [controls-in-older-stream-versions](controls-in-older-stream-versions.md) | подтверждено для 4 видов; остальные виды — отказ `unsupported-stream-version` |
| Находки пилота (этапы 1–3): новая командная панель, предупреждения, `clone-form .` | [pilot-findings](pilot-findings.md) | подтверждено в «Бух.»; ограничения в файле |
| Командные панели: 14 кодов `Standard:N`, подменю 3 уровней (V4, V7) | [verification-debt-night](verification-debt-night.md) | подтверждено в «Бух.»; нажатие кнопок не проверено |
| Сплошная проверка свойств | [property-sweep](property-sweep.md) | частично: 9 наблюдаемых изменений из 40 мутаций, 1 566 строк кнопок панелей не разобраны |
| `clone-form` | [clone-form](clone-form.md) | обработка и отчёт — да; справочник — нет дампа |
| Добавление элемента в малые формы | [add-remove-elements-small-forms](add-remove-elements-small-forms.md) | надпись в 3 формах; удаление и 26 типов — нет |
| Версия потока формы: `DESIGNER /ConvertFiles` и цикл выгрузка → сборка | [convert-files-experiment](convert-files-experiment.md) | `ConvertFiles` файлы не меняет (версия 25 / 26 прежняя); выгрузка в XML и сборка поднимают 25 / 26 → 27 с перестройкой потока; только платформа 8.3.27 |

## Как читать файлы

* `*.txt.gz` — дамп свойств запущенной формы (`gzip -dc`), UTF-8 с BOM, строки `pre:<элемент>.<свойство>=<Тип>|<значение>` (до показа формы) и `post:…` (после); `*.txt` без архива — малые дампы. `@object`, `@count` — служебные строки.
* Значения «организация» и «физическое лицо» из базы заменены на `<организация>` и `<физлицо>`; снимки экрана обрезаны до окна формы (без рабочего стола и заголовка программы).
* Правленные `Form.xml` прогонов, как правило, **не сохранялись** (лежали в `target/`); запрос читается по описанию в `tasks.md` и по результату. Исключение — повторно выгруженный `Form.xml` таблицы значений.
* Суммарный размер каталога ≈ 1,5 МБ. Крупные дампы (по 6 МБ) сжаты; остальные дампы вариантов `e2e-*.txt` в git не вошли.

## Долг: архивные изменения без файла доказательств

Эвристика `scripts/check_evidence.py` (строки `tasks.md` со словами «1C», «1С», «live», «живая»): часть совпадений — упоминания тестов, не проверок; но «Validate … in 1C Designer» в `pack-existing-form-xml` и «Checked in 1C» в `pack-added-control-anchors` — настоящие проверки без артефактов. Вывод на 28.10.2026:

| Изменение (архив) | Упоминаний 1С/live в tasks.md | docs/evidence/<имя>.md | Первое упоминание |
|---|---|---|---|
| 2026-10-23-add-usualgroup-choicefield-and-spreadsheet-events | 1 | **нет** | - [x] 3.1 Update `tests/pack_existing_form_xml.rs` for the 26-type `donor.epf` (testing `UsualGroup` and `C... |
| 2026-10-23-form-property-exploration-corpus | 2 | **нет** | - [x] 3.2 Gate platform-backed regeneration behind an explicit opt-in (env/flag) so default `cargo test` ne... |
| 2026-10-23-pack-discovered-control-properties | 1 | **нет** | - [x] 3.1 Add EPF and configuration `Form.bin` integration tests in `tests/pack_existing_form_xml.rs` cover... |
| 2026-10-23-pack-existing-control-events-and-renames | 1 | **нет** | - [x] 3.1 Add EPF and configuration `Form.bin` integration tests in `tests/pack_existing_form_xml.rs` for r... |
| 2026-10-23-pack-existing-form-xml | 1 | **нет** | - [x] 6.2 Validate packed representative forms in 1C Designer by opening, inspecting changed and untouched ... |
| 2026-10-23-pack-form-attributes-and-datapath | 1 | **нет** | - [x] 3.1 Add EPF and configuration `Form.bin` integration tests in `tests/pack_existing_form_xml.rs` for a... |
| 2026-10-23-pack-table-columns-and-panel-pages | 1 | **нет** | - [x] 3.1 Add EPF and configuration `Form.bin` integration tests in `tests/pack_existing_form_xml.rs` for e... |
| 2026-10-24-pack-control-anchors | 1 | **нет** | - [x] 3.2 Add integration tests in `tests/pack_existing_form_xml.rs` (both EPF and config targets) and veri... |
| 2026-10-24-pack-extended-control-catalog-properties | 1 | **нет** | - [x] 2.2 Verify with `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, and a real 1C Des... |
| 2026-10-24-pack-inputfield-flags-and-typelink | 1 | **нет** | - [x] 3.1 Add unit tests in `tests/form_patch.rs` and EPF/config integration tests in `tests/pack_existing_... |
| 2026-10-24-pack-ref-valuetypes-and-typelink | 1 | **нет** | - [x] 4.1 Add EPF and configuration `Form.bin` integration tests in `tests/pack_existing_form_xml.rs` and v... |
| 2026-10-24-pack-remaining-form-and-control-properties | 1 | **нет** | - [x] 3.1 Add EPF and configuration `Form.bin` integration tests in `tests/pack_existing_form_xml.rs` and v... |
| 2026-10-24-pack-table-grid-and-flags | 1 | **нет** | - [x] 3.1 Add unit tests in `tests/form_patch.rs` and EPF/config integration tests in `tests/pack_existing_... |
| 2026-10-26-bind-controls-to-object-attributes | 1 | есть | - [x] 1.7 Check in 1C: processor (`Объект.Реквизит`) and a catalog and a document of the Бух. test base (va... |
| 2026-10-26-bind-tables-to-tabular-sections | 2 | есть | - [x] 1.1 S1: `TabularSchema` type id (XML InternalInfo and processor container); table link, value type an... |
| 2026-10-26-lossless-datapath-steps | 1 | есть | - [x] 1.4 Checks in 1C: a form of the Бух. base with a multi-step link (`Движения`) keeps its runtime `Данн... |
| 2026-10-26-pack-added-control-anchors | 1 | **нет** | - [x] 1.4 Checked in 1C: add, remove and edit properties of the main form of the test processor. |
| 2026-10-27-add-form-attributes-and-value-tables | 1 | есть | - [x] 1.3 Checked in 1C (external processor): a table bound to a value-table attribute with String, Number ... |
| 2026-10-27-add-new-form-command | 1 | есть | - [x] 1.3 Checks in 1C (test base «Бух.»): new catalog item, catalog list, document, processor and report f... |
| 2026-10-27-add-panel-with-pages | 2 | есть | - [x] 1.2 Stage B: rename, add, remove pages of an existing panel; renumbering (unit-tested; not yet checke... |
| 2026-10-27-boolean-check-box-columns | 1 | есть | - [x] 1.4 Checked in 1C: a table with a check-box column in the Бух. base has the properties of the existin... |
| 2026-10-27-edit-command-bar-buttons | 1 | есть | - [x] 1.6 Checks in 1C (test base «Бух.»): added handler button and separator, a user sub-menu with a handl... |

Всего с упоминанием: 22; без файла доказательств: 14

Что делать: либо найти и положить дамп (если проверка была), либо заменить в `tasks.md` «проверено в 1С» на «тесты» (если не была) и убрать изменение из списка.
