# HOW-WE-APPLY (внутренний документ, дизайнеру не отдавать)

Как мы применяем возвращённую папку `response/` (style-sheet.html, render-spec.md, theme.css, screenshots/, questions.md) к `assets/form_theme.css` и `assets/form_render.js`.

## 0. Приёмка

1. `python docs/design/preview-designer/tools/check_spec.py --dir response/` — нет пропущенных ID, лист образцов самодостаточен.
2. Открыть `style-sheet.html` офлайн в Edge и Chrome; сравнить со `screenshots/`.
3. Прочитать `questions.md`: что просят снять (`gallery`-скрипт `target/designer/gallery/tools/…`, `live.py`), неясности — ответить до внедрения.
4. Прочитать render-spec §0 и §1: константы и общие структуры — основа всего остального.

## 1. Карта «ID → место в коде»

| Что в render-spec | Куда | Заметки |
|---|---|---|
| §0.1–0.4 константы, `theme.css` переменные | `assets/form_theme.css`: `#canvas{…}` (CSS:17), `#canvas.theme-xp` (CSS:19), общие правила `.c` (CSS:20) | новые переменные — в `#canvas`; **определить `--field-border`** (используется в CSS:104–125, нигде не задана) |
| §0.5 значки | `assets/form_render.js`: объект `ICON` (SVG для кнопок командных панелей, подписей) и `inputField()` (кнопки выбора); для «стрелок закладок», значков сортировки, галочки/точки — новые вставки в `tableBox`, `tabsHtml`, `ctl` | значки как inline `<svg>`/data-URI, без файлов |
| `InputField.*` | CSS:35–40 (`.t-InputField`, `.val`, `.num`, `.ph`, `.mark`, `.tb`); JS `inputField()` (JS:269): классы по ChoiceButton/ClearButton/OpenButton/ReadOnly/Font/цвета | JS нужен для: ReadOnly/Enabled-класс, Font (bold/italic/underline/strikeout/face/size), FieldBackColor/FieldTextColor/BorderColor, MultiLine, PasswordMode, HorizontalAlign, DropListButton/SpinButton |
| `LabelDecoration.*` | CSS:26–32; JS `label()` (JS:288) | Font целиком (не только bold), VerticalAlign, `kind=style` цвета |
| `Button.*`, `CheckBox.*`, `RadioButton.*` | CSS:44–49; JS `ctl()` ветки Button/CheckBox/RadioButton (JS:424+) | галочка/точка по `value`? **значение не читается из формы** — решить: показывать всегда снятым (как сейчас) или образцом |
| `Table.*`, `TableColumn.*` | CSS:72–85; JS `tableBox()` (JS:309) | HeaderFont/цвета/HeaderHeight, VerticalLines/HorizontalLines, UseAlternationRowColor — новые inline-стили/классы |
| `Panel.*`, `PanelPage.*` | CSS:60–69; JS `tabsHtml()` (JS:399) | ShowTabs member-значения (Top/Bottom/Left/Right) пока по геометрии |
| `UsualGroup.*` | CSS:52–57; JS `groupBox()` (JS:302) | Font/TextColor заголовка сейчас жёстко |
| `CommandBar.*`, `CommandBarButton.*` | CSS:88–100; JS `commandBar()` (JS:377), `cbtn()` (JS:348), `btnIcon()` (JS:337), `autoItems()` (JS:368) | |
| `PictureDecoration.*` | CSS:105–106; JS `label()` + `ctl()` (pic-box) | |
| `SpreadsheetDocumentField`, `TextDocumentField`, `CalendarField`, `ProgressBar`, `TrackBar`, `ListBox`, `Splitter`, `HTMLDocumentField`, `GraphicalSchemaField` | CSS:103–126; JS `scrollDocBox()` (JS:420), `calendarBox()` (JS:411), `ctl()` | |
| `Font.*`, `Color.*`, `Border.*`, `Picture.*` (§1) | новые JS-хелперы: `fontCss(e)` (читает `Font@*`), `colorCss(e)` (rgb → hex; `kind=style` → `value`, иначе ничего), общие для всех классов | единственный источник перевода XML→CSS; заменить частные чтения (`bold`, `TextColor`, `BackColor`) |
| Состояния (disabled/readonly/selected/checked) | классы `.dis`, `.ro`, `.sel`, `.on` на `div.c` (JS) + правила в CSS | `Enabled=false`, `ReadOnly=true` читаются из XML в `parse()` (JS:96) |

Номера строк — по `assets/form_theme.css`/`assets/form_render.js` на момент создания пакета; ориентир — имена функций.

## 2. Порядок внедрения

1. **Тема и константы**: перенести §0/`theme.css` в `assets/form_theme.css` (переменные, шрифты, палитра, определить `--field-border`). Пересобрать (см. п. 4) и убедиться, что формы не сломались (раскладка не меняется).
2. **Общие хелперы** `fontCss`/`colorCss`/рамки по §1.
3. **Класс за классом**, начиная с самых частых (по таблице частот каталога: Label, InputField, Table/Column, CommandBar, Button, Panel/Page, UsualGroup, CheckBox/Radio). Для каждой строки «ID | условие | действие» — реализовать либо CSS-правило (если достаточно класса), либо JS (если нужно читать новое свойство из XML).
4. Новые свойства, которые надо читать из XML, добавлять в `parse()` (модель `C[]`), не в `ctl()`.
5. После каждого класса — сверка (п. 3 ниже). Коммит — по классу.
6. Обновить `docs/property-coverage-map.md` (какие свойства теперь рисуются) и golden-превью (`tests/preview_golden.rs`, `GOLDEN_BLESS=1`), т. к. HTML изменится.
7. `cargo build` — ассеты вшиваются через `include_str!` (`src/preview.rs`:68–78).

## 3. Проверка каждого правила

Для каждой строки таблицы «Свойства → правила»:

1. Найти в `style-sheet.html` образец с `data-xml` этого ID и значением.
2. Взять **эталонную форму** с таким свойством: `rg -l '<Tag>' …` по корпусам (`target/a1-live/cfgx`, `target/corpus/ut2`) или составить минимальный `Form.xml` (скопировать образец из `tools/samples/` и добавить тег; такие формы класть в `tests/fixtures/preview-style/`).
3. Собрать превью: `python docs/design/preview-designer/tools/rebuild_preview.py <форма> -o target/designer/out/<id>.html --theme assets/form_theme.css --render assets/form_render.js`, снять PNG Edge headless (`--screenshot`, `?chrome=0`).
4. Снять PNG образца из `style-sheet.html` (элемент `figure.specimen[data-xml*="ID"]`, Edge headless + `--window-size`, либо скриншот страницы и кадрирование по `getBoundingClientRect` через небольшой скрипт).
5. **Сравнить**: визуально бок о бок и числом — попиксельный diff (`PIL.ImageChops.difference`, порог: среднее отклонение канала < 6/255 и нет различий по контуру > 1 px). Расхождения — в правило (если дизайнер описал неточно → вопрос ему) или в наш код.
6. Для классов целиком — сверка с `gallery/NN-*-1c.png`: пары «1С — превью» должны стать визуально ближе (диф-метрика и взгляд); обновить `04-gallery.md`.
7. Регрессия: `cargo test` (golden `preview_golden`), `rebuild_preview.py` на всех 5 образцах `tools/samples/` — координаты и габариты каждого `.c` не изменились (сравнить `getBoundingClientRect` до/после скриптом: допускается отличие 0 px).

## 4. Как пересобирать без Rust

`python docs/design/preview-designer/tools/rebuild_preview.py <форма> -o out.html --theme assets/form_theme.css --render assets/form_render.js` (скрипт читает шаблон из `tools/skeleton/preview_template.rs.txt`; если `src/preview.rs` менялся — обновить снимок). Для проверки результата с настоящим CLI — `cargo build --release && target/release/oneform.exe preview <каталог>`.

## 5. Чего дизайнер не решает (оставляем за собой)

Координаты и раскладка, чтение свойств из XML, режимы «Конфигуратор»/«Предприятие», инспектор и подсветки, тема `xp`, поведение при отсутствии данных (образцы значений). Если правило дизайнера требует нового чтения свойства — это задача по JS, учитывайте объём.

## 6. Скрипты и данные

| Файл | Назначение |
|---|---|
| `scripts/gen_designer_catalog.py` | Генерация `02-property-catalog.md` (+ `internal/scan-summary.json` с примерами файлов-источников сниппетов) из корпуса и данных |
| `scripts/designer_catalog_data.py` | Курируемые строки: какие свойства «визуальные», что делает превью, ссылки на эталоны |
| `scripts/gen_render_spec_skeleton.py` | Генерация `render-spec.skeleton.md` |
| `scripts/copy_designer_gallery.py` | Копирование/сжатие галереи в `docs/design/preview-designer/gallery/` и генерация `04-gallery.md` |
| `scripts/check_md_tables.py` | Проверка, что во всех таблицах Markdown одинаковое число ячеек |

После изменения правил превью (`assets/*`) обновить колонку «Что делает превью сейчас»: поправить `designer_catalog_data.py` и перегенерировать каталог.
