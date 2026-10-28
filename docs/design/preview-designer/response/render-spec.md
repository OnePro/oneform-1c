# render-spec.md — как собрать каждую часть формы 1С в HTML/CSS

Язык: русский. Формулировки правил — «если в XML …, то в HTML …». Все цвета, размеры и значки — в §0 и в `theme.css` (переменные на `#canvas`). Классы-модификаторы (`.ro`, `.dis`, `.b`, `.ha-*`, `.bd-*`…) ставит `form_render.js` по свойствам Form.xml; CSS к ним уже есть в `theme.css`. Образцы на листе: `style-sheet.html`, атрибут `data-xml` с тем же ID. ROOT в таблицах — корневой `<div class="c t-<Класс>">` элемента.

## §0. Константы темы

Все значения ниже стоят в `theme.css` (переменные на `#canvas, .canvas`) и нарисованы на `style-sheet.html` (раздел «Тема»). Цвета сняты попиксельно со снимков `gallery/*-1c.png`; где эталона нет — помечено «допущение» и вынесено в `questions.md`.

### 0.1 Шрифты

| Роль | font-family | размер | начертание | где применяется |
|---|---|---|---|---|
| Текст формы | `Tahoma, Arial, sans-serif` (`var(--font)`) | 8 pt (≈ 11 px) | обычный | все `.c` холста: подписи, значения полей, ячейки |
| Кнопка | как текст формы | 8 pt | обычный; кнопка по умолчанию (`.def`, «OK» `.cbtn.ok`) — жирный | `.t-Button .cap`, `.cbtn .cap` |
| Таблица (ячейки) | как текст формы | 8 pt | обычный | `.t-Table .cell` |
| Шапка таблицы | как текст формы | 8 pt | обычный (жирный только по `HeaderFont`) | `.t-Table .colh` |
| Закладка | как текст формы | 8 pt | обычный, у активной — тоже обычный | `.ptab` |
| Заголовок группы | как текст формы | 8 pt | жирный, цвет `var(--group-title)` | `.t-UsualGroup .gt` |
| Подсказка | — | — | — | не рисуется на холсте; текст `Tooltip` в `title=` (браузерная подсказка) |
| Заголовок окна (вне холста) | `Tahoma, Arial, sans-serif` | 8 pt | жирный | `.form-title` |
| Образец значения (designer) | как текст формы | 8 pt | обычный, цвет текста (не курсив) | `.val` |

### 0.2 Цвета (имя CSS-переменной → значение)

| Переменная | Значение | Роль |
|---|---|---|
| `--form-bg` | `#FCFAEB` | фон формы, страниц панели, активной закладки |
| `--cmdbar-bg` | `#F5F2DD` | командная панель, нижняя полоса кнопок |
| `--border` | `#B3AC86` | рамка полей, таблиц, кнопок, страницы, картинки, линия группы |
| `--field-border` | `#B3AC86` | то же для «прочих» полей (раньше переменная не была определена) |
| `--sep` | `#CCC6AD` | рамка неактивной закладки, разделители командной панели, низ шапки таблицы |
| `--tab-bg` | `#EBE6CC` | неактивная закладка |
| `--tab-accent` | `#FADFA3` | оранжевая полоска 2 px сверху активной закладки |
| `--head-from` | `#FAFAF0` | шапка/подвал таблицы: верх градиента |
| `--head-to` | `#F2EBCC` | шапка/подвал таблицы: низ градиента |
| `--head-sep` | `#BFB999` | вертикальный разделитель колонок в шапке |
| `--row-line` | `#CBD2D6` | горизонтальная линия между строками таблицы |
| `--col-line` | `#E3E0D0` | вертикальная линия между ячейками (VerticalLines=true) |
| `--sel-bg` | `#536AC2` | текущая (выделенная) строка таблицы, выделение в календаре/списке |
| `--sel-text` | `#FFFFFF` | текст выделенной строки/даты |
| `--sel-line` | `#B5CBE5` | нижняя светлая линия выделенной строки |
| `--group-title` | `#9F6500` | заголовок группы |
| `--scroll-bg` | `#FAF9F3` | жёлоб полосы прокрутки |
| `--scroll-border` | `#BDB79B` | линия, отделяющая полосу прокрутки |
| `--scroll-arrow` | `#A8A38E` | треугольники на кнопках прокрутки |
| `--tb-from` | `#FAF8F2` | кнопка внутри поля («…», календарь, «×», лупа): верх градиента |
| `--tb-to` | `#EAE6CD` | … низ градиента |
| `--tb-sep` | `#D3CDAA` | левая граница кнопки внутри поля |
| `--tb-ink` | `#664C00` | точки «…», крестик «×» |
| `--btn-from` | `#FFFFFF` | кнопка: верх градиента |
| `--btn-mid` | `#F6F3E1` | кнопка: середина |
| `--btn-to` | `#EAE5CD` | кнопка: низ |
| `--check-border` | `#99926B` | рамка флажка и переключателя |
| `--check-ink` | `#2B2B2B` | галочка, точка переключателя |
| `--text` | `#000000` | текст формы |
| `--text-dis` | `#A39E87` | текст недоступного элемента (допущение, см. questions.md) |
| `--link` | `#0054E3` | гиперссылка (допущение) |
| `--mark` | `#FF0000` | отметка незаполненного (точки 1 px через 1 px) |
| `--neg` | `#FF0000` | отрицательное число при MarkNegatives |
| `--holiday` | `#C00000` | выходные в календаре |
| `--ph-bg` | `#F3F0DE` | заглушка картинки, которую нельзя показать |
| `--ph-ink` | `#B3AC86` | линии заглушки картинки |
| `--font` | `8pt Tahoma,Arial,sans-serif` | шрифт формы (≈ 11 px) |

### 0.3 Рамки, тени, скругления

| Что | CSS |
|---|---|
| Рамка поля ввода / поля выбора | `border:1px solid var(--border); border-radius:var(--r-field)` (2 px) |
| Поле «только просмотр» | `background:var(--form-bg); box-shadow:inset 0 0 0 2px #FFFFFF` (рамка та же) |
| Кнопка внутри поля | `border-left:1px solid var(--tb-sep); background:linear-gradient(180deg,var(--tb-from),var(--tb-to))` |
| Кнопка | `border:1px solid var(--border); border-radius:var(--r-btn)` (3 px); `background:linear-gradient(180deg,var(--btn-from) 0,var(--btn-mid) 45%,var(--btn-to) 100%)`; `box-shadow:inset 0 -1px 0 rgba(255,255,255,.55)` |
| Флажок | `border:1px solid var(--check-border); border-radius:var(--r-check)` (2 px); `background:linear-gradient(135deg,#F4F2E8 0,#FFFFFF 70%)` |
| Переключатель | то же, `border-radius:50%` |
| Таблица / картинка / документы / список | `border:1px solid var(--border)`, без скругления |
| Страница панели | `border:1px solid var(--border)`; без закладок — без рамки |
| Закладка | неактивная: `border:1px solid var(--sep); border-bottom:none; border-radius:var(--r-tab) var(--r-tab) 0 0` (4 px); активная: `border-color:var(--border); box-shadow:inset 0 2px 0 var(--tab-accent)` |
| Группа Frame | `.gf{border:1px solid var(--border); border-radius:3px; top:7px}` |
| Группа SingleLine | `.gf{border-bottom:1px solid var(--border)}` |
| Командная панель | `border-bottom:1px solid var(--border)`; нижняя полоса формы — `border-top` |
| Полоса прокрутки | жёлоб `var(--scroll-bg)`, линия `1px solid var(--scroll-border)`, кнопки `var(--cmdbar-bg)` без рамки, бегунок `linear-gradient(#FFFFFF,#ECE8D3)` + `1px solid var(--scroll-border)` + `border-radius:2px` |
| Тени | у элементов теней нет; тень только у оболочки `.formwrap` (не часть формы) |

### 0.4 Размеры

| Что | Значение |
|---|---|
| Ширина кнопки выбора в поле | `--tb-w` = 15 px (+ 1 px левой границы) — замер «…» в 02 |
| Значки внутри поля | 12×12 |
| Значки командной панели и строк таблицы | 16×16 (`.ic`) |
| Высота кнопки командной панели | 21 px, поля 4 px по горизонтали |
| Разделитель командной панели | 1×16 px, отступы 2 px |
| Флажок / переключатель | 13×13 px, зазор до подписи 3 px |
| Высота шапки таблицы | `--head-h` = 20 px (HeaderHeight N → 20 + (N−1)·13) |
| Шаг строки таблицы | `--row-h` = 19 px (18 + линия 1 px) |
| Высота подвала таблицы | 20 px |
| Полоса прокрутки | `--sb` = 16 px |
| Закладка | высота 18 px, активная 19 px (выступает на 1 px вниз), поля 8 px |
| Стрелки прокрутки закладок | 9×14 px каждая |
| Заголовок группы | высота строки 14 px, отступ 6 px слева (Frame) |
| Отметка незаполненного | точки 1×1 px с шагом 2 px, на 1 px выше нижней границы, отступ 2 px слева/справа |
| Заголовок окна | 22 px |
| Единица координат | 1 единица Form.xml = 1 CSS-px (без изменений) |

### 0.5 Значки

Все значки — inline SVG, без внешних ссылок; цвета заданы в самих SVG. В превью они заменяют текущий набор `ICON.*` в `form_render.js` (имена соответствуют). Разметка значка в DOM: `<span class="ic">SVG</span>` (16×16) или прямо в `.tb` (12×12).

| Имя | Размер | Где | SVG |
|---|---|---|---|
| `icon-choice` | 12×12 | кнопка выбора «…» (ChoiceButton, ссылочный/строковый тип) | `<svg width="12" height="12" viewBox="0 0 12 12" xmlns="http://www.w3.org/2000/svg"><rect x="1" y="8" width="2" height="2" fill="#664C00"/><rect x="5" y="8" width="2" height="2" fill="#664C00"/><rect x="9" y="8" width="2" height="2" fill="#664C00"/></svg>` |
| `icon-calendar` | 12×12 | кнопка выбора даты (ChoiceButton, тип Дата) | `<svg width="12" height="12" viewBox="0 0 12 12" xmlns="http://www.w3.org/2000/svg"><rect x="1.5" y="1.5" width="9" height="9" fill="#fff" stroke="#664C00"/><path d="M1.5 4.5h9M4.5 4.5v6M7.5 4.5v6M1.5 7.5h9" stroke="#887437"/></svg>` |
| `icon-calc` | 12×12 | кнопка калькулятора (ChoiceButton, тип Число) | `<svg width="12" height="12" viewBox="0 0 12 12" xmlns="http://www.w3.org/2000/svg"><rect x="1.5" y="1.5" width="9" height="9" fill="#fff" stroke="#664C00"/><rect x="3" y="3" width="6" height="2" fill="#887437"/><path d="M3 7h1.5M5.25 7h1.5M7.5 7h1.5M3 9h1.5M5.25 9h1.5M7.5 9h1.5" stroke="#664C00"/></svg>` |
| `icon-type` | 12×12 | кнопка выбора типа «T» (ChoiceButton, составной тип) | `<svg width="12" height="12" viewBox="0 0 12 12" xmlns="http://www.w3.org/2000/svg"><path d="M2 2.5h8v2H7v6H5v-6H2z" fill="#3D3D3D"/></svg>` |
| `icon-clear` | 12×12 | кнопка очистки «×» (ClearButton) | `<svg width="12" height="12" viewBox="0 0 12 12" xmlns="http://www.w3.org/2000/svg"><path d="M3 3l6 6M9 3l-6 6" stroke="#664C00" stroke-width="1.8"/></svg>` |
| `icon-open` | 12×12 | кнопка открытия «лупа» (OpenButton) | `<svg width="12" height="12" viewBox="0 0 12 12" xmlns="http://www.w3.org/2000/svg"><circle cx="5" cy="5" r="3.3" fill="#fff" stroke="#5A5A5A" stroke-width="1.4"/><path d="M7.4 7.4L10.6 10.6" stroke="#5A5A5A" stroke-width="2"/></svg>` |
| `icon-drop` | 7×4 | стрелка списка ▾ (DropListButton, ChoiceField, Button.MenuMode, подменю) | `<svg width="7" height="4" viewBox="0 0 7 4" xmlns="http://www.w3.org/2000/svg"><path d="M0 0h7L3.5 4z" fill="#3D3D3D"/></svg>` |
| `icon-spin` | 7×9 | кнопка регулирования ▲▼ (SpinButton) | `<svg width="7" height="9" viewBox="0 0 7 9" xmlns="http://www.w3.org/2000/svg"><path d="M0 3.5h7L3.5 0zM0 5.5h7L3.5 9z" fill="#3D3D3D"/></svg>` |
| `icon-check` | 9×8 | галочка флажка (рисуется CSS ::after у .box.on; SVG — для ячеек таблицы) | `<svg width="9" height="8" viewBox="0 0 9 8" xmlns="http://www.w3.org/2000/svg"><path d="M1 4l2.5 2.5L8 1.5" fill="none" stroke="#2B2B2B" stroke-width="2"/></svg>` |
| `icon-sort-asc` | 7×4 | значок сортировки ▲ в шапке колонки | `<svg width="7" height="4" viewBox="0 0 7 4" xmlns="http://www.w3.org/2000/svg"><path d="M0 4h7L3.5 0z" fill="#8C8670"/></svg>` |
| `icon-tree-plus` | 9×9 | узел дерева «+» | `<svg width="9" height="9" viewBox="0 0 9 9" xmlns="http://www.w3.org/2000/svg"><rect x=".5" y=".5" width="8" height="8" fill="#fff" stroke="#A9A9A9"/><path d="M2 4.5h5M4.5 2v5" stroke="#333"/></svg>` |
| `icon-folder` | 16×16 | значок группы в строке таблицы | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M1.5 4.5h5l1 1.5h7v7.5h-13z" fill="#F7D774" stroke="#C9A227"/><path d="M1.5 6.5h13" stroke="#E8BE3C"/></svg>` |
| `icon-item` | 16×16 | значок элемента в строке таблицы | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="3" y="7" width="10" height="3" rx="1" fill="#7FA6D6" stroke="#3A6EA5"/></svg>` |
| `icon-arrow-left` | 5×7 | стрелка прокрутки ◂ (таблица, закладки) | `<svg width="5" height="7" viewBox="0 0 5 7" xmlns="http://www.w3.org/2000/svg"><path d="M4.5 0v7L0 3.5z" fill="#A8A38E"/></svg>` |
| `icon-arrow-right` | 5×7 | стрелка прокрутки ▸ | `<svg width="5" height="7" viewBox="0 0 5 7" xmlns="http://www.w3.org/2000/svg"><path d="M.5 0v7L5 3.5z" fill="#A8A38E"/></svg>` |
| `icon-arrow-up` | 7×5 | стрелка прокрутки ▴ | `<svg width="7" height="5" viewBox="0 0 7 5" xmlns="http://www.w3.org/2000/svg"><path d="M0 4.5h7L3.5 0z" fill="#A8A38E"/></svg>` |
| `icon-arrow-down` | 7×5 | стрелка прокрутки ▾ | `<svg width="7" height="5" viewBox="0 0 7 5" xmlns="http://www.w3.org/2000/svg"><path d="M0 .5h7L3.5 5z" fill="#A8A38E"/></svg>` |
| `icon-tab-left` | 5×7 | стрелка прокрутки закладок ◀ (активная) | `<svg width="5" height="7" viewBox="0 0 5 7" xmlns="http://www.w3.org/2000/svg"><path d="M4.5 0v7L0 3.5z" fill="#3D3D3D"/></svg>` |
| `icon-tab-right` | 5×7 | стрелка прокрутки закладок ▶ (активная) | `<svg width="5" height="7" viewBox="0 0 5 7" xmlns="http://www.w3.org/2000/svg"><path d="M.5 0v7L5 3.5z" fill="#3D3D3D"/></svg>` |
| `icon-picture` | 16×16 | заглушка картинки (Picture задан ссылкой refGuid — пикселей в Form.xml нет) | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="1.5" y="2.5" width="13" height="11" fill="#fff" stroke="#B3AC86"/><path d="M3 12l3.5-4 2.5 3 2-2 2 3z" fill="#C9C2A0"/><circle cx="11" cy="5.5" r="1.3" fill="#E8C34A"/></svg>` |
| `icon-user` | 16×16 | картинка «пользователь» у надписи автора (Picture=true без refGuid) | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><circle cx="8" cy="5" r="2.8" fill="#E8A33C" stroke="#9A5B00"/><path d="M2.5 15c0-3.2 2.5-5 5.5-5s5.5 1.8 5.5 5z" fill="#4C7BC5" stroke="#274A85"/></svg>` |
| `icon-add` | 16×16 | Добавить | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><circle cx="8" cy="8" r="6.5" fill="#43A047" stroke="#2E7D32"/><path d="M8 4.5v7M4.5 8h7" stroke="#fff" stroke-width="2"/></svg>` |
| `icon-add-group` | 16×16 | Добавить группу | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M1.5 4.5h5l1 1.5h7v7.5h-13z" fill="#F7D774" stroke="#C9A227"/><circle cx="11.5" cy="11.5" r="3.5" fill="#43A047"/><path d="M11.5 9.5v4M9.5 11.5h4" stroke="#fff" stroke-width="1.4"/></svg>` |
| `icon-copy` | 16×16 | Скопировать | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="2.5" y="1.5" width="8" height="10" fill="#fff" stroke="#6B6B6B"/><rect x="5.5" y="4.5" width="8" height="10" fill="#fff" stroke="#6B6B6B"/><circle cx="12" cy="12.5" r="3" fill="#43A047"/><path d="M12 11v3M10.5 12.5h3" stroke="#fff"/></svg>` |
| `icon-edit` | 16×16 | Изменить | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M3 13l1-3.5 7-7 2.5 2.5-7 7z" fill="#8BC34A" stroke="#4E7A1E"/><path d="M3 13l1-3.5 2.5 2.5z" fill="#F0D9A0" stroke="#4E7A1E"/></svg>` |
| `icon-delete` | 16×16 | Удалить / Пометить на удаление | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="2.5" y="2.5" width="11" height="11" fill="#fff" stroke="#B0B0B0"/><path d="M5 5l6 6M11 5l-6 6" stroke="#E53935" stroke-width="2"/></svg>` |
| `icon-save` | 16×16 | Закончить редактирование / Записать | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="2.5" y="2.5" width="11" height="11" fill="#7A92B8" stroke="#3B5579"/><rect x="5" y="2.5" width="6" height="4" fill="#fff"/><rect x="5" y="9" width="6" height="4.5" fill="#DCDCDC"/></svg>` |
| `icon-up` | 16×16 | Переместить вверх | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M8 2.5l5 5h-3v6H6v-6H3z" fill="#5C8FD6" stroke="#2F5F9A"/></svg>` |
| `icon-down` | 16×16 | Переместить вниз | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M8 13.5l5-5h-3v-6H6v6H3z" fill="#5C8FD6" stroke="#2F5F9A"/></svg>` |
| `icon-sort-az` | 16×16 | Сортировать по возрастанию | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><text x="1" y="7" font-size="7" font-family="Tahoma" font-weight="bold" fill="#333">А</text><text x="1" y="15" font-size="7" font-family="Tahoma" font-weight="bold" fill="#333">Я</text><path d="M12 3v10M9.5 10.5L12 13l2.5-2.5" stroke="#2F6FD0" fill="none" stroke-width="1.4"/></svg>` |
| `icon-sort-za` | 16×16 | Сортировать по убыванию | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><text x="1" y="7" font-size="7" font-family="Tahoma" font-weight="bold" fill="#333">Я</text><text x="1" y="15" font-size="7" font-family="Tahoma" font-weight="bold" fill="#333">А</text><path d="M12 3v10M9.5 5.5L12 3l2.5 2.5" stroke="#2F6FD0" fill="none" stroke-width="1.4"/></svg>` |
| `icon-filter` | 16×16 | Установить отбор | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M2.5 3h11l-4 5v5l-3-1.5V8z" fill="#9FC3E8" stroke="#2F5F9A"/></svg>` |
| `icon-filter-value` | 16×16 | Отбор по значению | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M2.5 3h11l-4 5v5l-3-1.5V8z" fill="#F2C94C" stroke="#8A6D00"/></svg>` |
| `icon-filter-history` | 16×16 | История отборов | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M2.5 3h11l-4 5v5l-3-1.5V8z" fill="#9FC3E8" stroke="#2F5F9A"/><path d="M12.5 10v5M10 12.5h5" stroke="#333"/></svg>` |
| `icon-filter-off` | 16×16 | Отключить отбор | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M2.5 3h11l-4 5v5l-3-1.5V8z" fill="#E6E6E6" stroke="#8A8A8A"/><path d="M9.5 9.5l5 5M14.5 9.5l-5 5" stroke="#E53935" stroke-width="1.6"/></svg>` |
| `icon-refresh` | 16×16 | Обновить | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M13 8a5 5 0 1 1-1.5-3.6" fill="none" stroke="#43A047" stroke-width="2"/><path d="M13.5 1.5v4h-4z" fill="#43A047"/></svg>` |
| `icon-output-list` | 16×16 | Вывести список | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="2.5" y="2.5" width="11" height="11" fill="#fff" stroke="#6B6B6B"/><path d="M4.5 5.5h7M4.5 8h7M4.5 10.5h4" stroke="#8A8A8A"/><circle cx="12" cy="12" r="2.6" fill="#43A047"/></svg>` |
| `icon-hierarchy` | 16×16 | Иерархический просмотр | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="1.5" y="2.5" width="6" height="4" fill="#F7D774" stroke="#C9A227"/><rect x="8.5" y="9.5" width="6" height="4" fill="#fff" stroke="#6B6B6B"/><path d="M4.5 6.5v5h4" fill="none" stroke="#6B6B6B"/></svg>` |
| `icon-help` | 16×16 | Справка | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><circle cx="8" cy="8" r="6.5" fill="#F0A830" stroke="#B26B00"/><text x="5.3" y="11.6" font-size="9.5" font-weight="bold" font-family="Tahoma" fill="#fff">?</text></svg>` |
| `icon-movements` | 16×16 | Движения документа (Дт/Кт) | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><text x=".5" y="7.5" font-size="6.5" font-weight="bold" font-family="Tahoma" fill="#2F5F9A">Дт</text><text x="6" y="14.5" font-size="6.5" font-weight="bold" font-family="Tahoma" fill="#B03030">Кт</text></svg>` |
| `icon-print` | 16×16 | Печать | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="4.5" y="1.5" width="7" height="5" fill="#fff" stroke="#6B6B6B"/><rect x="1.5" y="6.5" width="13" height="6" fill="#B8C4D6" stroke="#55657F"/><rect x="4.5" y="10.5" width="7" height="4" fill="#fff" stroke="#6B6B6B"/></svg>` |
| `icon-run` | 16×16 | Сформировать / Выполнить | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><path d="M4 2.5l9 5.5-9 5.5z" fill="#43A047" stroke="#2E7D32"/></svg>` |
| `icon-cfg-marker` | 16×16 | служебный маркер конфигуратора в командной панели (только режим designer) | `<svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg"><rect x="2.5" y="2.5" width="11" height="11" rx="3" fill="none" stroke="#333" stroke-dasharray="2 1.5"/></svg>` |

Стандартные действия командной панели → значок: `Standard:AddRow`→`icon-add`, `CopyRow`→`icon-copy`, `EditRow`→`icon-edit`, `DeleteRow`→`icon-delete`, `EndEdit`→`icon-save`, `MoveUp`/`MoveDown`→`icon-up`/`icon-down`, `SortAsc`/`SortDesc`→`icon-sort-az`/`icon-sort-za`, `SetFilter`→`icon-filter`, `FilterByValue`→`icon-filter-value`, `FilterHistory`→`icon-filter-history`, `ClearFilter`→`icon-filter-off`, `OutputList`→`icon-output-list`, `Standard:146`→`icon-refresh`, `Standard:147`→`icon-open`, `Standard:145`→`icon-copy`, `Standard:300`→`icon-help`, `OpenMovements`→`icon-movements`; «Сформировать», «Выполнить» (кнопка по умолчанию отчёта/обработки с картинкой) → `icon-run`; «Печать» → `icon-print`.

### 0.6 Правила для отсутствующих свойств

Общий принцип: **нет тега = значение по умолчанию платформы**, а оно = «как у нового элемента в Конфигураторе». Превью не подставляет ничего «на всякий случай» (например, кнопку «…» у поля без `ChoiceButton` — только если тип поля этого требует, см. §3). Цвет, заданный `kind="style"` без `value`, считается отсутствующим.

| Роль цвета | По умолчанию |
|---|---|
| TextColor / FieldTextColor / HeaderTextColor / FooterTextColor | `var(--text)` (чёрный) |
| BackColor надписи | designer: `var(--form-bg)`; runtime: прозрачный |
| BackColor прочих элементов | не задаётся (фон класса) |
| FieldBackColor | `#FFFFFF` (у `.ro`/`.dis` — `var(--form-bg)`) |
| BorderColor | `var(--border)` |
| ButtonBackColor (поле) | градиент `--tb-from`→`--tb-to`; (кнопка) градиент `--btn-*` |
| HeaderBackColor / FooterBackColor | градиент `--head-from`→`--head-to` |
| SelectionBackColor / SelectionTextColor | `var(--sel-bg)` / `var(--sel-text)` |
| TextColor заголовка группы | `var(--group-title)` |
| HolidayColor | `var(--holiday)` |

Исключения: у UsualGroup без `Font` заголовок жирный; у RadioButton без данных выбран первый переключатель группы (§7); у InputField без `ChoiceButton` кнопка выбора есть, если тип ссылочный, дата или число (§3).

### 0.7 Общие состояния

В 1С у элементов бывают hover (подсветка кнопок командной панели), focus (курсор в поле), pressed, disabled, read-only, selected, checked. Превью статично, мыши и фокуса нет, поэтому показываем только состояния, которые следуют из Form.xml или из образца данных:

| Состояние | Показываем? | Как |
|---|---|---|
| disabled (`Enabled=false`) | да | класс `.dis` на ROOT: текст `var(--text-dis)`, значки `grayscale` + `opacity`; поле — фон формы |
| read-only (`ReadOnly=true`) | да | класс `.ro` (поле/поле выбора); таблица — пустые ячейки-образцы |
| selected | да, как образец | первая строка таблицы-списка (если таблица — основной список формы) и дерево: `.tr.sel`; календарь — дата 22; список значений — первая строка |
| checked | да, по данным/образцам | флажок `.box.on` — только в образцах и при значении `true`; переключатель — первый в группе `.rad.on`; кнопка командной панели `checked=true` — `.cbtn.on` |
| pressed | только `checked` | `.cbtn.on` |
| hover / focus | нет | классы `.cbtn.hov` описаны на будущее, превью их не ставит |
| hidden (`Visible=false`) | только в режиме «показывать скрытые» | класс `.hid` (`opacity:.35; outline:1px dashed #888`) |

### 0.8 Языки и длинные подписи

- Подпись берётся строго на выбранном языке (`ru`/`ro`); если её нет — пусто (у надписи в designer — `<<Имя>>`, у закладки — имя страницы, у формы — имя формы). Подстановки другого языка нет.
- Одна строка: `white-space:nowrap; overflow:hidden; text-overflow:ellipsis` на `.cap`, `.val`, `.colh`, `.gt`. Румынские подписи длиннее — обрезка многоточием, размеры элемента не меняются.
- Перенос по словам только там, где он есть в 1С: надпись высотой ≥ 28 px или `VerticalAlign=Top` (`.ml`), `MultiLine` у поля и кнопки, `HeaderHeight` > 1.
- Закладки не сжимаются (полные подписи), лишние обрезаются краем полосы; справа — стрелки `.pscroll`.
- Кнопка: минимальная ширина = ширина элемента из Form.xml; подпись обрезается многоточием; у кнопки уже 24 px подпись не выводится.
- Диакритика `ă â î ș ț` должна быть в шрифте: Tahoma/Arial её содержат.

## §1. Общие структуры

Правила перевода общих структур в CSS; классы-носители перечислены в «Свойства → правила» каждого класса. Порядок применения: шрифт класса по умолчанию → `<Font>` элемента; цвет роли по умолчанию (§0.6) → `<…Color>` элемента.

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Font.face` | `face="Verdana"` (любое имя) | inline `font-family:"<face>",Tahoma,Arial,sans-serif` на элементе-носителе текста (см. Font в классе) |
| `Font.face` | нет атрибута | не задавать: наследуется `var(--font)` (Tahoma 8 pt) |
| `Font.size` | `size="N"` (пункты) | inline `font-size:Npt` на носителе текста |
| `Font.size` | нет атрибута | не задавать (8 pt из `--font`) |
| `Font.bold` | `bold="true"` | класс `.b` (`font-weight:bold`) на носителе текста |
| `Font.bold` | `bold="false"` | класс `.b` снять, даже если класс задаёт жирный по умолчанию (заголовок группы, «OK»): inline `font-weight:normal` |
| `Font.bold` | нет атрибута | начертание класса по умолчанию |
| `Font.italic` | `italic="true"` | класс `.f-i` (`font-style:italic`) |
| `Font.italic` | нет / `false` | прямой |
| `Font.underline` | `underline="true"` | класс `.f-u` (`text-decoration:underline`) |
| `Font.underline` | нет / `false` | без подчёркивания (кроме гиперссылки) |
| `Font.strikeout` | `strikeout="true"` | класс `.f-s` (`text-decoration:line-through`); вместе с `.f-u` — `underline line-through` |
| `Font.strikeout` | нет / `false` | без зачёркивания |
| `Font.scale` | `scale="N"` (%) | inline `font-size:calc(8pt * N / 100)`; если задан и `size` — `calc(<size>pt * N / 100)` |
| `Font.scale` | нет / `100` | без изменений |
| `Font.source` | `source="bin"` / `"style"` / нет | не отображать: служебный признак происхождения шрифта; на вид не влияет (видно только в инспекторе) |
| `Color.kind` | `kind="rgb"` | цвет = атрибут `value` |
| `Color.kind` | `kind="style"` и есть `value` | цвет = `value` (запасное значение стилевого цвета) |
| `Color.kind` | `kind="style"` без `value` | цвет не определён → значение роли по умолчанию (§0.6, таблица «роль → по умолчанию»); `ref` показать в инспекторе |
| `Color.kind` | `kind="auto"` / неизвестный | как отсутствие тега |
| `Color.value` | `value="#RRGGBB"` | использовать как есть в CSS (`color` / `background` / `border-color` — в зависимости от тега-носителя) |
| `Color.value` | нет атрибута | см. Color.kind |
| `Color.ref` | `ref="<GUID>"` без value | не отображать: таблицы «GUID стиля → hex» в превью нет; применяется значение роли по умолчанию (§0.6), GUID — только в инспекторе |
| `Color.ref` | `ref` вместе с `value` | использовать `value` |
| `Border.type` | `type="None"` | класс `.bd-None` (рамки нет) |
| `Border.type` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame` (1 px, цвет BorderColor или `var(--border)`); у UsualGroup — `.bs-SingleLine` / `.bs-Frame` (§12) |
| `Border.type` | `Single3D`, `Double`, `Rounded`, `Overline`, `Underline`, `Embossed`, `Indented` | класс `.bd-<type>` (theme.css, раздел 1) |
| `Border.type` | нет тега | рамка класса по умолчанию |
| `Border.code` | `code="N"` | не отображать: дублирует `type` (0 = None, 4 = SingleLine, 6 = Frame); используется только если `type` отсутствует: 0 → `.bd-None`, 4 → `.bd-SingleLine`, 6 → `.bd-Frame` |
| `Picture.kind` | `kind="1"` + `refGuid` | картинка из конфигурации — заглушка `icon-picture` (§0.5) в месте картинки, размер 16×16 или область картинки (PictureDecoration) |
| `Picture.kind` | `true` (`<Picture>true</Picture>`) | то же; у надписи автора (имя `НадписьАвтор`) — `icon-user` |
| `Picture.kind` | нет тега | картинки нет |
| `Picture.refGuid` | `refGuid="<GUID>"` | не отображать: изображение по GUID недоступно (пикселей в Form.xml нет); GUID показать в инспекторе, на холсте — заглушка по Picture.kind |
| `Shortcut.key` | `<Shortcut key="…" mods="…"/>` | не отображать: в обычной форме 1С сочетание клавиш на кнопке не подписывается (видно только в пунктах меню); превью добавляет его в `title=` как «(Ctrl+Enter)» |
| `Shortcut.mods` | `mods="N"` | не отображать: см. Shortcut.key |

## §2. Форма (корень Form.xml) — `Form`

### Структура HTML

```html
<!-- вне холста (скрыт при ?chrome=0) -->
<div class="form-title"><span class="pic16"><!-- icon-picture, только если есть CaptionPicture --></span>Организации: Создание</div>
<!-- холст: размер = Width × Height; переменные темы на #canvas -->
<div id="canvas" class="theme-v82" style="width:540px;height:434px"> … элементы .c … </div>
```

### CSS

Правила из `theme.css` (раздел 0a), вставлены без изменений:

```css
/* ---------- 0a. Заголовок окна (вне холста; при chrome=0 скрыт) ---------- */
.form-title{height:22px;line-height:22px;padding:0 6px;font:bold 8pt Tahoma,Arial,sans-serif;color:#000;background:linear-gradient(180deg,#F7F6F0,#E7E4D8);border-bottom:1px solid #B2B2B2;display:flex;align-items:center;gap:4px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.form-title .pic16{display:inline-flex;width:16px;height:16px}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Form.Width` | `<Width>N</Width>` | `#canvas{width:Npx}`; `.formwrap` = N+2 (без изменений) |
| `Form.Height` | `<Height>N</Height>` | `#canvas{height:Npx}` (без изменений) |
| `Form.Caption` | `<Caption>` с языком `lang` | текст в `.form-title` (вне холста; при `chrome=0` не выводится). Заголовок окна: высота 22 px, фон `linear-gradient(#F7F6F0,#E7E4D8)`, жирный 8 pt, слева 16×16 `icon-picture`, если задан `CaptionPicture` |
| `Form.Caption` | нет подписи на языке `lang` | имя формы (`Form@name`) |
| `Form.ShowTabs` | `code≠0` (`Top`, `TopMultiLine`, `TopScrolling`…) | закладки страниц верхнего уровня рисуются как у Panel (§10, Panel.ShowTabs) |
| `Form.ShowTabs` | `None` / `DontUse` / нет тега | закладок нет, видна первая видимая страница |
| `Form.CaptionPicture` | `<CaptionPicture …/>` | в `.form-title` перед текстом `<span class="pic16">` + `icon-picture` |
| `Form.CaptionPicture` | нет тега | без значка |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `#canvas` | фон `var(--form-bg)`, шрифт `var(--font)` | `data-xml="Form.Caption"`, `data-state="normal"` |

### Особые случаи

- Язык подписи окна: если подписи на выбранном языке нет — имя формы (`Form@name`), без подстановки другого языка.
- Холст не масштабируется; `.formwrap` = Width+2 (рамка 1 px), тень `.formwrap` остаётся частью оболочки просмотра, а не формы.

## §3. Поле ввода — `InputField`

### Структура HTML

```html
<div class="c t-InputField [ro] [dis] [hid] [bd-<type>]" style="left;top;width;height[;background;border-color]" title="<Tooltip|имя>">
  [<span class="pic16"><!-- icon-picture --></span>]          <!-- Picture -->
  <span class="val [num] [ml] [pw] [ha-*] [va-*] [b] [f-i] [f-u] [f-s]" [style="color;font-family;font-size"]>
    <!-- образец значения (designer) -->
    [<i class="mark"></i>]                                     <!-- AutoMarkIncomplete -->
  </span>
  [<span class="tb dd"><!-- icon-drop --></span>]             <!-- DropListButton -->
  [<span class="tb"><!-- icon-choice | icon-calendar | icon-calc | icon-type --></span>]  <!-- ChoiceButton -->
  [<span class="tb"><!-- icon-clear --></span>]               <!-- ClearButton -->
  [<span class="tb"><!-- icon-open --></span>]                <!-- OpenButton -->
  [<span class="tb spin"><i>▲</i><i>▼</i></span>]             <!-- SpinButton -->
  [<span class="vsb"><span class="sbb">…</span><span class="sbb">…</span></span>]  <!-- VerticalScrollBar -->
</div>
<!-- у каждой .tb могут быть доп. классы: tr-bg (TransparentBack), colored (ButtonBackColor через --tb-bg) -->
```

### CSS

Правила из `theme.css` (раздел 3), вставлены без изменений:

```css
/* ---------- 3. Поле ввода — InputField (и общее для ChoiceField) ---------- */
.t-InputField,.t-ChoiceField{background:#FFFFFF;border:1px solid var(--border);border-radius:var(--r-field);display:flex;align-items:stretch}
.t-InputField .val,.t-ChoiceField .val{flex:1;min-width:0;position:relative;overflow:hidden;padding:0 2px;display:flex;align-items:center;white-space:pre;color:var(--text)}
.val.num{justify-content:flex-end;padding-right:3px}
.val.ph{color:#8C8775;font-style:normal}
.val.ml{align-items:flex-start;padding-top:1px;white-space:pre-wrap}
.val.pw{letter-spacing:1px}
.val .mark{position:absolute;left:2px;right:2px;bottom:1px;height:1px;border:0;
  background:repeating-linear-gradient(90deg,var(--mark) 0 1px,transparent 1px 2px)}
/* кнопки внутри поля: «…», календарь, калькулятор, «T», «×», лупа, ▾, ▲▼ */
.tb{flex:none;width:var(--tb-w);background:linear-gradient(180deg,var(--tb-from),var(--tb-to));border:0;border-left:1px solid var(--tb-sep);display:flex;align-items:center;justify-content:center;color:var(--tb-ink);font-size:9px;line-height:1;overflow:hidden}
.tb{font-weight:bold;font-size:11px;letter-spacing:-.5px} /* «...» текстом, пока JS не перейдёт на icon-choice */
.tb.bold{font:bold 11px Tahoma,Arial,sans-serif;color:#3D3D3D}
.tb:last-child{border-top-right-radius:1px;border-bottom-right-radius:1px}
.tb.spin{flex-direction:column;justify-content:stretch}
.tb.spin i{flex:1;display:flex;align-items:center;justify-content:center;font-style:normal;font-size:6px}
.tb.spin i+i{border-top:1px solid var(--tb-sep)}
.tb.colored{background:var(--tb-bg,#F1EEDD)}
/* только просмотр: белая полоса 2 px внутри рамки, середина цвета формы, кнопки бледнее */
.t-InputField.ro,.t-ChoiceField.ro{background:var(--form-bg);box-shadow:inset 0 0 0 2px #FFFFFF}
.t-InputField.ro .tb,.t-ChoiceField.ro .tb{opacity:.55}
/* недоступно: как «только просмотр», текст и значки бледные */
.t-InputField.dis,.t-ChoiceField.dis{background:var(--form-bg);box-shadow:none}
.t-InputField.dis .tb,.t-ChoiceField.dis .tb{opacity:.45}
/* картинка в поле (Picture) — слева от текста */
.t-InputField .pic16,.t-ChoiceField .pic16{margin:0 0 0 1px;align-self:center}
/* многострочное поле с полосой прокрутки (MultiLine + VerticalScrollBar) */
.t-InputField .vsb{flex:none;width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between}

/* общие модификаторы (раздел 1 theme.css), используются этим классом */
/* ---------- 1. Общие модификаторы (§0.6, §0.7, §1) ---------- */
#canvas .c,.canvas .c{font:var(--font)}
.c .cap{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}
/* Font.* (JS: классы на .cap/.val/.colh) */
.b{font-weight:bold}
.f-i{font-style:italic}
.f-u{text-decoration:underline}
.f-s{text-decoration:line-through}
.f-u.f-s{text-decoration:underline line-through}
/* недоступный элемент (Enabled=false) (JS: класс .dis на корне .c) */
.c.dis,.c.dis .cap,.c.dis .val{color:var(--text-dis)!important}
.c.dis svg{opacity:.45;filter:grayscale(1)}
/* скрытый элемент в режиме «показывать скрытые» — без изменений */
#canvas .c.hid,.canvas .c.hid{opacity:.35;outline:1px dashed #888}
/* выравнивание текста (HorizontalAlign / VerticalAlign) (JS) */
.ha-Left{justify-content:flex-start!important;text-align:left}
.ha-Center{justify-content:center!important;text-align:center}
.ha-Right{justify-content:flex-end!important;text-align:right}
.ha-Justify{justify-content:flex-start!important;text-align:justify}
.va-Top{align-items:flex-start!important}
.va-Center{align-items:center!important}
.va-Bottom{align-items:flex-end!important}
/* рамка по Border.type (JS: класс на корне .c). Цвет — inline border-color из BorderColor, иначе --border */
.bd-None{border-style:none!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented{border-width:1px!important;border-style:solid!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented,.bd-Double,.bd-Overline,.bd-Underline{border-color:var(--border)}
.bd-Single3D{box-shadow:inset 1px 1px 0 #fff}
.bd-Double{border-width:3px!important;border-style:double!important}
.bd-Rounded{border-radius:4px!important}
.bd-Overline{border-style:none!important;border-top-style:solid!important;border-top-width:1px!important}
.bd-Underline{border-style:none!important;border-bottom-style:solid!important;border-bottom-width:1px!important}
.bd-Embossed{box-shadow:inset -1px -1px 0 #fff,inset 1px 1px 0 #D9D3B6}
.bd-Indented{box-shadow:inset 1px 1px 0 #D9D3B6,inset -1px -1px 0 #fff}
/* прозрачный фон (TransparentBack=true) */
.tr-bg{background:transparent!important}
/* картинка-заглушка 16×16 (Picture.refGuid задан, пикселей нет) */
.pic16{display:inline-flex;flex:none;width:16px;height:16px;align-items:center;justify-content:center}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `InputField.ChoiceButton` | `true`, ValueType составной (`composite="true"`) | `<span class="tb">` + `icon-type` («T») |
| `InputField.ChoiceButton` | `true`, тип Дата/ДатаВремя | `<span class="tb">` + `icon-calendar` |
| `InputField.ChoiceButton` | `true`, тип Число | `<span class="tb">` + `icon-calc` |
| `InputField.ChoiceButton` | `true`, прочие типы (ссылка, строка) | `<span class="tb">` + `icon-choice` («…» — три квадрата 2×2 у нижнего края) |
| `InputField.ChoiceButton` | `false` | кнопки выбора нет |
| `InputField.ChoiceButton` | нет тега | как `true` с типом из ValueType (поведение по умолчанию у ссылочных полей); если ValueType = Строка/Булево без длины — кнопки нет |
| `InputField.ClearButton` | `<ClearButton>true</ClearButton>` | `<span class="tb">` + `icon-clear` после кнопки выбора — **независимо от наличия ChoiceButton** |
| `InputField.ClearButton` | `false` / нет тега | нет |
| `InputField.OpenButton` | `<OpenButton>true</OpenButton>` | `<span class="tb">` + `icon-open` последней кнопкой — независимо от ChoiceButton |
| `InputField.OpenButton` | `false` / нет тега | нет |
| `InputField.DropListButton` | `<DropListButton>true</DropListButton>` | добавить `<span class="tb dd">` + `icon-drop` перед кнопкой выбора |
| `InputField.DropListButton` | `false` / нет тега | кнопки ▾ нет |
| `InputField.SpinButton` | `<SpinButton>true</SpinButton>` | добавить последней кнопкой `<span class="tb spin"><i>▲</i><i>▼</i></span>` (или `icon-spin`) |
| `InputField.SpinButton` | `false` / нет тега | нет |
| `InputField.AutoMarkIncomplete` | `true` | внутри `.val`: `<i class="mark"></i>` — точки 1 px через 1 px цвета `var(--mark)` (#FF0000) на 1 px выше нижней границы, отступ 2 px слева и справа (как «Фискальный код» в 02) |
| `InputField.AutoMarkIncomplete` | `false` / нет тега | нет |
| `InputField.ReadOnly` | `<ReadOnly>true</ReadOnly>` | класс `.ro` на ROOT: `background:var(--form-bg); box-shadow:inset 0 0 0 2px #FFFFFF` (белая полоса 2 px внутри рамки, как «Пробег общий» в 06); кнопки `.tb` — `opacity:.55`; рамка остаётся |
| `InputField.ReadOnly` | нет тега / `false` | белое поле |
| `InputField.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: фон `var(--form-bg)`, текст `var(--text-dis)`, кнопки `.tb` `opacity:.45`; рамка остаётся `var(--border)` |
| `InputField.Enabled` | нет тега / `true` | без изменений |
| `InputField.MultiLine` | `<MultiLine>true</MultiLine>` | класс `.ml` на `.val`: `align-items:flex-start; padding-top:1px; white-space:pre-wrap` (текст с верхней строки); кнопки `.tb` остаются по всей высоте |
| `InputField.MultiLine` | нет тега / `false` | одна строка по центру высоты |
| `InputField.PasswordMode` | `<PasswordMode>true</PasswordMode>` | в режиме designer образец значения `*****` (класс `.pw` на `.val`); в runtime пусто |
| `InputField.PasswordMode` | нет / `false` | обычный образец |
| `InputField.MarkNegatives` | `true` | не отображать: превью показывает образец `0`/`0,00`, отрицательных значений нет |
| `InputField.HorizontalAlign` | `Left` / `Center` / `Right` / `Justify` | класс `.ha-Left` / `.ha-Center` / `.ha-Right` / `.ha-Justify` на `.val` |
| `InputField.HorizontalAlign` | `Auto` или нет тега | число (ValueType=Number) — `.num` (вправо), остальное влево |
| `InputField.VerticalAlign` | `Top` / `Center` / `Bottom` | класс `.va-Top` / `.va-Center` / `.va-Bottom` на `.val` (вместо `align-items:center`) |
| `InputField.VerticalAlign` | нет тега | по центру высоты (как сейчас); при MultiLine — сверху |
| `InputField.VerticalScrollBar` | `UseAlways` / `true` | рисовать полосу вертикальную (`<span class="vsb">` справа внутри ROOT, после кнопок: стрелки `icon-arrow-up`/`icon-arrow-down`), даже если содержимого мало |
| `InputField.VerticalScrollBar` | `DontUse` / `false` | полосу вертикальную не рисовать |
| `InputField.VerticalScrollBar` | `AutoUse` / `Auto` / нет тега | по правилу класса (см. «Особые случаи») |
| `InputField.HorizontalScrollBar` | любое значение | не отображать: горизонтальной полосы у поля ввода в 1С 8.x нет (свойство применимо к многострочному тексту без переноса); в корпусе не встречается |
| `InputField.ChoiceListMode` | любое значение | не отображать: режим ввода (только выбор из списка) не меняет вид поля |
| `InputField.Format` | `ДФ=`/`ДЛФ=` только с датой (`dd.MM.yyyy`, `ДЛФ=D`) | образец значения (designer) — маска даты без времени `" . ."` |
| `InputField.Format` | `ДФ=`/`ДЛФ=` со временем (`HH:mm`, `ДЛФ=DT`, `ДЛФ=T`) | маска `" . .   : :"` (`T` — `"  : :"`) |
| `InputField.Format` | `ЧДЦ=N` (число) | образец `0` с N знаками после запятой (`0,00` при N=2); `ЧДЦ=0` → `0` |
| `InputField.Format` | прочие / нет тега | образец по ValueType |
| `InputField.Mask` | `<Mask>…</Mask>` | в режиме designer образец значения = маска, где символы-заполнители `9 # N U X ! @ h` заменены пробелом, а прочие (литералы: `.`, `-`, `/`, `(`, `)`, `:`) остаются: `@@@.@@@` → `"   .   "`; в runtime пусто |
| `InputField.Mask` | нет тега | образец по ValueType/Format |
| `InputField.ValueType` | `kind="Number"` | designer: образец `0` (`fd="0"`) или `0,00` (`fd>0`, столько нулей, сколько `fd`), класс `.num` (вправо) |
| `InputField.ValueType` | `kind="Date"` / `"DateTime"` / `"Time"` | designer: маска `" . ."` / `" . .   : :"` / `"  : :"` |
| `InputField.ValueType` | `kind="String"`, `"Boolean"`, ссылки | пусто |
| `InputField.ValueType` | `composite="true"` | пусто; кнопка выбора — `icon-type` |
| `InputField.ValueType` | runtime-режим | всегда пусто (класс `.ph` не использовать: путь к данным только в инспекторе и `title=`) |
| `InputField.Font` | `<Font … bold="true"/>` | класс `.b` на `.val` (значение поля) |
| `InputField.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.val` (значение поля) (подчёркнутый и зачёркнутый вместе — оба класса) |
| `InputField.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.val` (значение поля) по §1 (Font.face, Font.size, Font.scale) |
| `InputField.Font` | нет тега | шрифт формы |
| `InputField.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.val` (если задан FieldTextColor — он важнее) |
| `InputField.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `InputField.TextColor` | `<TextColor kind="style"/>` без value / нет тега | чёрный |
| `InputField.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT (если задан и FieldBackColor — FieldBackColor важнее) |
| `InputField.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `InputField.BackColor` | `<BackColor kind="style"/>` без value / нет тега | см. FieldBackColor |
| `InputField.FieldBackColor` | `<FieldBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT (перекрывает `.ro`) |
| `InputField.FieldBackColor` | `<FieldBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `InputField.FieldBackColor` | `<FieldBackColor kind="style"/>` без value / нет тега | белый `#FFFFFF` (у `.ro`/`.dis` — `var(--form-bg)`) |
| `InputField.FieldTextColor` | `<FieldTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.val` |
| `InputField.FieldTextColor` | `<FieldTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `InputField.FieldTextColor` | `<FieldTextColor kind="style"/>` без value / нет тега | чёрный `var(--text)` |
| `InputField.ButtonBackColor` | `<ButtonBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на всех `.tb` поля (класс `.colored`, переменная `--tb-bg`) |
| `InputField.ButtonBackColor` | `<ButtonBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `InputField.ButtonBackColor` | `<ButtonBackColor kind="style"/>` без value / нет тега | градиент `--tb-from`→`--tb-to` |
| `InputField.ButtonTextColor` | `<ButtonTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на всех `.tb` поля |
| `InputField.ButtonTextColor` | `<ButtonTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `InputField.ButtonTextColor` | `<ButtonTextColor kind="style"/>` без value / нет тега | `var(--tb-ink)` |
| `InputField.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `InputField.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `InputField.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `InputField.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `InputField.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `InputField.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `InputField.BorderStyle` | нет тега | рамка 1 px `var(--border)`, скругление `var(--r-field)` |
| `InputField.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на кнопках `.tb` (фон области под кнопками прозрачный; рамка `--tb-sep` остаётся) (`background:transparent!important`) |
| `InputField.TransparentBack` | нет тега / `false` | градиент кнопок `--tb-from`→`--tb-to` |
| `InputField.Picture` | `<Picture kind="1" refGuid="…"/>` (картинка из конфигурации) | вставить `<span class="pic16">` + `icon-picture` (§0.5) первым ребёнком ROOT, перед `.val` (`margin-left:1px`) — пикселей картинки в Form.xml нет |
| `InputField.Picture` | `<Picture>true</Picture>` без refGuid | то же, значок `icon-picture` |
| `InputField.Picture` | нет тега | картинки нет |
| `InputField.ChoiceButtonPicture` | `<ChoiceButtonPicture …/>` | в кнопке выбора вместо значка типа — `icon-picture` 12×12 (уменьшить `width/height` SVG до 12) |
| `InputField.ChoiceButtonPicture` | нет тега | значок по типу (см. ChoiceButton) |
| `InputField.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `InputField.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `InputField.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-InputField` | белое поле, рамка `var(--border)` | `data-xml="InputField.ChoiceButton"`, `data-state="normal"` |
| readonly | `.t-InputField.ro` | фон `var(--form-bg)` + белая полоса 2 px внутри; `.tb` `opacity:.55` | `data-xml="InputField.ReadOnly"`, `data-state="readonly"` |
| disabled | `.t-InputField.dis` | фон `var(--form-bg)`, текст `var(--text-dis)`, `.tb` `opacity:.45` | `data-xml="InputField.Enabled"`, `data-state="disabled"` |
| focus | — | не показываем: в 1С рамка активного поля не меняется заметно; в статике фокуса нет | — |
| hover | — | не показываем (нет мыши) | — |

### Особые случаи

- Поле ниже 19 px (бывает 15–17): кнопки `.tb` сохраняют ширину `--tb-w`, значки по центру, без обрезки по ширине; если ширина поля < (кнопки + 12 px) — кнопки не выводить, только рамка.
- Комбинации: `ReadOnly`+`AutoMarkIncomplete` — отметка рисуется (в 1С она есть и у недоступных для ввода полей); `Enabled=false` важнее `ReadOnly` (класс `.dis`, без `.ro`).
- Порядок кнопок справа налево фиксирован: SpinButton, OpenButton, ClearButton, ChoiceButton, DropListButton (как в 1С).
- Режим runtime: значение всегда пустое; путь к данным и имя — только в `title=` и инспекторе (класс `.ph` с курсивным путём больше не используется).
- `Visible=false` — см. §0.7; скрытое поле не участвует в выравнивании соседей (координаты абсолютные).
- `VerticalScrollBar` = `AutoUse`/`Auto`/нет тега: полосы нет (содержимого в превью нет); полоса появляется только при `UseAlways`.

## §4. Надпись — `LabelDecoration`

### Структура HTML

```html
<div class="c t-LabelDecoration [link] [dis] [hid] [ha-*] [va-*] [ml] [pl-Right|pl-Top|pl-Bottom|pl-Center] [tr-bg] [bd-<type>]"
     style="left;top;width;height[;background][;border-color]" title="<Tooltip|имя>">
  [<span class="pic16"><!-- icon-picture | icon-user --></span>]   <!-- Picture -->
  <span class="cap [b] [f-i] [f-u] [f-s]" data-k="…" [style="color;font-family;font-size"]>Текст | &lt;&lt;Имя&gt;&gt;</span>
</div>
```

### CSS

Правила из `theme.css` (раздел 2), вставлены без изменений:

```css
/* ---------- 2. Надпись — LabelDecoration ---------- */
.t-LabelDecoration{display:flex;align-items:center;gap:3px}
.t-LabelDecoration .cap{padding:0 1px}
.t-LabelDecoration.va-Top{padding-top:2px}
.t-LabelDecoration.link .cap{color:var(--link);text-decoration:underline}
.t-LabelDecoration .ic,.t-LabelDecoration .pic16{width:16px;height:16px;margin:0}
.t-LabelDecoration.pl-Right{flex-direction:row-reverse;justify-content:flex-end}
.t-LabelDecoration.pl-Top{flex-direction:column;justify-content:center}
.t-LabelDecoration.pl-Bottom{flex-direction:column-reverse;justify-content:center}
.t-LabelDecoration.pl-Center .cap{position:absolute;left:0;right:0;text-align:center}
.t-LabelDecoration.ml .cap{white-space:normal;overflow:hidden}
.t-LabelDecoration .cap.neg{color:var(--neg)}

/* общие модификаторы (раздел 1 theme.css), используются этим классом */
/* ---------- 1. Общие модификаторы (§0.6, §0.7, §1) ---------- */
#canvas .c,.canvas .c{font:var(--font)}
.c .cap{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}
/* Font.* (JS: классы на .cap/.val/.colh) */
.b{font-weight:bold}
.f-i{font-style:italic}
.f-u{text-decoration:underline}
.f-s{text-decoration:line-through}
.f-u.f-s{text-decoration:underline line-through}
/* недоступный элемент (Enabled=false) (JS: класс .dis на корне .c) */
.c.dis,.c.dis .cap,.c.dis .val{color:var(--text-dis)!important}
.c.dis svg{opacity:.45;filter:grayscale(1)}
/* скрытый элемент в режиме «показывать скрытые» — без изменений */
#canvas .c.hid,.canvas .c.hid{opacity:.35;outline:1px dashed #888}
/* выравнивание текста (HorizontalAlign / VerticalAlign) (JS) */
.ha-Left{justify-content:flex-start!important;text-align:left}
.ha-Center{justify-content:center!important;text-align:center}
.ha-Right{justify-content:flex-end!important;text-align:right}
.ha-Justify{justify-content:flex-start!important;text-align:justify}
.va-Top{align-items:flex-start!important}
.va-Center{align-items:center!important}
.va-Bottom{align-items:flex-end!important}
/* рамка по Border.type (JS: класс на корне .c). Цвет — inline border-color из BorderColor, иначе --border */
.bd-None{border-style:none!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented{border-width:1px!important;border-style:solid!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented,.bd-Double,.bd-Overline,.bd-Underline{border-color:var(--border)}
.bd-Single3D{box-shadow:inset 1px 1px 0 #fff}
.bd-Double{border-width:3px!important;border-style:double!important}
.bd-Rounded{border-radius:4px!important}
.bd-Overline{border-style:none!important;border-top-style:solid!important;border-top-width:1px!important}
.bd-Underline{border-style:none!important;border-bottom-style:solid!important;border-bottom-width:1px!important}
.bd-Embossed{box-shadow:inset -1px -1px 0 #fff,inset 1px 1px 0 #D9D3B6}
.bd-Indented{box-shadow:inset 1px 1px 0 #D9D3B6,inset -1px -1px 0 #fff}
/* прозрачный фон (TransparentBack=true) */
.tr-bg{background:transparent!important}
/* картинка-заглушка 16×16 (Picture.refGuid задан, пикселей нет) */
.pic16{display:inline-flex;flex:none;width:16px;height:16px;align-items:center;justify-content:center}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `LabelDecoration.Caption` | `<Caption>` на языке `lang` | текст в `.cap`, `text-overflow:ellipsis` при нехватке ширины |
| `LabelDecoration.Caption` | пусто / нет на языке `lang` | designer: `<<ИмяЭлемента>>`; runtime: пусто |
| `LabelDecoration.HorizontalAlign` | `Left` / `Center` / `Right` / `Justify` | класс `.ha-Left` / `.ha-Center` / `.ha-Right` / `.ha-Justify` на ROOT |
| `LabelDecoration.HorizontalAlign` | `Auto` или нет тега | слева |
| `LabelDecoration.VerticalAlign` | `Top` / `Center` / `Bottom` | класс `.va-Top` / `.va-Center` / `.va-Bottom` на ROOT; при `Top` текст многострочный (`.ml`), отступ сверху 2 px |
| `LabelDecoration.VerticalAlign` | нет тега | по центру высоты; если высота ≥ 2 строк (h ≥ 28 px) — класс `.ml`, перенос по словам |
| `LabelDecoration.Hyperlink` | `true` | класс `.link`: `.cap{color:var(--link);text-decoration:underline}`; TextColor, если задан, важнее цвета ссылки |
| `LabelDecoration.Hyperlink` | `false` / нет | обычный текст |
| `LabelDecoration.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.cap` |
| `LabelDecoration.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `LabelDecoration.TextColor` | `<TextColor kind="style"/>` без value / нет тега | чёрный; у гиперссылки `var(--link)` |
| `LabelDecoration.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `LabelDecoration.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `LabelDecoration.BackColor` | `<BackColor kind="style"/>` без value / нет тега | designer: `var(--form-bg)`; runtime: прозрачный |
| `LabelDecoration.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `LabelDecoration.TransparentBack` | нет тега / `false` | designer: фон `var(--form-bg)` (надпись перекрывает то, что под ней); runtime: прозрачный |
| `LabelDecoration.Font` | `<Font … bold="true"/>` | класс `.b` на `.cap` |
| `LabelDecoration.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.cap` (подчёркнутый и зачёркнутый вместе — оба класса) |
| `LabelDecoration.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.cap` по §1 (Font.face, Font.size, Font.scale) |
| `LabelDecoration.Font` | нет тега | шрифт формы `var(--font)` |
| `LabelDecoration.Picture` | `<Picture kind="1" refGuid="…"/>` (картинка из конфигурации) | вставить `<span class="pic16">` + `icon-picture` (§0.5) перед `.cap` (зазор 3 px); у надписи с именем `НадписьАвтор` — `icon-user` — пикселей картинки в Form.xml нет |
| `LabelDecoration.Picture` | `<Picture>true</Picture>` без refGuid | то же, значок `icon-picture` |
| `LabelDecoration.Picture` | нет тега | картинки нет |
| `LabelDecoration.PictureSize` | `AutoSize` / `RealSize` | заглушка 16×16 в своём размере (`.pic16`), без растяжения |
| `LabelDecoration.PictureSize` | `Proportionally` / `Stretch` | заглушка 16×16 по центру области картинки надписи (настоящие пиксели неизвестны, масштабировать нечего) |
| `LabelDecoration.PictureSize` | `Tile` | заглушка 16×16 в левом верхнем углу |
| `LabelDecoration.PictureSize` | нет тега | как `AutoSize` |
| `LabelDecoration.LabelPictureLocation` | `Left` / нет тега | картинка слева от текста, зазор 3 px |
| `LabelDecoration.LabelPictureLocation` | `Right` | класс `.pl-Right` (`flex-direction:row-reverse`) |
| `LabelDecoration.LabelPictureLocation` | `Top` / `Bottom` | класс `.pl-Top` / `.pl-Bottom` (колонкой, по центру) |
| `LabelDecoration.LabelPictureLocation` | `Center` | класс `.pl-Center`: картинка по центру, текст поверх |
| `LabelDecoration.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `LabelDecoration.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `LabelDecoration.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `LabelDecoration.BorderStyle` | нет тега | рамки нет |
| `LabelDecoration.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT (вместе с BorderStyle) |
| `LabelDecoration.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `LabelDecoration.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `LabelDecoration.Format` | `<Format>` | не отображать: форматирует значение из данных; у надписи с текстом подписи на вид не влияет |
| `LabelDecoration.MarkNegatives` | любое значение | не отображать: текст надписи — подпись, не число из данных |
| `LabelDecoration.ScrollingText` | `Fast` / `Slow` / `Normal`… | не отображать: бегущая строка — анимация; превью статично, текст стоит на месте |
| `LabelDecoration.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: текст `var(--text-dis)`; у гиперссылки — тоже серый, без подчёркивания |
| `LabelDecoration.Enabled` | нет тега / `true` | без изменений |
| `LabelDecoration.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `LabelDecoration.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `LabelDecoration.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-LabelDecoration` | текст на фоне формы | `data-xml="LabelDecoration.Caption"`, `data-state="normal"` |
| disabled | `.t-LabelDecoration.dis` | текст `var(--text-dis)` | `data-xml="LabelDecoration.Enabled"`, `data-state="disabled"` |
| hover (ссылка) | — | не показываем | — |

### Особые случаи

- Пустая подпись в designer: `<<Имя>>` тем же шрифтом и цветом; в runtime — ничего.
- Длинная подпись: одна строка, `ellipsis`; если высота ≥ 28 px или `VerticalAlign=Top` — перенос по словам (`.ml`), обрезка по высоте.
- Надпись поверх другого элемента (ZOrder): фон `var(--form-bg)` в designer закрывает нижний элемент — так в Конфигураторе; при `TransparentBack=true` — не закрывает.

## §5. Кнопка (самостоятельная) — `Button`

### Структура HTML

```html
<div class="c t-Button [def] [dis] [hid] [ml] [pl-Right] [tr-bg] [colored] [ha-*] [va-*]" style="left;top;width;height[;--btn-bg][;border-color]">
  [<span class="pic16"><!-- icon-picture --></span>]
  <span class="cap [b] [f-*]" [style="color"]>Изменить...</span>
  [<span class="mn"><!-- icon-drop --></span>]     <!-- MenuMode=Use/UseExtra -->
</div>
```

### CSS

Правила из `theme.css` (раздел 4), вставлены без изменений:

```css
/* ---------- 4. Кнопка — Button ---------- */
.t-Button{background:linear-gradient(180deg,var(--btn-from) 0,var(--btn-mid) 45%,var(--btn-to) 100%);border:1px solid var(--border);border-radius:var(--r-btn);
  display:flex;align-items:center;justify-content:center;gap:3px;padding:0 3px;box-shadow:inset 0 -1px 0 rgba(255,255,255,.55)}
.t-Button .cap{white-space:nowrap}
.t-Button.ml .cap{white-space:normal;text-align:center;line-height:11px}
.t-Button.def{border-color:#8F8866;box-shadow:inset 0 0 0 1px #E0D9B9}
.t-Button.def .cap{font-weight:bold}
.t-Button.pl-Right{flex-direction:row-reverse}
.t-Button .mn{flex:none;margin-left:1px;border-left:1px solid var(--tb-sep);padding-left:2px;font-size:7px;align-self:stretch;display:flex;align-items:center}
.t-Button.dis{background:linear-gradient(180deg,#FCFBF6,#F2EFE2)}
.t-Button.tr-bg{border-color:transparent!important;box-shadow:none}
.t-Button.colored{background:var(--btn-bg)!important}

/* общие модификаторы (раздел 1 theme.css), используются этим классом */
/* ---------- 1. Общие модификаторы (§0.6, §0.7, §1) ---------- */
#canvas .c,.canvas .c{font:var(--font)}
.c .cap{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}
/* Font.* (JS: классы на .cap/.val/.colh) */
.b{font-weight:bold}
.f-i{font-style:italic}
.f-u{text-decoration:underline}
.f-s{text-decoration:line-through}
.f-u.f-s{text-decoration:underline line-through}
/* недоступный элемент (Enabled=false) (JS: класс .dis на корне .c) */
.c.dis,.c.dis .cap,.c.dis .val{color:var(--text-dis)!important}
.c.dis svg{opacity:.45;filter:grayscale(1)}
/* скрытый элемент в режиме «показывать скрытые» — без изменений */
#canvas .c.hid,.canvas .c.hid{opacity:.35;outline:1px dashed #888}
/* выравнивание текста (HorizontalAlign / VerticalAlign) (JS) */
.ha-Left{justify-content:flex-start!important;text-align:left}
.ha-Center{justify-content:center!important;text-align:center}
.ha-Right{justify-content:flex-end!important;text-align:right}
.ha-Justify{justify-content:flex-start!important;text-align:justify}
.va-Top{align-items:flex-start!important}
.va-Center{align-items:center!important}
.va-Bottom{align-items:flex-end!important}
/* рамка по Border.type (JS: класс на корне .c). Цвет — inline border-color из BorderColor, иначе --border */
.bd-None{border-style:none!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented{border-width:1px!important;border-style:solid!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented,.bd-Double,.bd-Overline,.bd-Underline{border-color:var(--border)}
.bd-Single3D{box-shadow:inset 1px 1px 0 #fff}
.bd-Double{border-width:3px!important;border-style:double!important}
.bd-Rounded{border-radius:4px!important}
.bd-Overline{border-style:none!important;border-top-style:solid!important;border-top-width:1px!important}
.bd-Underline{border-style:none!important;border-bottom-style:solid!important;border-bottom-width:1px!important}
.bd-Embossed{box-shadow:inset -1px -1px 0 #fff,inset 1px 1px 0 #D9D3B6}
.bd-Indented{box-shadow:inset 1px 1px 0 #D9D3B6,inset -1px -1px 0 #fff}
/* прозрачный фон (TransparentBack=true) */
.tr-bg{background:transparent!important}
/* картинка-заглушка 16×16 (Picture.refGuid задан, пикселей нет) */
.pic16{display:inline-flex;flex:none;width:16px;height:16px;align-items:center;justify-content:center}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Button.Caption` | `<Caption>` на языке `lang` | текст `.cap` по центру, `ellipsis` |
| `Button.Caption` | пусто | пустая кнопка (только картинка, если есть) |
| `Button.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `Button.Visible` | нет тега / `true` | выводить как обычно |
| `Button.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: текст `var(--text-dis)`, фон `linear-gradient(#FCFBF6,#F2EFE2)`, картинка серая |
| `Button.Enabled` | нет тега / `true` | без изменений |
| `Button.Picture` | `<Picture kind="1" refGuid="…"/>` (картинка из конфигурации) | вставить `<span class="pic16">` + `icon-picture` (§0.5) перед `.cap` (или вместо него, если подписи нет) — пикселей картинки в Form.xml нет |
| `Button.Picture` | `<Picture>true</Picture>` без refGuid | то же, значок `icon-picture` |
| `Button.Picture` | нет тега | картинки нет |
| `Button.PictureLocation` | `Right` | класс `.pl-Right` (картинка справа от подписи) |
| `Button.PictureLocation` | `Left` / нет тега | картинка слева |
| `Button.PictureSize` | `AutoSize` / `RealSize` | заглушка 16×16 в своём размере (`.pic16`), без растяжения |
| `Button.PictureSize` | `Proportionally` / `Stretch` | заглушка 16×16 по центру области картинки кнопки (настоящие пиксели неизвестны, масштабировать нечего) |
| `Button.PictureSize` | `Tile` | заглушка 16×16 в левом верхнем углу |
| `Button.PictureSize` | нет тега | как `AutoSize` |
| `Button.Font` | `<Font … bold="true"/>` | класс `.b` на `.cap` |
| `Button.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.cap` (подчёркнутый и зачёркнутый вместе — оба класса) |
| `Button.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.cap` по §1 (Font.face, Font.size, Font.scale) |
| `Button.Font` | нет тега | шрифт формы, обычный (у кнопки по умолчанию — жирный, см. DefaultButton) |
| `Button.ButtonBackColor` | `<ButtonBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT (вместо градиента: класс `.colored` + переменная `--btn-bg`) |
| `Button.ButtonBackColor` | `<ButtonBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Button.ButtonBackColor` | `<ButtonBackColor kind="style"/>` без value / нет тега | градиент кнопки |
| `Button.ButtonTextColor` | `<ButtonTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.cap` |
| `Button.ButtonTextColor` | `<ButtonTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Button.ButtonTextColor` | `<ButtonTextColor kind="style"/>` без value / нет тега | чёрный |
| `Button.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `Button.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Button.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `Button.HorizontalAlign` | `Left` / `Center` / `Right` / `Justify` | класс `.ha-Left` / `.ha-Center` / `.ha-Right` / `.ha-Justify` на ROOT |
| `Button.HorizontalAlign` | `Auto` или нет тега | по центру |
| `Button.VerticalAlign` | `Top` / `Center` / `Bottom` | класс `.va-Top` / `.va-Center` / `.va-Bottom` на ROOT |
| `Button.VerticalAlign` | нет тега | по центру |
| `Button.MultiLine` | `true` | класс `.ml`: `.cap{white-space:normal;text-align:center;line-height:11px}` |
| `Button.MultiLine` | `false` / нет | одна строка, `ellipsis` |
| `Button.DefaultButton` | `true` | класс `.def`: подпись жирная, рамка `#8F8866`, внутренняя светлая обводка 1 px `#E0D9B9` |
| `Button.DefaultButton` | `false` / нет | обычная кнопка |
| `Button.MenuMode` | `Use` / `UseExtra` | после `.cap` добавить `<span class="mn">` + `icon-drop` (отделён линией `--tb-sep`) |
| `Button.MenuMode` | `DontUse` / нет тега | без стрелки |
| `Button.Shortcut` | любое значение | не отображать: в обычной форме 1С сочетание клавиш на кнопке не подписывается (видно только в пунктах меню); превью добавляет его в `title=` как «(Ctrl+Enter)» |
| `Button.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `Button.TransparentBack` | нет тега / `false` | градиент `--btn-from`→`--btn-mid`→`--btn-to`, рамка `var(--border)` |
| `Button.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-Button` | градиент, рамка `var(--border)` | `data-xml="Button.Caption"`, `data-state="normal"` |
| disabled | `.t-Button.dis` | серый текст, светлый градиент | `data-xml="Button.Enabled"`, `data-state="disabled"` |
| default | `.t-Button.def` | жирная подпись, рамка `#8F8866` | `data-xml="Button.DefaultButton"`, `data-state="normal"` |
| hover / pressed | — | не показываем (нет мыши) | — |

### Особые случаи

- Очень узкая кнопка (< 24 px): подпись не выводить, только картинка.
- Высота кнопки 19–23 px: подпись по центру; выше 28 px с `MultiLine` — перенос.
- Нет подписи и картинки — пустая кнопка с градиентом (не подставлять имя).

## §6. Флажок — `CheckBox`

### Структура HTML

```html
<div class="c t-CheckBox [tl-Left] [dis] [hid] [ha-*] [va-*] [tr-bg] [bd-<type>]" style="left;top;width;height[;background]">
  <span class="box [on] [mix]" [style="border-color;background;--check-ink"]></span>
  <span class="cap [b] [f-i] [f-u] [f-s]" [style="color"]>Подпись</span>
</div>
```

### CSS

Правила из `theme.css` (раздел 5), вставлены без изменений:

```css
/* ---------- 5. Флажок и переключатель — CheckBox / RadioButton ---------- */
.t-CheckBox,.t-RadioButton{display:flex;align-items:center;gap:3px}
.t-CheckBox.tl-Left,.t-RadioButton.tl-Left{flex-direction:row-reverse;justify-content:flex-end}
.box,.rad{position:relative;width:13px;height:13px;border:1px solid var(--check-border);background:linear-gradient(135deg,#F4F2E8 0,#FFFFFF 70%);display:inline-block;flex:none}
.box{border-radius:var(--r-check)}
.rad{border-radius:50%}
.box.on::after{content:"";position:absolute;left:3px;top:0;width:3px;height:7px;border:solid var(--check-ink);border-width:0 2px 2px 0;transform:rotate(40deg)}
.box.mix::after{content:"";position:absolute;left:2px;top:2px;right:2px;bottom:2px;background:#8A8466}
.rad.on::after{content:"";position:absolute;left:3px;top:3px;width:5px;height:5px;border-radius:50%;background:var(--check-ink)}
.c.dis .box,.c.dis .rad{border-color:#C9C4AB;background:#F7F5EC}
.c.dis .box.on::after{border-color:#B9B49E}
.c.dis .rad.on::after{background:#B9B49E}

/* общие модификаторы (раздел 1 theme.css), используются этим классом */
/* ---------- 1. Общие модификаторы (§0.6, §0.7, §1) ---------- */
#canvas .c,.canvas .c{font:var(--font)}
.c .cap{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}
/* Font.* (JS: классы на .cap/.val/.colh) */
.b{font-weight:bold}
.f-i{font-style:italic}
.f-u{text-decoration:underline}
.f-s{text-decoration:line-through}
.f-u.f-s{text-decoration:underline line-through}
/* недоступный элемент (Enabled=false) (JS: класс .dis на корне .c) */
.c.dis,.c.dis .cap,.c.dis .val{color:var(--text-dis)!important}
.c.dis svg{opacity:.45;filter:grayscale(1)}
/* скрытый элемент в режиме «показывать скрытые» — без изменений */
#canvas .c.hid,.canvas .c.hid{opacity:.35;outline:1px dashed #888}
/* выравнивание текста (HorizontalAlign / VerticalAlign) (JS) */
.ha-Left{justify-content:flex-start!important;text-align:left}
.ha-Center{justify-content:center!important;text-align:center}
.ha-Right{justify-content:flex-end!important;text-align:right}
.ha-Justify{justify-content:flex-start!important;text-align:justify}
.va-Top{align-items:flex-start!important}
.va-Center{align-items:center!important}
.va-Bottom{align-items:flex-end!important}
/* рамка по Border.type (JS: класс на корне .c). Цвет — inline border-color из BorderColor, иначе --border */
.bd-None{border-style:none!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented{border-width:1px!important;border-style:solid!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented,.bd-Double,.bd-Overline,.bd-Underline{border-color:var(--border)}
.bd-Single3D{box-shadow:inset 1px 1px 0 #fff}
.bd-Double{border-width:3px!important;border-style:double!important}
.bd-Rounded{border-radius:4px!important}
.bd-Overline{border-style:none!important;border-top-style:solid!important;border-top-width:1px!important}
.bd-Underline{border-style:none!important;border-bottom-style:solid!important;border-bottom-width:1px!important}
.bd-Embossed{box-shadow:inset -1px -1px 0 #fff,inset 1px 1px 0 #D9D3B6}
.bd-Indented{box-shadow:inset 1px 1px 0 #D9D3B6,inset -1px -1px 0 #fff}
/* прозрачный фон (TransparentBack=true) */
.tr-bg{background:transparent!important}
/* картинка-заглушка 16×16 (Picture.refGuid задан, пикселей нет) */
.pic16{display:inline-flex;flex:none;width:16px;height:16px;align-items:center;justify-content:center}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `CheckBox.Caption` | `<Caption>` | текст `.cap` |
| `CheckBox.Caption` | пусто | только квадрат |
| `CheckBox.Caption` | состояние | designer: квадрат пустой (данных нет); `.box.on` — только для образцов/данных |
| `CheckBox.Font` | `<Font … bold="true"/>` | класс `.b` на `.cap` (стили к квадрату не применяются) |
| `CheckBox.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.cap` (стили к квадрату не применяются) (подчёркнутый и зачёркнутый вместе — оба класса) |
| `CheckBox.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.cap` (стили к квадрату не применяются) по §1 (Font.face, Font.size, Font.scale) |
| `CheckBox.Font` | нет тега | шрифт формы `var(--font)` |
| `CheckBox.TitleLocation` | `TitleLeft` / `Left` | класс `.tl-Left` на ROOT (подпись слева, квадрат справа) |
| `CheckBox.TitleLocation` | `TitleRight` / нет тега | квадрат слева, подпись справа, зазор 3 px |
| `CheckBox.ThreeState` | `true` | designer: квадрат с серым заполнением (класс `.mix` — третье состояние «не определено») |
| `CheckBox.ThreeState` | `false` / нет | квадрат пустой |
| `CheckBox.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: подпись `var(--text-dis)`, квадрат: рамка `#C9C4AB`, фон `#F7F5EC`, галочка `#B9B49E` |
| `CheckBox.Enabled` | нет тега / `true` | без изменений |
| `CheckBox.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.cap` |
| `CheckBox.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CheckBox.TextColor` | `<TextColor kind="style"/>` без value / нет тега | чёрный |
| `CheckBox.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `CheckBox.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CheckBox.BackColor` | `<BackColor kind="style"/>` без value / нет тега | прозрачный (фон формы) |
| `CheckBox.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `CheckBox.TransparentBack` | нет тега / `false` | фон по палитре класса |
| `CheckBox.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `CheckBox.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `CheckBox.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `CheckBox.BorderStyle` | нет тега | рамки вокруг элемента нет |
| `CheckBox.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на `.box` |
| `CheckBox.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CheckBox.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--check-border)` |
| `CheckBox.FieldBackColor` | `<FieldBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.box` |
| `CheckBox.FieldBackColor` | `<FieldBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CheckBox.FieldBackColor` | `<FieldBackColor kind="style"/>` без value / нет тега | градиент `#F4F2E8`→`#FFFFFF` |
| `CheckBox.FieldTextColor` | `<FieldTextColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на галочки `.box.on::after` (border-color) |
| `CheckBox.FieldTextColor` | `<FieldTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CheckBox.FieldTextColor` | `<FieldTextColor kind="style"/>` без value / нет тега | `var(--check-ink)` |
| `CheckBox.HorizontalAlign` | `Left` / `Center` / `Right` / `Justify` | класс `.ha-Left` / `.ha-Center` / `.ha-Right` / `.ha-Justify` на ROOT (группа «квадрат+подпись») |
| `CheckBox.HorizontalAlign` | `Auto` или нет тега | слева |
| `CheckBox.VerticalAlign` | `Top` / `Center` / `Bottom` | класс `.va-Top` / `.va-Center` / `.va-Bottom` на ROOT |
| `CheckBox.VerticalAlign` | нет тега | по центру высоты |
| `CheckBox.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `CheckBox.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `CheckBox.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.box` | пустой квадрат | `data-xml="CheckBox.Caption"`, `data-state="normal"` |
| checked | `.box.on` | галочка `var(--check-ink)` | `data-xml="CheckBox.Caption"`, `data-state="checked"` |
| indeterminate | `.box.mix` | серое заполнение | `data-xml="CheckBox.ThreeState"`, `data-state="normal"` |
| disabled | `.t-CheckBox.dis` | бледная рамка/фон, серая подпись | `data-xml="CheckBox.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Высота 13–15 px: квадрат 13 px не сжимать, подпись `line-height:13px`.
- Флажок без подписи — только квадрат у левого края.
- Состояние в designer всегда «не отмечен» (данных нет).

## §7. Переключатель — `RadioButton`

### Структура HTML

```html
<div class="c t-RadioButton [tl-Left] [dis] [hid] [ha-*] [va-*] [tr-bg] [bd-<type>]" style="left;top;width;height[;background]">
  <span class="rad [on]" [style="border-color;background;--check-ink"]></span>
  <span class="cap [b] [f-*]" [style="color"]>Подпись</span>
</div>
```

### CSS

Правила из `theme.css` (раздел 5), вставлены без изменений:

```css
/* ---------- 5. Флажок и переключатель — CheckBox / RadioButton ---------- */
.t-CheckBox,.t-RadioButton{display:flex;align-items:center;gap:3px}
.t-CheckBox.tl-Left,.t-RadioButton.tl-Left{flex-direction:row-reverse;justify-content:flex-end}
.box,.rad{position:relative;width:13px;height:13px;border:1px solid var(--check-border);background:linear-gradient(135deg,#F4F2E8 0,#FFFFFF 70%);display:inline-block;flex:none}
.box{border-radius:var(--r-check)}
.rad{border-radius:50%}
.box.on::after{content:"";position:absolute;left:3px;top:0;width:3px;height:7px;border:solid var(--check-ink);border-width:0 2px 2px 0;transform:rotate(40deg)}
.box.mix::after{content:"";position:absolute;left:2px;top:2px;right:2px;bottom:2px;background:#8A8466}
.rad.on::after{content:"";position:absolute;left:3px;top:3px;width:5px;height:5px;border-radius:50%;background:var(--check-ink)}
.c.dis .box,.c.dis .rad{border-color:#C9C4AB;background:#F7F5EC}
.c.dis .box.on::after{border-color:#B9B49E}
.c.dis .rad.on::after{background:#B9B49E}

/* общие модификаторы (раздел 1 theme.css), используются этим классом */
/* ---------- 1. Общие модификаторы (§0.6, §0.7, §1) ---------- */
#canvas .c,.canvas .c{font:var(--font)}
.c .cap{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}
/* Font.* (JS: классы на .cap/.val/.colh) */
.b{font-weight:bold}
.f-i{font-style:italic}
.f-u{text-decoration:underline}
.f-s{text-decoration:line-through}
.f-u.f-s{text-decoration:underline line-through}
/* недоступный элемент (Enabled=false) (JS: класс .dis на корне .c) */
.c.dis,.c.dis .cap,.c.dis .val{color:var(--text-dis)!important}
.c.dis svg{opacity:.45;filter:grayscale(1)}
/* скрытый элемент в режиме «показывать скрытые» — без изменений */
#canvas .c.hid,.canvas .c.hid{opacity:.35;outline:1px dashed #888}
/* выравнивание текста (HorizontalAlign / VerticalAlign) (JS) */
.ha-Left{justify-content:flex-start!important;text-align:left}
.ha-Center{justify-content:center!important;text-align:center}
.ha-Right{justify-content:flex-end!important;text-align:right}
.ha-Justify{justify-content:flex-start!important;text-align:justify}
.va-Top{align-items:flex-start!important}
.va-Center{align-items:center!important}
.va-Bottom{align-items:flex-end!important}
/* рамка по Border.type (JS: класс на корне .c). Цвет — inline border-color из BorderColor, иначе --border */
.bd-None{border-style:none!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented{border-width:1px!important;border-style:solid!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented,.bd-Double,.bd-Overline,.bd-Underline{border-color:var(--border)}
.bd-Single3D{box-shadow:inset 1px 1px 0 #fff}
.bd-Double{border-width:3px!important;border-style:double!important}
.bd-Rounded{border-radius:4px!important}
.bd-Overline{border-style:none!important;border-top-style:solid!important;border-top-width:1px!important}
.bd-Underline{border-style:none!important;border-bottom-style:solid!important;border-bottom-width:1px!important}
.bd-Embossed{box-shadow:inset -1px -1px 0 #fff,inset 1px 1px 0 #D9D3B6}
.bd-Indented{box-shadow:inset 1px 1px 0 #D9D3B6,inset -1px -1px 0 #fff}
/* прозрачный фон (TransparentBack=true) */
.tr-bg{background:transparent!important}
/* картинка-заглушка 16×16 (Picture.refGuid задан, пикселей нет) */
.pic16{display:inline-flex;flex:none;width:16px;height:16px;align-items:center;justify-content:center}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `RadioButton.Caption` | `<Caption>` | текст `.cap` |
| `RadioButton.Caption` | пусто | только круг |
| `RadioButton.SelectionValue` | `<SelectionValue>` у переключателя, который **первый** в группе (наименьший TabOrder/ZOrder среди переключателей с тем же DataPath) | designer: класс `.on` у `.rad` этого переключателя — точка `var(--check-ink)` 5×5 (в 1С значение реквизита по умолчанию соответствует первому варианту, см. 04) |
| `RadioButton.SelectionValue` | остальные переключатели группы | пустой круг |
| `RadioButton.SelectionValue` | нет тега | пустой круг |
| `RadioButton.Font` | `<Font … bold="true"/>` | класс `.b` на `.cap` |
| `RadioButton.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.cap` (подчёркнутый и зачёркнутый вместе — оба класса) |
| `RadioButton.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.cap` по §1 (Font.face, Font.size, Font.scale) |
| `RadioButton.Font` | нет тега | шрифт формы `var(--font)` |
| `RadioButton.TitleLocation` | `TitleLeft` / `Left` | класс `.tl-Left` (подпись слева) |
| `RadioButton.TitleLocation` | нет тега | подпись справа |
| `RadioButton.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: подпись `var(--text-dis)`, круг: рамка `#C9C4AB`, фон `#F7F5EC` |
| `RadioButton.Enabled` | нет тега / `true` | без изменений |
| `RadioButton.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.cap` |
| `RadioButton.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `RadioButton.TextColor` | `<TextColor kind="style"/>` без value / нет тега | чёрный |
| `RadioButton.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `RadioButton.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `RadioButton.BackColor` | `<BackColor kind="style"/>` без value / нет тега | прозрачный |
| `RadioButton.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `RadioButton.TransparentBack` | нет тега / `false` | фон по палитре класса |
| `RadioButton.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `RadioButton.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `RadioButton.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `RadioButton.BorderStyle` | нет тега | рамки вокруг элемента нет |
| `RadioButton.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на `.rad` |
| `RadioButton.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `RadioButton.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--check-border)` |
| `RadioButton.FieldBackColor` | `<FieldBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.rad` |
| `RadioButton.FieldBackColor` | `<FieldBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `RadioButton.FieldBackColor` | `<FieldBackColor kind="style"/>` без value / нет тега | градиент `#F4F2E8`→`#FFFFFF` |
| `RadioButton.FieldTextColor` | `<FieldTextColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на точки `.rad.on::after` (background) |
| `RadioButton.FieldTextColor` | `<FieldTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `RadioButton.FieldTextColor` | `<FieldTextColor kind="style"/>` без value / нет тега | `var(--check-ink)` |
| `RadioButton.HorizontalAlign` | `Left` / `Center` / `Right` / `Justify` | класс `.ha-Left` / `.ha-Center` / `.ha-Right` / `.ha-Justify` на ROOT |
| `RadioButton.HorizontalAlign` | `Auto` или нет тега | слева |
| `RadioButton.VerticalAlign` | `Top` / `Center` / `Bottom` | класс `.va-Top` / `.va-Center` / `.va-Bottom` на ROOT |
| `RadioButton.VerticalAlign` | нет тега | по центру высоты |
| `RadioButton.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `RadioButton.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `RadioButton.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.rad` | пустой круг | `data-xml="RadioButton.Caption"`, `data-state="checked"` |
| checked | `.rad.on` | точка 5×5 | `data-xml="RadioButton.SelectionValue"`, `data-state="checked"` |
| disabled | `.t-RadioButton.dis` | бледный круг, серая подпись | `data-xml="RadioButton.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Группа определяется общим DataPath; если DataPath нет — каждая кнопка сама по себе, ни одна не выбрана.
- Без подписи — только круг.

## §8. Табличное поле — `Table`

### Структура HTML

```html
<div class="c t-Table [vl] [no-hl] [dis] [hid] [tr-bg] [bd-<type>]" style="left;top;width;height[;--head-h][;--alt-bg][;color;background;border-color]">
  <div class="th" [style="height;background;color"]>
    <span class="colh [fixed] [ha-*] [b]" style="flex:0 0 Wpx;width:Wpx">Заголовок[<span class="srt"><!-- icon-sort-asc --></span>]</span> …
  </div>
  <!-- designer: одна строка-образец (UseAlternationRowColor → вторая с .alt) -->
  <div class="tr [sel] [alt]" style="top:<head-h>px">
    <span class="cell [ro] [num] [ctr] [fixed] [sel]" style="flex:0 0 Wpx;width:Wpx">
      <span class="abl">ab|</span>&lt;Поле ввода&gt; | <span class="box"></span> | 0,00 | <span class="tree">+</span><!-- icon-folder --> | <!-- icon-item -->
    </span> …
  </div>
  [<div class="tf" style="bottom:16px|0"><span class="fd [num] [b]" style="flex:0 0 Wpx;width:Wpx">FooterText</span> …</div>]   <!-- Footer=true -->
  [<div class="sb" [style="right:16px"]><span class="sbb"><!-- icon-arrow-left --></span><span class="sbt" style="flex:1"><i style="width:N%"></i></span><span class="sbb"><!-- icon-arrow-right --></span></div>]
  [<div class="vsb" [style="bottom:16px"]><span class="sbb"><!-- icon-arrow-up --></span><span class="sbb"><!-- icon-arrow-down --></span></div>]
</div>
```

### CSS

Правила из `theme.css` (раздел 8), вставлены без изменений:

```css
/* ---------- 8. Табличное поле — Table / TableColumn ---------- */
.t-Table{background:#FFFFFF;border:1px solid var(--border)}
.t-Table .th,.t-Table .tr,.t-Table .tf{position:absolute;left:0;right:0;display:flex;overflow:hidden}
.t-Table .th{top:0;height:var(--head-h);background:linear-gradient(180deg,var(--head-from) 0%,var(--head-to) 100%);border-bottom:1px solid var(--sep)}
.t-Table .colh{flex:0 0 auto;height:100%;padding:0 4px;display:flex;align-items:center;gap:3px;border-right:1px solid var(--head-sep);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.t-Table .th.mh .colh{white-space:normal;line-height:11px;justify-content:center;text-align:center} /* HeaderHeight > 1 */
.t-Table .colh .srt{margin-left:auto;font-size:7px;color:#8C8670}
.t-Table .tr{top:var(--head-h);height:var(--row-h);border-bottom:1px solid var(--row-line)}
.t-Table .cell{flex:0 0 auto;height:100%;display:flex;align-items:center;gap:3px;padding-left:3px;overflow:hidden;white-space:nowrap}
.t-Table .cell.ro{border-right:0}
.t-Table.vl .cell{border-right:1px solid var(--col-line)}
.t-Table.no-hl .tr{border-bottom-color:transparent}
.t-Table .tr.sel{background:var(--sel-bg);color:var(--sel-text);border-bottom-color:var(--sel-line)}
.t-Table .tr.alt{background:var(--alt-bg,#F7F6EE)}
.t-Table .cell.num{justify-content:flex-end;padding-right:3px}
.t-Table .cell.ctr{justify-content:center;padding-left:0}
.t-Table .cell .lnk{color:var(--link);text-decoration:underline}
.t-Table .cell .tree{width:9px;height:9px;border:1px solid #A9A9A9;background:#fff;font:7px/7px Tahoma,Arial,sans-serif;text-align:center;flex:none;color:#333}
.t-Table .abl{display:inline-block;border:1px solid #6B7B8B;background:#fff;font:bold 7px/8px Tahoma,Arial,sans-serif;padding:0 1px;color:#1D4B7A}
.t-Table .tf{height:var(--head-h);background:linear-gradient(180deg,var(--head-from) 0%,var(--head-to) 100%);border-top:1px solid var(--sep)}
.t-Table .tf .fd{flex:0 0 auto;height:100%;border-right:1px solid var(--head-sep);display:flex;align-items:center;padding:0 3px;overflow:hidden}
.t-Table .fixed{position:relative;z-index:1;box-shadow:1px 0 0 var(--head-sep)}
/* полосы прокрутки (горизонтальная .sb, вертикальная .vsb) */
.t-Table .sb{position:absolute;left:0;right:0;bottom:0;height:var(--sb);background:var(--scroll-bg);border-top:1px solid var(--scroll-border);display:flex;align-items:stretch;font-size:7px}
.t-Table .sb .sbb,.t-Table .vsb .sbb{flex:none;width:var(--sb);display:flex;align-items:center;justify-content:center;background:var(--cmdbar-bg);border:0;color:var(--scroll-arrow)}
.t-Table .sb .sbt{position:relative;margin:2px 1px}
.t-Table .sb .sbt i{position:absolute;left:0;top:0;bottom:0;background:linear-gradient(180deg,#FFFFFF,#ECE8D3);border:1px solid var(--scroll-border);border-radius:2px}
.t-Table .vsb{position:absolute;top:0;right:0;bottom:0;width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between;font-size:7px}
.t-Table .vsb .sbb{height:var(--sb)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Table.Footer` | `true` | подвал `.tf` над горизонтальной полосой (bottom = высота `.sb` или 0), ячейки `.fd` по ширинам колонок, текст FooterText |
| `Table.Footer` | `false` / нет | подвала нет |
| `Table.HeaderHeight` | `N` (строк) | высота шапки `20 + (N-1)*13` px: переменная `--head-h` на ROOT и `height` на `.th`; класс `.mh` на `.th` — подписи `.colh` переносятся по словам, по центру |
| `Table.HeaderHeight` | нет / `1` | 20 px |
| `Table.FooterHeight` | `N` (строк) | высота подвала `.tf` = `20 + (N-1)*13` px |
| `Table.FooterHeight` | нет / `1` | 20 px |
| `Table.HeaderFont` | `<HeaderFont …/>` | классы/стили §1 на все `.colh` |
| `Table.HeaderFont` | нет тега | шрифт таблицы, обычный (в 1С шапка не жирная в ≥ 99 % форм) |
| `Table.FooterFont` | `<FooterFont …/>` | классы/стили §1 на все `.fd` |
| `Table.FooterFont` | нет тега | шрифт таблицы |
| `Table.HeaderBackColor` | `<HeaderBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.th` и всех `.colh` (сплошной, без градиента) |
| `Table.HeaderBackColor` | `<HeaderBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.HeaderBackColor` | `<HeaderBackColor kind="style"/>` без value / нет тега | градиент `--head-from`→`--head-to` |
| `Table.HeaderTextColor` | `<HeaderTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на всех `.colh` |
| `Table.HeaderTextColor` | `<HeaderTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.HeaderTextColor` | `<HeaderTextColor kind="style"/>` без value / нет тега | чёрный |
| `Table.FooterBackColor` | `<FooterBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.tf` и всех `.fd` |
| `Table.FooterBackColor` | `<FooterBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.FooterBackColor` | `<FooterBackColor kind="style"/>` без value / нет тега | градиент шапки |
| `Table.FooterTextColor` | `<FooterTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на всех `.fd` |
| `Table.FooterTextColor` | `<FooterTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.FooterTextColor` | `<FooterTextColor kind="style"/>` без value / нет тега | чёрный |
| `Table.Font` | `<Font … bold="true"/>` | класс `.b` на ROOT (все ячейки и шапка, если нет HeaderFont) |
| `Table.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на ROOT (все ячейки и шапка, если нет HeaderFont) (подчёркнутый и зачёркнутый вместе — оба класса) |
| `Table.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на ROOT (все ячейки и шапка, если нет HeaderFont) по §1 (Font.face, Font.size, Font.scale) |
| `Table.Font` | нет тега | шрифт формы `var(--font)` |
| `Table.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT (область без строк) |
| `Table.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.BackColor` | `<BackColor kind="style"/>` без value / нет тега | белый |
| `Table.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на всех `.cell` (FieldTextColor важнее) |
| `Table.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.TextColor` | `<TextColor kind="style"/>` без value / нет тега | чёрный |
| `Table.FieldBackColor` | `<FieldBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на всех `.tr` (кроме `.sel`) |
| `Table.FieldBackColor` | `<FieldBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.FieldBackColor` | `<FieldBackColor kind="style"/>` без value / нет тега | белый |
| `Table.FieldTextColor` | `<FieldTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на всех `.cell` |
| `Table.FieldTextColor` | `<FieldTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.FieldTextColor` | `<FieldTextColor kind="style"/>` без value / нет тега | чёрный |
| `Table.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `Table.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `Table.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `Table.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `Table.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `Table.BorderStyle` | нет тега | рамка 1 px `var(--border)` |
| `Table.HorizontalLines` | `false` | класс `.no-hl` на ROOT: `.tr{border-bottom-color:transparent}` |
| `Table.HorizontalLines` | `true` / нет тега | линия `1px solid var(--row-line)` под каждой строкой |
| `Table.VerticalLines` | `true` / нет тега | класс `.vl` на ROOT: `.cell{border-right:1px solid var(--col-line)}` |
| `Table.VerticalLines` | `false` | без вертикальных линий в строках (шапка — с разделителями всегда) |
| `Table.UseAlternationRowColor` | `true` | строки с чётным номером получают класс `.alt` (`background:var(--alt-bg,#F7F6EE)`); в designer — вторая строка-образец (если выводится) |
| `Table.UseAlternationRowColor` | `false` / нет | все строки белые |
| `Table.AlternationRowBackColor` | `<AlternationRowBackColor kind="rgb" value="#RRGGBB"/>` | inline `--alt-bg:#RRGGBB` на переменной `--alt-bg` на ROOT |
| `Table.AlternationRowBackColor` | `<AlternationRowBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.AlternationRowBackColor` | `<AlternationRowBackColor kind="style"/>` без value / нет тега | `#F7F6EE` |
| `Table.SelectionBackColor` | `<SelectionBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на строки `.tr.sel` |
| `Table.SelectionBackColor` | `<SelectionBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.SelectionBackColor` | `<SelectionBackColor kind="style"/>` без value / нет тега | `var(--sel-bg)` #536AC2 |
| `Table.SelectionTextColor` | `<SelectionTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.tr.sel` |
| `Table.SelectionTextColor` | `<SelectionTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Table.SelectionTextColor` | `<SelectionTextColor kind="style"/>` без value / нет тега | `var(--sel-text)` белый |
| `Table.HorizontalScrollBar` | `UseAlways` / `true` | рисовать полосу горизонтальную (`.sb` внизу, высота `--sb`), даже если содержимого мало |
| `Table.HorizontalScrollBar` | `DontUse` / `false` | полосу горизонтальную не рисовать |
| `Table.HorizontalScrollBar` | `AutoUse` / `Auto` / нет тега | по правилу класса (см. «Особые случаи») |
| `Table.VerticalScrollBar` | `UseAlways` / `true` | рисовать полосу вертикальную (`.vsb` справа, ширина `--sb`, со стрелками `icon-arrow-up`/`icon-arrow-down`), даже если содержимого мало |
| `Table.VerticalScrollBar` | `DontUse` / `false` | полосу вертикальную не рисовать |
| `Table.VerticalScrollBar` | `AutoUse` / `Auto` / нет тега | по правилу класса (см. «Особые случаи») |
| `Table.InitialTreeView` | `NoExpand` / `ExpandTopLevel` / `ExpandAllLevels` | если у таблицы есть колонка с ShowHierarchy=true или таблица — дерево: в первой видимой колонке строки-образца перед текстом `icon-tree-plus` (свёрнутый узел) + `icon-folder` |
| `Table.InitialTreeView` | нет тега | как плоская таблица |
| `Table.SelectionMode` | `MultiLine` / `SingleLine` | не отображать: режим выделения влияет только на поведение; статично выделенная строка выглядит одинаково |
| `Table.RowSelectionMode` | `Cell` | у выделенной строки-образца (`.tr.sel`) синим залита только первая редактируемая ячейка: класс `.sel` на `.cell`, у `.tr` фон белый, рамка ячейки 1 px `var(--sel-bg)` |
| `Table.RowSelectionMode` | `Row` / нет | выделяется вся строка |
| `Table.FixedLeft` | `N` > 0 | первые N видимых колонок (`.colh`, `.cell`, `.fd`) получают класс `.fixed` (тень-разделитель 1 px справа от последней) — горизонтальная прокрутка их не сдвигает |
| `Table.FixedLeft` | `0` / нет | нет |
| `Table.FixedRight` | `N` > 0 | последние N видимых колонок — класс `.fixed` (разделитель слева) |
| `Table.FixedRight` | `0` / нет | нет |
| `Table.ReadOnly` | `true` | в строке-образце (designer) все ячейки пустые: `.cell.ro` без `abl`/`<Поле ввода>` (как колонка ReadOnly); фон белый |
| `Table.ReadOnly` | нет тега | по колонкам |
| `Table.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: шапка и текст ячеек `var(--text-dis)`, рамка без изменений |
| `Table.Enabled` | нет тега / `true` | без изменений |
| `Table.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `Table.TransparentBack` | нет тега / `false` | белый фон |
| `Table.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `Table.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `Table.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-Table` | шапка, строка-образец, полосы | `data-xml="Table.Visible"`, `data-state="normal"` |
| selected | `.tr.sel` | фон `var(--sel-bg)`, текст белый, линия `var(--sel-line)` | `data-xml="Table.SelectionBackColor"`, `data-state="selected"` |
| readonly | `.t-Table` (ReadOnly) | ячейки-образцы пустые | `data-xml="Table.ReadOnly"`, `data-state="readonly"` |
| disabled | `.t-Table.dis` | серый текст | `data-xml="Table.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Сумма ширин колонок > ширины таблицы: колонки обрезаются правым краем, бегунок `.sbt i` = `inner/total*100%` (минимум 8 %).
- Таблица ниже 40 px: строку-образец не выводить, только шапку и полосы.
- Пустой Header колонки — пустая ячейка шапки (в 1С имя не подставляется).
- Таблица-дерево (InitialTreeView задан или колонка ShowHierarchy) рисуется с узлом «+» и папкой в первой видимой колонке.
- Полосы при `AutoUse`/нет тега: горизонтальная рисуется всегда (так на снимках 01 и 06, даже когда колонки помещаются); вертикальная — только если строки не помещаются по высоте (в designer одна строка — вертикальной нет).

## §9. Колонка табличного поля — `TableColumn`

### Структура HTML

```html
<!-- колонка не имеет своего корня: это одна .colh в .th, по одной .cell в каждой .tr и одна .fd в .tf -->
<span class="colh [ha-*] [b] [f-*] [fixed]" style="flex:0 0 Wpx;width:Wpx[;background;color]" title="<Tooltip|Header>">[<!-- icon-picture -->]Заголовок</span>
<span class="cell [ro] [num] [ctr] [b] [f-*]" style="flex:0 0 Wpx;width:Wpx[;background;color]">образец</span>
<span class="fd [num] [b]" style="flex:0 0 Wpx;width:Wpx[;background;color]">FooterText</span>
```

### CSS

Правила из `theme.css` (раздел 8), вставлены без изменений:

```css
/* ---------- 8. Табличное поле — Table / TableColumn ---------- */
.t-Table{background:#FFFFFF;border:1px solid var(--border)}
.t-Table .th,.t-Table .tr,.t-Table .tf{position:absolute;left:0;right:0;display:flex;overflow:hidden}
.t-Table .th{top:0;height:var(--head-h);background:linear-gradient(180deg,var(--head-from) 0%,var(--head-to) 100%);border-bottom:1px solid var(--sep)}
.t-Table .colh{flex:0 0 auto;height:100%;padding:0 4px;display:flex;align-items:center;gap:3px;border-right:1px solid var(--head-sep);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.t-Table .th.mh .colh{white-space:normal;line-height:11px;justify-content:center;text-align:center} /* HeaderHeight > 1 */
.t-Table .colh .srt{margin-left:auto;font-size:7px;color:#8C8670}
.t-Table .tr{top:var(--head-h);height:var(--row-h);border-bottom:1px solid var(--row-line)}
.t-Table .cell{flex:0 0 auto;height:100%;display:flex;align-items:center;gap:3px;padding-left:3px;overflow:hidden;white-space:nowrap}
.t-Table .cell.ro{border-right:0}
.t-Table.vl .cell{border-right:1px solid var(--col-line)}
.t-Table.no-hl .tr{border-bottom-color:transparent}
.t-Table .tr.sel{background:var(--sel-bg);color:var(--sel-text);border-bottom-color:var(--sel-line)}
.t-Table .tr.alt{background:var(--alt-bg,#F7F6EE)}
.t-Table .cell.num{justify-content:flex-end;padding-right:3px}
.t-Table .cell.ctr{justify-content:center;padding-left:0}
.t-Table .cell .lnk{color:var(--link);text-decoration:underline}
.t-Table .cell .tree{width:9px;height:9px;border:1px solid #A9A9A9;background:#fff;font:7px/7px Tahoma,Arial,sans-serif;text-align:center;flex:none;color:#333}
.t-Table .abl{display:inline-block;border:1px solid #6B7B8B;background:#fff;font:bold 7px/8px Tahoma,Arial,sans-serif;padding:0 1px;color:#1D4B7A}
.t-Table .tf{height:var(--head-h);background:linear-gradient(180deg,var(--head-from) 0%,var(--head-to) 100%);border-top:1px solid var(--sep)}
.t-Table .tf .fd{flex:0 0 auto;height:100%;border-right:1px solid var(--head-sep);display:flex;align-items:center;padding:0 3px;overflow:hidden}
.t-Table .fixed{position:relative;z-index:1;box-shadow:1px 0 0 var(--head-sep)}
/* полосы прокрутки (горизонтальная .sb, вертикальная .vsb) */
.t-Table .sb{position:absolute;left:0;right:0;bottom:0;height:var(--sb);background:var(--scroll-bg);border-top:1px solid var(--scroll-border);display:flex;align-items:stretch;font-size:7px}
.t-Table .sb .sbb,.t-Table .vsb .sbb{flex:none;width:var(--sb);display:flex;align-items:center;justify-content:center;background:var(--cmdbar-bg);border:0;color:var(--scroll-arrow)}
.t-Table .sb .sbt{position:relative;margin:2px 1px}
.t-Table .sb .sbt i{position:absolute;left:0;top:0;bottom:0;background:linear-gradient(180deg,#FFFFFF,#ECE8D3);border:1px solid var(--scroll-border);border-radius:2px}
.t-Table .vsb{position:absolute;top:0;right:0;bottom:0;width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between;font-size:7px}
.t-Table .vsb .sbb{height:var(--sb)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `TableColumn.Header` | `<Header>` на языке `lang` | текст `.colh`, `ellipsis` |
| `TableColumn.Header` | пусто | пустая ячейка шапки (имя колонки — только в `title=`; в 1С пустой заголовок не заменяется) |
| `TableColumn.Width` | `N` | `.colh`, `.cell`, `.fd`: `flex:0 0 Npx;width:Npx` |
| `TableColumn.Width` | `0` / нет тега | 75 px |
| `TableColumn.Visible` | `false` | колонка не выводится (шапка, ячейки, подвал); в режиме «показывать скрытые» — с `opacity:.35` |
| `TableColumn.Visible` | нет / `true` | выводится |
| `TableColumn.ReadOnly` | `true` | designer: ячейка-образец пустая (`.cell.ro`) |
| `TableColumn.ReadOnly` | нет | designer: `abl <Поле ввода>` |
| `TableColumn.Editor` | `CheckBox` | в ячейке-образце `<span class="box"></span>` (13×13), класс `.ctr` — по центру |
| `TableColumn.Editor` | `InputField` / нет тега | `abl` + `<Поле ввода>` |
| `TableColumn.Editor` | `ChoiceField` | `abl` + `<Поле выбора>` |
| `TableColumn.Editor` | прочие | пустая ячейка |
| `TableColumn.ValueType` | `Number` | designer: ячейка-образец с текстом `0`/`0,00` (как InputField.ValueType), класс `.num`; вместо `abl <Поле ввода>`, если колонка ReadOnly |
| `TableColumn.ValueType` | прочие | designer: `abl` + `<Поле ввода>` у редактируемой колонки, пусто у ReadOnly |
| `TableColumn.HeaderFont` | `<HeaderFont …/>` | классы/стили §1 на `.colh` колонки |
| `TableColumn.HeaderFont` | нет | шрифт шапки таблицы |
| `TableColumn.HeaderTextColor` | `<HeaderTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.colh` колонки |
| `TableColumn.HeaderTextColor` | `<HeaderTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TableColumn.HeaderTextColor` | `<HeaderTextColor kind="style"/>` без value / нет тега | чёрный |
| `TableColumn.HeaderBackColor` | `<HeaderBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.colh` колонки |
| `TableColumn.HeaderBackColor` | `<HeaderBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TableColumn.HeaderBackColor` | `<HeaderBackColor kind="style"/>` без value / нет тега | градиент шапки |
| `TableColumn.HeaderHorizontalAlign` | `Center` / `Right` | класс `.ha-Center` / `.ha-Right` на `.colh` |
| `TableColumn.HeaderHorizontalAlign` | `Auto` / `Left` / нет | слева |
| `TableColumn.HorizontalAlignInColumn` | `Right` / `Center` | класс `.num` / `.ctr` на `.cell` колонки |
| `TableColumn.HorizontalAlignInColumn` | `Auto` / нет | число — вправо, остальное влево |
| `TableColumn.HeaderPicture` | `<HeaderPicture …/>` | `icon-picture` 16×16 в `.colh` перед текстом (без подписи — только значок, по центру) |
| `TableColumn.HeaderPicture` | нет | без картинки |
| `TableColumn.RowsPictures` | `<RowsPictures …/>` (или колонка с именем `Картинка`) | в ячейке-образце значок `icon-item` 16×16 по центру |
| `TableColumn.RowsPictures` | нет | нет значка |
| `TableColumn.FooterText` | `<FooterText>` на языке `lang` | текст в `.fd` этой колонки (`ellipsis`), если у таблицы Footer=true |
| `TableColumn.FooterText` | нет | пустая ячейка подвала |
| `TableColumn.FooterFont` | `<FooterFont …/>` | классы/стили §1 на `.fd` колонки |
| `TableColumn.FooterFont` | нет | шрифт подвала таблицы |
| `TableColumn.FooterBackColor` | `<FooterBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.fd` колонки |
| `TableColumn.FooterBackColor` | `<FooterBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TableColumn.FooterBackColor` | `<FooterBackColor kind="style"/>` без value / нет тега | градиент подвала |
| `TableColumn.FooterTextColor` | `<FooterTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.fd` колонки |
| `TableColumn.FooterTextColor` | `<FooterTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TableColumn.FooterTextColor` | `<FooterTextColor kind="style"/>` без value / нет тега | чёрный |
| `TableColumn.FieldBackColor` | `<FieldBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.cell` колонки |
| `TableColumn.FieldBackColor` | `<FieldBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TableColumn.FieldBackColor` | `<FieldBackColor kind="style"/>` без value / нет тега | белый |
| `TableColumn.FieldTextColor` | `<FieldTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.cell` колонки |
| `TableColumn.FieldTextColor` | `<FieldTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TableColumn.FieldTextColor` | `<FieldTextColor kind="style"/>` без value / нет тега | чёрный |
| `TableColumn.TextFont` | `<TextFont …/>` | классы/стили §1 на `.cell` колонки |
| `TableColumn.TextFont` | нет | шрифт таблицы |
| `TableColumn.Hyperlink` | `true` | текст ячейки-образца в `<span class="lnk">` (синий, подчёркнутый) |
| `TableColumn.Hyperlink` | нет | обычный |
| `TableColumn.MarkNegatives` | `true` | не отображать: превью не показывает данных, отрицательных чисел нет |
| `TableColumn.CheckBoxThreeState` | `true` | флажок в ячейке-образце — класс `.mix` (третье состояние) |
| `TableColumn.CheckBoxThreeState` | нет | пустой флажок |
| `TableColumn.ShowHierarchy` | `true` | в ячейке-образце перед текстом `icon-tree-plus` + `icon-folder` (уровень 1, отступ 2 px) |
| `TableColumn.ShowHierarchy` | нет | нет |
| `TableColumn.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: текст `.colh` и `.cell` колонки `var(--text-dis)` |
| `TableColumn.Enabled` | нет тега / `true` | без изменений |
| `TableColumn.Tooltip` | `<Tooltip>` | не отображать: на холсте подсказки нет; текст в `title=` у `.colh` (как сейчас) |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.colh` / `.cell` | по колонке | `data-xml="TableColumn.Width"`, `data-state="normal"` |
| disabled | `.colh`/`.cell` с `color:var(--text-dis)` | серый текст колонки | `data-xml="TableColumn.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Колонка с Width=0 — 75 px (так 1С подставляет ширину по умолчанию).
- Невидимая колонка не занимает места.
- Колонка шириной < 20 px — только разделители, без текста.

## §10. Панель (закладки) — `Panel`

### Структура HTML

```html
<div class="c t-Panel [no-tabs] [dis] [hid]" style="left;top;width;height">
  <div class="ptabs [btabs|vtabs|multi]" style="height:<Top первой страницы>px">        <!-- нет при ShowTabs=None -->
    <span class="ptab [on] [dis] [hid] [b] [f-*]" style="flex:none[;color]" data-k="…">[<span class="pic16">…</span>]Подпись</span> …
    [<span class="pscroll"><i class="[off]"><!-- icon-tab-left --></i><i><!-- icon-tab-right --></i></span>]  <!-- не помещаются -->
  </div>
  <div class="c t-Page [on]" style="left;top;width;height[;background;border-color]"> … элементы страницы … </div> …
</div>
```

### CSS

Правила из `theme.css` (раздел 7), вставлены без изменений:

```css
/* ---------- 7. Панель и страницы — Panel / PanelPage ---------- */
#canvas .t-Panel,.canvas .t-Panel{overflow:visible}
#canvas .t-Page,.canvas .t-Page{display:none;background:var(--form-bg);border:1px solid var(--border);overflow:hidden}
#canvas .t-Page.on,.canvas .t-Page.on{display:block}
.t-Panel.no-tabs>.t-Page{border:none!important}
.ptabs{position:absolute;left:0;top:0;right:0;display:flex;gap:2px;padding:0 2px;align-items:flex-end;overflow:hidden;user-select:none;z-index:2}
.ptab{background:var(--tab-bg);border:1px solid var(--sep);border-bottom:none;border-radius:var(--r-tab) var(--r-tab) 0 0;
  padding:0 8px;flex:0 1 auto;min-width:24px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;cursor:pointer;
  line-height:17px;height:18px;margin-bottom:0;font:var(--font)}
.ptabs .ptab{flex:none!important;max-width:none!important} /* полные подписи; лишние закладки уходят под .pscroll */
.ptab.on{background:var(--form-bg);border-color:var(--border);box-shadow:inset 0 2px 0 var(--tab-accent);position:relative;top:1px;height:19px;padding-top:1px;z-index:3}
.ptab.dis{color:var(--text-dis)}
.ptab.hid{opacity:.55;font-style:italic}
.ptab .pic16{display:inline-flex;vertical-align:-3px;margin-right:3px}
/* стрелки прокрутки закладок (JS: .pscroll, когда закладки не помещаются) */
.ptabs .pscroll{position:absolute;right:0;bottom:1px;display:flex;gap:0;background:var(--form-bg);padding-left:2px;z-index:4}
.ptabs .pscroll i{width:9px;height:14px;display:flex;align-items:center;justify-content:center;font-style:normal;font-size:7px;color:#3D3D3D}
.ptabs .pscroll i.off{color:#BDB79B}
/* закладки снизу и слева */
.ptabs.btabs{top:auto;bottom:0;align-items:flex-start}
.ptabs.btabs .ptab{border-top:none;border-bottom:1px solid var(--sep);border-radius:0 0 var(--r-tab) var(--r-tab)}
.ptabs.btabs .ptab.on{top:-1px;box-shadow:inset 0 -2px 0 var(--tab-accent);border-color:var(--border)}
.ptabs.vtabs{right:auto;bottom:0;flex-direction:column;align-items:stretch;padding:2px 0}
.ptabs.vtabs .ptab{border:1px solid var(--sep);border-right:none;border-radius:var(--r-tab) 0 0 var(--r-tab);height:18px;width:100%;padding:0 4px}
.ptabs.vtabs .ptab.on{top:0;left:1px;height:18px;padding-top:0;box-shadow:inset 2px 0 0 var(--tab-accent);border-color:var(--border)}
.ptabs.multi{flex-wrap:wrap-reverse;align-content:flex-start}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Panel.ShowTabs` | `Top`, `TopScrolling` | `.ptabs` сверху (как сейчас); закладки **не сжимаются** (`flex:none`, полные подписи); если сумма ширин > ширины панели — лишние обрезаются краем, справа `.pscroll` со стрелками `icon-tab-left`/`icon-tab-right` |
| `Panel.ShowTabs` | `TopMultiLine`, `TopMultilineTransposition` | `.ptabs.multi` — закладки в несколько рядов (снизу вверх), высота полосы = `Top` первой страницы |
| `Panel.ShowTabs` | `Bottom`, `BottomMultiLine`… | `.ptabs.btabs` снизу панели |
| `Panel.ShowTabs` | `LeftVertical`, `LeftHorizontal`, `Right…` | `.ptabs.vtabs` слева (Right… — справа, зеркально) |
| `Panel.ShowTabs` | `None`/`DontUse`/`code=0` | закладок нет: класс `.no-tabs` на ROOT, у страниц рамки нет, видна первая видимая |
| `Panel.ShowTabs` | нет тега | по геометрии первой страницы (как сейчас) |
| `Panel.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на всех `.t-Page` панели |
| `Panel.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Panel.BackColor` | `<BackColor kind="style"/>` без value / нет тега | `var(--form-bg)` |
| `Panel.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на всех `.ptab` |
| `Panel.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Panel.TextColor` | `<TextColor kind="style"/>` без value / нет тега | чёрный |
| `Panel.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на всех `.t-Page` панели и `.ptab.on` |
| `Panel.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Panel.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `Panel.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на всех `.t-Page` (рамки нет) |
| `Panel.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `Panel.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `Panel.BorderStyle` | нет тега | страницы: рамка 1 px `var(--border)` (если закладок нет — без рамки, класс `.no-tabs`) |
| `Panel.Font` | `<Font … bold="true"/>` | класс `.b` на всех `.ptab` |
| `Panel.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на всех `.ptab` (подчёркнутый и зачёркнутый вместе — оба класса) |
| `Panel.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на всех `.ptab` по §1 (Font.face, Font.size, Font.scale) |
| `Panel.Font` | нет тега | шрифт формы `var(--font)` |
| `Panel.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на всех `.t-Page` панели (`background:transparent!important`) |
| `Panel.TransparentBack` | нет тега / `false` | фон страниц `var(--form-bg)` |
| `Panel.Picture` | любое значение | не отображать: фоновая картинка панели недоступна (GUID без пикселей), в корпусе не встречается |
| `Panel.PanelPictureLocation` | `Top`/`Left`/`Bottom`/`Right`/`Center` | не отображать: фоновая картинка панели не встречается в корпусе; без пикселей показывать нечего |
| `Panel.PictureSize` | любое значение | не отображать: относится к фоновой картинке панели, которая не отображается |
| `Panel.ScrollPageMode` | `true` | на активной странице справа вертикальная полоса прокрутки (как `.sdoc-v`), если содержимое выходит за высоту страницы; иначе ничего |
| `Panel.ScrollPageMode` | нет | без полосы |
| `Panel.SeparateByPage` | `true`/`false` | не отображать: влияет на порядок обхода элементов, не на вид |
| `Panel.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: подписи закладок `var(--text-dis)` (класс `.dis` на `.ptab`); содержимое страниц — как у недоступных элементов |
| `Panel.Enabled` | нет тега / `true` | без изменений |
| `Panel.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `Panel.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `Panel.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.ptab` / `.ptab.on` | неактивные `var(--tab-bg)`, активная — фон страницы и полоска `var(--tab-accent)` | `data-xml="Panel.ShowTabs"`, `data-state="normal"` |
| disabled | `.ptab.dis` | серые подписи | `data-xml="Panel.Enabled"`, `data-state="normal"` |
| hover | — | не показываем | — |

### Особые случаи

- Закладки, не поместившиеся по ширине, обрезаются; справа `.pscroll` (левая стрелка серая, если первая закладка видна).
- Вложенные панели рисуются так же; у вложенной страницы рамка тоже `var(--border)`.
- Панель без видимых страниц — пустой прямоугольник с рамкой.

## §11. Страница панели — `PanelPage`

### Структура HTML

```html
<span class="ptab [on] [dis] [hid]" style="flex:none">[<span class="pic16"><!-- icon-picture --></span>]Title</span>   <!-- в .ptabs панели -->
<div class="c t-Page [on]" style="left;top;width;height"> … </div>
```

### CSS

Правила из `theme.css` (раздел 7), вставлены без изменений:

```css
/* ---------- 7. Панель и страницы — Panel / PanelPage ---------- */
#canvas .t-Panel,.canvas .t-Panel{overflow:visible}
#canvas .t-Page,.canvas .t-Page{display:none;background:var(--form-bg);border:1px solid var(--border);overflow:hidden}
#canvas .t-Page.on,.canvas .t-Page.on{display:block}
.t-Panel.no-tabs>.t-Page{border:none!important}
.ptabs{position:absolute;left:0;top:0;right:0;display:flex;gap:2px;padding:0 2px;align-items:flex-end;overflow:hidden;user-select:none;z-index:2}
.ptab{background:var(--tab-bg);border:1px solid var(--sep);border-bottom:none;border-radius:var(--r-tab) var(--r-tab) 0 0;
  padding:0 8px;flex:0 1 auto;min-width:24px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;cursor:pointer;
  line-height:17px;height:18px;margin-bottom:0;font:var(--font)}
.ptabs .ptab{flex:none!important;max-width:none!important} /* полные подписи; лишние закладки уходят под .pscroll */
.ptab.on{background:var(--form-bg);border-color:var(--border);box-shadow:inset 0 2px 0 var(--tab-accent);position:relative;top:1px;height:19px;padding-top:1px;z-index:3}
.ptab.dis{color:var(--text-dis)}
.ptab.hid{opacity:.55;font-style:italic}
.ptab .pic16{display:inline-flex;vertical-align:-3px;margin-right:3px}
/* стрелки прокрутки закладок (JS: .pscroll, когда закладки не помещаются) */
.ptabs .pscroll{position:absolute;right:0;bottom:1px;display:flex;gap:0;background:var(--form-bg);padding-left:2px;z-index:4}
.ptabs .pscroll i{width:9px;height:14px;display:flex;align-items:center;justify-content:center;font-style:normal;font-size:7px;color:#3D3D3D}
.ptabs .pscroll i.off{color:#BDB79B}
/* закладки снизу и слева */
.ptabs.btabs{top:auto;bottom:0;align-items:flex-start}
.ptabs.btabs .ptab{border-top:none;border-bottom:1px solid var(--sep);border-radius:0 0 var(--r-tab) var(--r-tab)}
.ptabs.btabs .ptab.on{top:-1px;box-shadow:inset 0 -2px 0 var(--tab-accent);border-color:var(--border)}
.ptabs.vtabs{right:auto;bottom:0;flex-direction:column;align-items:stretch;padding:2px 0}
.ptabs.vtabs .ptab{border:1px solid var(--sep);border-right:none;border-radius:var(--r-tab) 0 0 var(--r-tab);height:18px;width:100%;padding:0 4px}
.ptabs.vtabs .ptab.on{top:0;left:1px;height:18px;padding-top:0;box-shadow:inset 2px 0 0 var(--tab-accent);border-color:var(--border)}
.ptabs.multi{flex-wrap:wrap-reverse;align-content:flex-start}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `PanelPage.Title` | `<Title>` на языке `lang` | текст `.ptab` полностью (без `max-width`), `padding:0 8px` |
| `PanelPage.Title` | пусто | имя страницы |
| `PanelPage.Visible` | `false` | закладка не выводится; в режиме «показывать скрытые» — `.ptab.hid` |
| `PanelPage.Visible` | нет / `true` | выводится |
| `PanelPage.Enabled` | `false` | класс `.dis` на `.ptab` этой страницы: подпись `var(--text-dis)`; содержимое страницы как недоступное |
| `PanelPage.Enabled` | нет / `true` | обычная закладка |
| `PanelPage.TitlePicture` | `<TitlePicture …/>` | в `.ptab` перед подписью `<span class="pic16">` + `icon-picture` 16×16 |
| `PanelPage.TitlePicture` | нет | без значка |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal / selected | `.ptab` / `.ptab.on` | см. Panel | `data-xml="PanelPage.Title"`, `data-state="normal"` |
| disabled | `.ptab.dis` | серая подпись | `data-xml="PanelPage.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Скрытая страница не выводится (её закладки нет); в режиме «показывать скрытые» закладка полупрозрачная курсивом.
- Длинная подпись не обрезается внутри закладки; обрезается вся полоса закладок.

## §12. Группа (рамка) — `UsualGroup`

### Структура HTML

```html
<div class="c t-UsualGroup bs-SingleLine|bs-Frame|bs-None [hid]" style="left;top;width;height">
  <i class="gf" [style="border-color;background"]></i>                         <!-- нет при bs-None -->
  <span class="gt" [style="color;font-weight;background"]><span class="cap" data-k="…">Заголовок</span></span>   <!-- нет при пустом Caption -->
</div>
```

### CSS

Правила из `theme.css` (раздел 6), вставлены без изменений:

```css
/* ---------- 6. Группа — UsualGroup ---------- */
#canvas .t-UsualGroup,.canvas .t-UsualGroup{overflow:visible}
.t-UsualGroup .gt{position:absolute;color:var(--group-title);font-weight:bold;background:var(--form-bg);padding:0 2px;line-height:14px;white-space:nowrap;max-width:100%;overflow:hidden;text-overflow:ellipsis}
.t-UsualGroup.bs-Frame .gf{position:absolute;left:0;right:0;top:7px;bottom:0;border:1px solid var(--border);border-radius:3px}
.t-UsualGroup.bs-Frame .gt{left:6px;top:0}
.t-UsualGroup.bs-SingleLine .gt{left:0;top:0;background:transparent;padding:0}
.t-UsualGroup.bs-SingleLine .gf{position:absolute;left:0;right:0;bottom:0;border-bottom:1px solid var(--border)}
.t-UsualGroup.bs-None .gt{left:0;top:0;background:transparent;padding:0}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `UsualGroup.Caption` | `<Caption>` | текст `.gt`, `ellipsis` по ширине группы |
| `UsualGroup.Caption` | пусто | только линия/рамка |
| `UsualGroup.BorderStyle` | `SingleLine` (`code=4`) | `.bs-SingleLine`: заголовок слева сверху без подложки, линия 1 px `var(--border)` по нижнему краю элемента (у рамки высотой 16 px — сразу под текстом) |
| `UsualGroup.BorderStyle` | `Frame` (`code=6`) | `.bs-Frame`: рамка 1 px со скруглением 3 px от `top:7px` до низа, заголовок на линии с отступом 6 px и подложкой `var(--form-bg)` |
| `UsualGroup.BorderStyle` | `None` (`code=0`) | `.bs-None`: только заголовок |
| `UsualGroup.BorderStyle` | нет тега | высота > 20 px → Frame, иначе SingleLine (как сейчас) |
| `UsualGroup.Font` | `<Font bold="true" …/>` | заголовок жирный (как по умолчанию) |
| `UsualGroup.Font` | `<Font bold="false"/>` | inline `font-weight:normal` на `.gt` |
| `UsualGroup.Font` | `italic`/`underline`/`face`/`size` | по §1 на `.gt` |
| `UsualGroup.Font` | нет тега | жирный (стандарт 1С для заголовка группы) |
| `UsualGroup.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.gt` |
| `UsualGroup.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `UsualGroup.TextColor` | `<TextColor kind="style"/>` без value / нет тега | `var(--group-title)` #9F6500 |
| `UsualGroup.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT внутри рамки (`.gf`) |
| `UsualGroup.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `UsualGroup.BackColor` | `<BackColor kind="style"/>` без value / нет тега | прозрачный |
| `UsualGroup.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на `.gf` (линия/рамка) |
| `UsualGroup.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `UsualGroup.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `UsualGroup.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на `.gt` (заголовок без подложки — линия рамки проходит под текстом) (`background:transparent!important`) |
| `UsualGroup.TransparentBack` | нет тега / `false` | подложка заголовка `var(--form-bg)` |
| `UsualGroup.Enabled` | `false` | заголовок `.gt` — `var(--text-dis)`; линия без изменений |
| `UsualGroup.Enabled` | нет | обычный |
| `UsualGroup.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `UsualGroup.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `UsualGroup.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-UsualGroup` | жирный заголовок `var(--group-title)` | `data-xml="UsualGroup.BorderStyle"`, `data-state="normal"` |
| disabled | `.gt` с `color:var(--text-dis)` | серый заголовок | `data-xml="UsualGroup.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Группа высотой ≤ 20 px без BorderStyle — заголовок-линия (SingleLine).
- Заголовок длиннее ширины — `ellipsis`; линия SingleLine всё равно на всю ширину.
- Пустой Caption у Frame — рамка без разрыва.

## §13. Командная панель — `CommandBar`

### Структура HTML

```html
<div class="c t-CommandBar [bottom] [right|center] [tall] [vert] [tr-bg] [bd-None] [hid]" style="left;top;width;height[;background;border-color]">
  <span class="cbtn [ok] [on] [dis]"><span class="ic"><!-- icon-* 16×16 --></span><span class="cap">Подпись</span>[<span class="ar"><!-- icon-drop --></span>]</span>
  <span class="csep"></span>
  …
</div>
```

### CSS

Правила из `theme.css` (раздел 9), вставлены без изменений:

```css
/* ---------- 9. Командная панель — CommandBar / CommandBarButton ---------- */
.t-CommandBar{background:var(--cmdbar-bg);border-bottom:1px solid var(--border);display:flex;align-items:center;gap:1px;padding:0 2px;overflow:hidden}
.t-CommandBar.bottom{border-bottom:none;border-top:1px solid var(--border)}
.t-CommandBar.right{justify-content:flex-end}
.t-CommandBar.center{justify-content:center}
.t-CommandBar.tall{flex-wrap:wrap;align-content:flex-start;padding:2px 4px;gap:2px 4px;background:var(--form-bg);border:none}
.t-CommandBar.vert{flex-direction:column;align-items:stretch;padding:2px 1px;border-bottom:none;border-right:1px solid var(--border)}
.t-CommandBar.vert .csep{width:auto;height:1px;margin:2px 1px}
.t-CommandBar.tr-bg{border-color:transparent}
.cbtn{display:inline-flex;align-items:center;gap:3px;height:21px;padding:0 4px;border:1px solid transparent;border-radius:2px;white-space:nowrap;flex:none;color:var(--text)}
.cbtn .ic{width:16px;height:16px}
.cbtn.ok{font-weight:bold}
.cbtn.on{background:linear-gradient(180deg,#F3EFD9,#FDFBF0);border:1px solid var(--border)}
.cbtn.dis{color:var(--text-dis)}
.cbtn.dis .ic svg{opacity:.4;filter:grayscale(1)}
.cbtn .ar{font-size:7px;margin-left:1px}
.cbtn.hov{background:linear-gradient(180deg,#FFFDF2,#F6EFC9);border-color:#D9C88A}
.csep{flex:none;width:1px;height:16px;margin:0 2px;background:var(--sep)}
/* нижняя полоса формы: «OK | Записать | Закрыть» — разделители между текстовыми кнопками */
.t-CommandBar.bottom.right .cbtn+.cbtn{border-left:1px solid transparent;box-shadow:-2px 0 0 -1px var(--sep)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `CommandBar.AutoFill` | `true`, панель над таблицей / связана с таблицей | перед своими кнопками: `add copy edit del \| save up down sort-az sort-za \| filter filter-value filter-history filter-off \| output-list` (значки §0.5, только картинки) |
| `CommandBar.AutoFill` | `true`, верхняя панель формы элемента/документа | «Действия ▾» \| `copy`-нет; набор: «Действия ▾» `\|` свои кнопки `\|` «Перейти ▾» `\|` `help` |
| `CommandBar.AutoFill` | `true`, верхняя панель формы списка (на форме есть табличное поле — основной список) | «Действия ▾» `add add-group copy edit delete` `\|` `hierarchy` `\|` `filter filter-value filter-off` `\|` `refresh` `\|` «Перейти ▾» `\|` `help` (как в 01) |
| `CommandBar.AutoFill` | `true`, нижняя панель (`HorizontalAlign=Right`) | текстовые кнопки «OK» (жирная) `\|` «Записать» `\|` «Закрыть»; у документа — «OK» `\|` «Провести» `\|` «Записать» `\|` «Закрыть» |
| `CommandBar.AutoFill` | `false`/нет | только свои кнопки |
| `CommandBar.HorizontalAlign` | `Right` | класс `.right` (кнопки прижаты вправо) |
| `CommandBar.HorizontalAlign` | `Center` | класс `.center` |
| `CommandBar.HorizontalAlign` | `Left`/нет | слева |
| `CommandBar.Orientation` | `Vertical` | класс `.vert`: кнопки столбцом, разделители горизонтальные, линия справа |
| `CommandBar.Orientation` | `Horizontal`/`Auto`/нет | в строку (как сейчас) |
| `CommandBar.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `CommandBar.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CommandBar.BackColor` | `<BackColor kind="style"/>` без value / нет тега | `var(--cmdbar-bg)` |
| `CommandBar.ButtonBackColor` | `<ButtonBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на всех `.cbtn` |
| `CommandBar.ButtonBackColor` | `<ButtonBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CommandBar.ButtonBackColor` | `<ButtonBackColor kind="style"/>` без value / нет тега | прозрачный |
| `CommandBar.ButtonTextColor` | `<ButtonTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на всех `.cbtn` |
| `CommandBar.ButtonTextColor` | `<ButtonTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CommandBar.ButtonTextColor` | `<ButtonTextColor kind="style"/>` без value / нет тега | чёрный |
| `CommandBar.Font` | `<Font … bold="true"/>` | класс `.b` на всех `.cbtn .cap` |
| `CommandBar.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на всех `.cbtn .cap` (подчёркнутый и зачёркнутый вместе — оба класса) |
| `CommandBar.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на всех `.cbtn .cap` по §1 (Font.face, Font.size, Font.scale) |
| `CommandBar.Font` | нет тега | шрифт формы `var(--font)` |
| `CommandBar.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT (линия снизу/сверху) |
| `CommandBar.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CommandBar.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `CommandBar.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `CommandBar.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `CommandBar.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `CommandBar.BorderStyle` | нет тега | линия 1 px `var(--border)` снизу (у нижней полосы формы — сверху) |
| `CommandBar.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `CommandBar.TransparentBack` | нет тега / `false` | фон `var(--cmdbar-bg)` |
| `CommandBar.Secondary` | `true`/`false` | не отображать: признак второстепенной панели не меняет её вид |
| `CommandBar.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: все `.cbtn` получают `.dis` (текст серый, значки `grayscale`) |
| `CommandBar.Enabled` | нет тега / `true` | без изменений |
| `CommandBar.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `CommandBar.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `CommandBar.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-CommandBar` | фон `var(--cmdbar-bg)`, линия `var(--border)` | `data-xml="CommandBar.AutoFill"`, `data-state="normal"` |
| disabled | `.cbtn.dis` | серые подписи, значки `grayscale` | `data-xml="CommandBar.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Панель выше 32 px (`.tall`) — кнопки переносятся по строкам, фон формы, без линии.
- Нижняя полоса формы (`HorizontalAlign=Right`, внизу формы): линия сверху, между текстовыми кнопками разделители `--sep`.
- Пустая панель (нет кнопок, AutoFill=false) — только фон и линия.
- Кнопки, не поместившиеся по ширине, обрезаются (в 1С — «»» с выпадающим списком; в превью не рисуем).

## §14. Кнопка командной панели — `CommandBarButton`

### Структура HTML

```html
<span class="cbtn [ok] [on] [dis]" data-b="…" title="<Tooltip|Caption>">
  [<span class="ic"><!-- icon-* по action / icon-picture --></span>]
  [<span class="cap">Подпись</span>]
  [<span class="ar"><!-- icon-drop --></span>]     <!-- kind=Submenu -->
</span>
<span class="csep"></span>                         <!-- kind=Separator -->
```

### CSS

Правила из `theme.css` (раздел 9), вставлены без изменений:

```css
/* ---------- 9. Командная панель — CommandBar / CommandBarButton ---------- */
.t-CommandBar{background:var(--cmdbar-bg);border-bottom:1px solid var(--border);display:flex;align-items:center;gap:1px;padding:0 2px;overflow:hidden}
.t-CommandBar.bottom{border-bottom:none;border-top:1px solid var(--border)}
.t-CommandBar.right{justify-content:flex-end}
.t-CommandBar.center{justify-content:center}
.t-CommandBar.tall{flex-wrap:wrap;align-content:flex-start;padding:2px 4px;gap:2px 4px;background:var(--form-bg);border:none}
.t-CommandBar.vert{flex-direction:column;align-items:stretch;padding:2px 1px;border-bottom:none;border-right:1px solid var(--border)}
.t-CommandBar.vert .csep{width:auto;height:1px;margin:2px 1px}
.t-CommandBar.tr-bg{border-color:transparent}
.cbtn{display:inline-flex;align-items:center;gap:3px;height:21px;padding:0 4px;border:1px solid transparent;border-radius:2px;white-space:nowrap;flex:none;color:var(--text)}
.cbtn .ic{width:16px;height:16px}
.cbtn.ok{font-weight:bold}
.cbtn.on{background:linear-gradient(180deg,#F3EFD9,#FDFBF0);border:1px solid var(--border)}
.cbtn.dis{color:var(--text-dis)}
.cbtn.dis .ic svg{opacity:.4;filter:grayscale(1)}
.cbtn .ar{font-size:7px;margin-left:1px}
.cbtn.hov{background:linear-gradient(180deg,#FFFDF2,#F6EFC9);border-color:#D9C88A}
.csep{flex:none;width:1px;height:16px;margin:0 2px;background:var(--sep)}
/* нижняя полоса формы: «OK | Записать | Закрыть» — разделители между текстовыми кнопками */
.t-CommandBar.bottom.right .cbtn+.cbtn{border-left:1px solid transparent;box-shadow:-2px 0 0 -1px var(--sep)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `CommandBarButton.kind` | `Action` | `.cbtn` |
| `CommandBarButton.kind` | `Submenu`/`Popup` | `.cbtn` + после подписи `<span class="ar">` (`icon-drop`) |
| `CommandBarButton.kind` | `Separator` | `<span class="csep">` 1×16 px `var(--sep)`, отступы 2 px |
| `CommandBarButton.Caption` | `<Caption>` | текст `.cap` |
| `CommandBarButton.Caption` | пусто | имя кнопки; у подменю `Подменю`/`Подменю1` — «Действия»/«Перейти» |
| `CommandBarButton.representation` | `Picture` | только значок |
| `CommandBarButton.representation` | `Text` | только подпись |
| `CommandBarButton.representation` | `PictureText` | значок + подпись, зазор 3 px |
| `CommandBarButton.representation` | `Auto` | значок, если есть (Picture или стандартное действие со значком), иначе подпись |
| `CommandBarButton.checked` | `true` | класс `.on`: рамка `var(--border)`, фон `linear-gradient(#F3EFD9,#FDFBF0)` — «вдавленная» |
| `CommandBarButton.checked` | нет | без рамки |
| `CommandBarButton.action` | `Standard:<код>` из таблицы стандартных действий §0.5 | значок из §0.5 по коду (add/copy/edit/delete/…); у `Standard:141` — без значка, подпись «OK» жирная |
| `CommandBarButton.action` | своё действие (обработчик) | значок по Picture; если Picture нет — `representation` решает, без угадывания по имени (угадывание по словам «провод», «печать» оставить только как запасной вариант для representation=Picture) |
| `CommandBarButton.Picture` | `<Picture kind="1" refGuid="…"/>` | `<span class="ic">` + `icon-picture` (заглушка 16×16) |
| `CommandBarButton.Picture` | нет | значок по `action` (стандартные) или нет значка |
| `CommandBarButton.DefaultButton` | `true` | класс `.ok` (подпись жирная) |
| `CommandBarButton.DefaultButton` | нет | обычная (кроме `Standard:141`) |
| `CommandBarButton.Enabled` | `false` | класс `.dis` на `.cbtn`: подпись `var(--text-dis)`, значок `opacity:.4;filter:grayscale(1)` (как «Записать» в 05) |
| `CommandBarButton.Enabled` | нет | обычная |
| `CommandBarButton.Tooltip` | `<Tooltip>` | не отображать: подсказка всплывает при наведении; текст — в `title=` у `.cbtn` (как сейчас) |
| `CommandBarButton.Shortcut` | любое значение | не отображать: в обычной форме 1С сочетание клавиш на кнопке не подписывается (видно только в пунктах меню); превью добавляет его в `title=` как «(Ctrl+Enter)» |
| `CommandBarButton.ButtonOrder` | `Asc`/`Desc`/`DontOrder` | не отображать: порядок кнопок задаётся порядком в XML; в корпусе не встречается |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.cbtn` | без рамки | `data-xml="CommandBarButton.kind"`, `data-state="normal"` |
| pressed (checked) | `.cbtn.on` | рамка `var(--border)`, «вдавленный» градиент | `data-xml="CommandBarButton.checked"`, `data-state="pressed"` |
| disabled | `.cbtn.dis` | серая подпись, значок `opacity:.4;grayscale` | `data-xml="CommandBarButton.Enabled"`, `data-state="disabled"` |
| hover | `.cbtn.hov` | рамка `#D9C88A`, светлый градиент — только если превью когда-нибудь получит hover (сейчас не нужен) | — |

### Особые случаи

- Подменю без подписи: «Действия» / «Перейти» по имени (как сейчас).
- Кнопка без значка и подписи — не выводится.

## §15. Картинка (декорация) — `PictureDecoration`

### Структура HTML

```html
<!-- картинка без подписи, больше 24×24 -->
<div class="c t-PictureDecoration pic-box [pic-has] [bd-<type>] [tr-bg] [dis] [hid]" style="left;top;width;height[;background;border-color]">
  [<span class="nptxt">NonselectedPictureText</span>]
</div>
<!-- маленькая картинка или с подписью -->
<div class="c t-PictureDecoration [link] [dis]" style="…"><span class="pic16"><!-- icon-picture --></span><span class="cap [b]">Подпись</span></div>
```

### CSS

Правила из `theme.css` (раздел 10), вставлены без изменений:

```css
/* ---------- 10. Картинка — PictureDecoration ---------- */
.t-PictureDecoration{display:flex;align-items:center}
.t-PictureDecoration.pic-box{border:1px solid var(--border);background:transparent;display:flex;align-items:center;justify-content:center;padding:0}
.t-PictureDecoration.pic-box .pic-ph{display:none}
.t-PictureDecoration.pic-has{background:var(--ph-bg)}
.t-PictureDecoration.pic-has::after{content:"";width:16px;height:16px;flex:none;
  background:url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16'%3E%3Crect x='1.5' y='2.5' width='13' height='11' fill='%23fff' stroke='%23B3AC86'/%3E%3Cpath d='M3 12l3.5-4 2.5 3 2-2 2 3z' fill='%23C9C2A0'/%3E%3Ccircle cx='11' cy='5.5' r='1.3' fill='%23E8C34A'/%3E%3C/svg%3E") no-repeat center}
.t-PictureDecoration .nptxt{color:#8C8775;padding:2px;white-space:normal;text-align:center}
.t-PictureDecoration.link .cap{color:var(--link);text-decoration:underline}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `PictureDecoration.Picture` | `<Picture kind="1" refGuid="…"/>` | класс `.pic-box.pic-has`: рамка `var(--border)`, фон `var(--ph-bg)`, по центру `icon-picture` 16×16 (вместо «пейзажа») |
| `PictureDecoration.Picture` | `true` без refGuid (маленькая, ≤ 24 px) | значок `icon-picture` (или `icon-user` у `НадписьАвтор`) |
| `PictureDecoration.Picture` | нет тега, без подписи | класс `.pic-box`: только пустая рамка `var(--border)` (как в 1С без картинки) |
| `PictureDecoration.PictureSize` | `AutoSize`/`RealSize`/`Tile` | заглушка 16×16 в левом верхнем углу (`justify-content:flex-start;align-items:flex-start`, отступ 2 px) |
| `PictureDecoration.PictureSize` | `Proportionally`/`Stretch`/нет тега | заглушка 16×16 по центру |
| `PictureDecoration.Zoomable` | любое значение | не отображать: масштабирование колесом — поведение |
| `PictureDecoration.Hyperlink` | `true` | класс `.link`: подпись синяя подчёркнутая; рамка заглушки без изменений |
| `PictureDecoration.Hyperlink` | нет | обычная |
| `PictureDecoration.NonselectedPictureText` | `<NonselectedPictureText>` на языке `lang` | если картинки нет: текст по центру области (`.nptxt`, цвет `#8C8775`, перенос по словам) внутри пустой рамки |
| `PictureDecoration.NonselectedPictureText` | нет | пустая рамка |
| `PictureDecoration.UseContextMenu` | любое значение | не отображать: контекстное меню — поведение, на вид не влияет |
| `PictureDecoration.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `PictureDecoration.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `PictureDecoration.BackColor` | `<BackColor kind="style"/>` без value / нет тега | прозрачный (пустая рамка) / `var(--ph-bg)` у заглушки картинки |
| `PictureDecoration.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `PictureDecoration.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `PictureDecoration.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `PictureDecoration.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `PictureDecoration.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `PictureDecoration.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `PictureDecoration.BorderStyle` | нет тега | пустая рамка 1 px `var(--border)` (как логотип/печать в 02) |
| `PictureDecoration.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `PictureDecoration.TransparentBack` | нет тега / `false` | прозрачный |
| `PictureDecoration.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.cap` |
| `PictureDecoration.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `PictureDecoration.TextColor` | `<TextColor kind="style"/>` без value / нет тега | чёрный |
| `PictureDecoration.Font` | `<Font … bold="true"/>` | класс `.b` на `.cap` (подпись картинки с текстом) |
| `PictureDecoration.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.cap` (подпись картинки с текстом) (подчёркнутый и зачёркнутый вместе — оба класса) |
| `PictureDecoration.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.cap` (подпись картинки с текстом) по §1 (Font.face, Font.size, Font.scale) |
| `PictureDecoration.Font` | нет тега | шрифт формы `var(--font)` |
| `PictureDecoration.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: заглушка `opacity:.45`, подпись серая |
| `PictureDecoration.Enabled` | нет тега / `true` | без изменений |
| `PictureDecoration.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `PictureDecoration.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `PictureDecoration.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.pic-box` / `.pic-box.pic-has` | пустая рамка / заглушка | `data-xml="PictureDecoration.Picture"`, `data-state="normal"` |
| disabled | `.dis` | заглушка бледная | `data-xml="PictureDecoration.Enabled"`, `data-state="normal"` |

### Особые случаи

- Картинка ≤ 24×24 — просто значок 16×16 без рамки.
- Картинка с подписью — значок + текст в строку (как надпись).

## §16. Поле табличного документа — `SpreadsheetDocumentField`

### Структура HTML

```html
<div class="c t-SpreadsheetDocumentField [grid] [bd-<type>] [hid]" style="left;top;width;height[;border-color]">
  [<div class="shd-g"></div>]                                  <!-- ShowGroups -->
  [<div class="shd-h"></div><div class="shd-v"></div>]         <!-- ShowHeaders -->
  [<div class="sdoc-v"><span class="sbb">…</span><span class="sbb">…</span></div>]   <!-- VerticalScrollBar -->
  [<div class="sdoc-h"><span class="sbb">…</span><span class="sbb">…</span></div>]   <!-- HorizontalScrollBar -->
  [<span class="sdoc-c"></span>]                               <!-- угол, если обе полосы -->
</div>
```

### CSS

Правила из `theme.css` (раздел 12), вставлены без изменений:

```css
/* ---------- 12. Документы и прочие поля ---------- */
.t-SpreadsheetDocumentField,.t-TextDocumentField,.t-HTMLDocumentField,.t-GraphicalSchemaField,.t-ListBox,
.t-Chart,.t-PivotChart,.t-GanttChart,.t-Dendrogram,.t-GeographicalSchemaField,.t-PDFDocumentField,.t-ActiveXControl,.t-UnknownControl{background:#fff;border:1px solid var(--border);color:#8C8775}
.t-SpreadsheetDocumentField.grid{background-color:#fff;
  background-image:linear-gradient(#E3E0D3 1px,transparent 1px),linear-gradient(90deg,#E3E0D3 1px,transparent 1px);background-size:64px 17px;background-position:20px 16px}
.t-SpreadsheetDocumentField .shd-h{position:absolute;left:0;right:var(--sb);top:0;height:16px;background:linear-gradient(180deg,var(--head-from),var(--head-to));border-bottom:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-v{position:absolute;left:0;top:0;bottom:var(--sb);width:20px;background:linear-gradient(90deg,var(--head-from),var(--head-to));border-right:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-g{position:absolute;left:0;top:0;right:var(--sb);height:9px;border-bottom:1px solid var(--sep);background:var(--form-bg)}
.sdoc-v{position:absolute;top:0;right:0;bottom:var(--sb);width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between;font-size:7px}
.sdoc-h{position:absolute;left:0;right:var(--sb);bottom:0;height:var(--sb);background:var(--scroll-bg);border-top:1px solid var(--scroll-border);display:flex;align-items:stretch;justify-content:space-between;font-size:7px}
.sdoc-v .sbb,.sdoc-h .sbb{flex:none;width:var(--sb);height:var(--sb);display:flex;align-items:center;justify-content:center;background:var(--cmdbar-bg);border:0;color:var(--scroll-arrow)}
.sdoc-c{position:absolute;right:0;bottom:0;width:var(--sb);height:var(--sb);background:var(--cmdbar-bg)}
.t-TextDocumentField .txt-ph{position:absolute;left:3px;top:2px;color:#8C8775}
.t-HTMLDocumentField .html-ph{position:absolute;left:0;right:0;top:0;bottom:0;display:flex;align-items:center;justify-content:center;color:#B3AC86}
.t-GraphicalSchemaField.gs-grid{background-image:radial-gradient(#D7D2BC 1px,transparent 1px);background-size:10px 10px}
.t-ListBox{color:var(--text)}
.t-ListBox .lb-row{position:absolute;left:0;right:0;height:17px;display:flex;align-items:center;gap:3px;padding-left:3px;white-space:nowrap}
.t-ListBox .lb-row.sel{background:var(--sel-bg);color:var(--sel-text)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `SpreadsheetDocumentField.ShowGrid` | `true` / нет тега | класс `.grid` на ROOT: сетка 64×17 px линиями `#E3E0D3` |
| `SpreadsheetDocumentField.ShowGrid` | `false` | белое поле без сетки |
| `SpreadsheetDocumentField.ShowHeaders` | `true` | заголовки колонок `.shd-h` (16 px сверху) и строк `.shd-v` (20 px слева) с градиентом шапки |
| `SpreadsheetDocumentField.ShowHeaders` | `false` / нет тега | нет |
| `SpreadsheetDocumentField.ShowGroups` | `true` / нет тега | полоса группировок `.shd-g` высотой 9 px сверху (фон `var(--form-bg)`, линия `var(--sep)`) |
| `SpreadsheetDocumentField.ShowGroups` | `false` | нет |
| `SpreadsheetDocumentField.FixedBackground` | любое значение | не отображать: фиксация фона при прокрутке — поведение |
| `SpreadsheetDocumentField.HorizontalScrollBar` | `UseAlways` / `true` | рисовать полосу горизонтальную (`.sdoc-h`), даже если содержимого мало |
| `SpreadsheetDocumentField.HorizontalScrollBar` | `DontUse` / `false` | полосу горизонтальную не рисовать |
| `SpreadsheetDocumentField.HorizontalScrollBar` | `AutoUse` / `Auto` / нет тега | по правилу класса (см. «Особые случаи») |
| `SpreadsheetDocumentField.VerticalScrollBar` | `UseAlways` / `true` | рисовать полосу вертикальную (`.sdoc-v`), даже если содержимого мало |
| `SpreadsheetDocumentField.VerticalScrollBar` | `DontUse` / `false` | полосу вертикальную не рисовать |
| `SpreadsheetDocumentField.VerticalScrollBar` | `AutoUse` / `Auto` / нет тега | по правилу класса (см. «Особые случаи») |
| `SpreadsheetDocumentField.ReadOnly` | `<ReadOnly>true</ReadOnly>` / нет тега | не отображать: у табличного документа только просмотр не меняет вида поля |
| `SpreadsheetDocumentField.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `SpreadsheetDocumentField.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `SpreadsheetDocumentField.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `SpreadsheetDocumentField.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `SpreadsheetDocumentField.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `SpreadsheetDocumentField.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `SpreadsheetDocumentField.BorderStyle` | нет тега | рамка 1 px `var(--border)` |
| `SpreadsheetDocumentField.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `SpreadsheetDocumentField.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-SpreadsheetDocumentField` | белое поле, сетка, полосы | `data-xml="SpreadsheetDocumentField.ShowGrid"`, `data-state="normal"` |

### Особые случаи

- Полосы при `Auto`/нет тега: обе рисуются (как в 09); поле ниже 40 px — без полос.
- Данные отчёта (выведенные ячейки) не рисуются — только сетка.

## §17. Календарь — `CalendarField`

### Структура HTML

```html
<div class="c t-CalendarField [nonav] [nocur] [dis] [bd-<type>] [hid]" style="left;top;width;height[;background;color;--holiday;--sel-bg]">
  <div class="cal-hd"><span class="nav">◂◄</span><b>Октябрь 2026</b><span class="nav">►▸</span></div>
  <div class="cal-wk"><span>Пн</span>…<span class="wk">Сб</span><span class="wk">Вс</span></div>
  <div class="cal-gr"><span class="mut">28</span>…<span class="cur">22</span>…<span class="rng">21</span>…</div>
  <div class="cal-ft">22 октября 2026 г.</div>            <!-- нет при ShowCurrentDate=false -->
</div>
```

### CSS

Правила из `theme.css` (раздел 13), вставлены без изменений:

```css
/* ---------- 13. Календарь — CalendarField ---------- */
.t-CalendarField{border:1px solid var(--border);background:#fff;display:flex;flex-direction:column;font-size:10px;padding:1px;user-select:none;color:var(--text)}
.cal-hd{display:flex;justify-content:space-between;align-items:center;background:linear-gradient(180deg,var(--head-from),var(--head-to));padding:1px 3px;border-bottom:1px solid var(--sep);font:var(--font)}
.cal-hd .nav{color:#5A5A5A;font-size:8px}
.t-CalendarField.nonav .cal-hd .nav{visibility:hidden}
.cal-wk,.cal-gr{display:grid;grid-template-columns:repeat(7,1fr);text-align:center;line-height:14px;padding:0 2px}
.cal-wk{border-bottom:1px solid var(--sep);font-size:9px}
.cal-wk .wk,.cal-gr .wk{color:var(--holiday)}
.cal-gr .mut{color:#BDB8A6}
.cal-gr .cur{background:var(--sel-bg);color:var(--sel-text);font-weight:bold}
.cal-gr .rng{background:#C9D3F0}
.cal-ft{margin-top:auto;border-top:1px solid var(--sep);text-align:center;font-size:10px;line-height:15px;background:var(--form-bg)}
.t-CalendarField.nocur .cal-ft{display:none}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `CalendarField.FieldBackColor` | `<FieldBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `CalendarField.FieldBackColor` | `<FieldBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CalendarField.FieldBackColor` | `<FieldBackColor kind="style"/>` без value / нет тега | белый |
| `CalendarField.FieldTextColor` | `<FieldTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.cal-gr` |
| `CalendarField.FieldTextColor` | `<FieldTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CalendarField.FieldTextColor` | `<FieldTextColor kind="style"/>` без value / нет тега | чёрный |
| `CalendarField.ButtonBackColor` | `<ButtonBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.cal-hd` |
| `CalendarField.ButtonBackColor` | `<ButtonBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CalendarField.ButtonBackColor` | `<ButtonBackColor kind="style"/>` без value / нет тега | градиент шапки |
| `CalendarField.SelectionBackColor` | `<SelectionBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.cur` |
| `CalendarField.SelectionBackColor` | `<SelectionBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CalendarField.SelectionBackColor` | `<SelectionBackColor kind="style"/>` без value / нет тега | `var(--sel-bg)` |
| `CalendarField.SelectionTextColor` | `<SelectionTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.cur` |
| `CalendarField.SelectionTextColor` | `<SelectionTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CalendarField.SelectionTextColor` | `<SelectionTextColor kind="style"/>` без value / нет тега | белый |
| `CalendarField.HolidayColor` | `<HolidayColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на выходных `.wk` |
| `CalendarField.HolidayColor` | `<HolidayColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CalendarField.HolidayColor` | `<HolidayColor kind="style"/>` без value / нет тега | `var(--holiday)` |
| `CalendarField.ShowCurrentDate` | `false` | класс `.nocur`: подвал `.cal-ft` не выводится |
| `CalendarField.ShowCurrentDate` | `true` / нет | подвал «22 октября 2026 г.» |
| `CalendarField.CalendarNavigation` | `false` | класс `.nonav`: стрелки `.nav` в `.cal-hd` скрыты (`visibility:hidden`) |
| `CalendarField.CalendarNavigation` | `true` / нет | стрелки ◂◄ ►▸ |
| `CalendarField.SelectionMode` | `Interval` | designer: выделены 3 дня подряд — средние `.rng` (`#C9D3F0`), крайние `.cur` |
| `CalendarField.SelectionMode` | `Multiple` | выделены 2 несмежных дня `.cur` |
| `CalendarField.SelectionMode` | `Single` / нет | выделен один день `.cur` |
| `CalendarField.Font` | `<Font … bold="true"/>` | класс `.b` на ROOT (шапка, дни недели, числа) |
| `CalendarField.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на ROOT (шапка, дни недели, числа) (подчёркнутый и зачёркнутый вместе — оба класса) |
| `CalendarField.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на ROOT (шапка, дни недели, числа) по §1 (Font.face, Font.size, Font.scale) |
| `CalendarField.Font` | нет тега | шрифт формы `var(--font)` |
| `CalendarField.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `CalendarField.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `CalendarField.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `CalendarField.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `CalendarField.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `CalendarField.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `CalendarField.BorderStyle` | нет тега | рамка 1 px `var(--border)` |
| `CalendarField.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `CalendarField.TransparentBack` | нет тега / `false` | белый |
| `CalendarField.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: числа и шапка `var(--text-dis)`, выделение `#C9C4AB` |
| `CalendarField.Enabled` | нет тега / `true` | без изменений |
| `CalendarField.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `CalendarField.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `CalendarField.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal / selected | `.cur` | выделенная дата `var(--sel-bg)` | `data-xml="CalendarField.SelectionMode"`, `data-state="selected"` |
| disabled | `.t-CalendarField.dis` | серые числа | `data-xml="CalendarField.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Календарь меньше 140×110 — показывать только шапку и сетку без подвала.
- Месяц образца фиксированный (октябрь 2026), выделена дата 22.

## §18. Поле выбора — `ChoiceField`

### Структура HTML

```html
<div class="c t-ChoiceField [ro] [dis] [hid] [bd-<type>]" style="left;top;width;height[;background;border-color]">
  [<span class="pic16"><!-- icon-picture --></span>]
  <span class="val [b] [f-*]" [style="color"]></span>
  <span class="tb dd [colored] [tr-bg]"><!-- icon-drop | icon-picture 12×12 --></span>   <!-- ChoiceButton (по умолчанию есть) -->
  [<span class="tb"><!-- icon-clear --></span>] [<span class="tb"><!-- icon-open --></span>] [<span class="tb spin"><i>▲</i><i>▼</i></span>]
</div>
```

### CSS

Правила из `theme.css` (раздел 3, 11), вставлены без изменений:

```css
/* ---------- 3. Поле ввода — InputField (и общее для ChoiceField) ---------- */
.t-InputField,.t-ChoiceField{background:#FFFFFF;border:1px solid var(--border);border-radius:var(--r-field);display:flex;align-items:stretch}
.t-InputField .val,.t-ChoiceField .val{flex:1;min-width:0;position:relative;overflow:hidden;padding:0 2px;display:flex;align-items:center;white-space:pre;color:var(--text)}
.val.num{justify-content:flex-end;padding-right:3px}
.val.ph{color:#8C8775;font-style:normal}
.val.ml{align-items:flex-start;padding-top:1px;white-space:pre-wrap}
.val.pw{letter-spacing:1px}
.val .mark{position:absolute;left:2px;right:2px;bottom:1px;height:1px;border:0;
  background:repeating-linear-gradient(90deg,var(--mark) 0 1px,transparent 1px 2px)}
/* кнопки внутри поля: «…», календарь, калькулятор, «T», «×», лупа, ▾, ▲▼ */
.tb{flex:none;width:var(--tb-w);background:linear-gradient(180deg,var(--tb-from),var(--tb-to));border:0;border-left:1px solid var(--tb-sep);display:flex;align-items:center;justify-content:center;color:var(--tb-ink);font-size:9px;line-height:1;overflow:hidden}
.tb{font-weight:bold;font-size:11px;letter-spacing:-.5px} /* «...» текстом, пока JS не перейдёт на icon-choice */
.tb.bold{font:bold 11px Tahoma,Arial,sans-serif;color:#3D3D3D}
.tb:last-child{border-top-right-radius:1px;border-bottom-right-radius:1px}
.tb.spin{flex-direction:column;justify-content:stretch}
.tb.spin i{flex:1;display:flex;align-items:center;justify-content:center;font-style:normal;font-size:6px}
.tb.spin i+i{border-top:1px solid var(--tb-sep)}
.tb.colored{background:var(--tb-bg,#F1EEDD)}
/* только просмотр: белая полоса 2 px внутри рамки, середина цвета формы, кнопки бледнее */
.t-InputField.ro,.t-ChoiceField.ro{background:var(--form-bg);box-shadow:inset 0 0 0 2px #FFFFFF}
.t-InputField.ro .tb,.t-ChoiceField.ro .tb{opacity:.55}
/* недоступно: как «только просмотр», текст и значки бледные */
.t-InputField.dis,.t-ChoiceField.dis{background:var(--form-bg);box-shadow:none}
.t-InputField.dis .tb,.t-ChoiceField.dis .tb{opacity:.45}
/* картинка в поле (Picture) — слева от текста */
.t-InputField .pic16,.t-ChoiceField .pic16{margin:0 0 0 1px;align-self:center}
/* многострочное поле с полосой прокрутки (MultiLine + VerticalScrollBar) */
.t-InputField .vsb{flex:none;width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between}

/* ---------- 11. Поле выбора — ChoiceField ---------- */
.t-ChoiceField .tb.dd{font-size:7px}

/* общие модификаторы (раздел 1 theme.css), используются этим классом */
/* ---------- 1. Общие модификаторы (§0.6, §0.7, §1) ---------- */
#canvas .c,.canvas .c{font:var(--font)}
.c .cap{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;min-width:0}
/* Font.* (JS: классы на .cap/.val/.colh) */
.b{font-weight:bold}
.f-i{font-style:italic}
.f-u{text-decoration:underline}
.f-s{text-decoration:line-through}
.f-u.f-s{text-decoration:underline line-through}
/* недоступный элемент (Enabled=false) (JS: класс .dis на корне .c) */
.c.dis,.c.dis .cap,.c.dis .val{color:var(--text-dis)!important}
.c.dis svg{opacity:.45;filter:grayscale(1)}
/* скрытый элемент в режиме «показывать скрытые» — без изменений */
#canvas .c.hid,.canvas .c.hid{opacity:.35;outline:1px dashed #888}
/* выравнивание текста (HorizontalAlign / VerticalAlign) (JS) */
.ha-Left{justify-content:flex-start!important;text-align:left}
.ha-Center{justify-content:center!important;text-align:center}
.ha-Right{justify-content:flex-end!important;text-align:right}
.ha-Justify{justify-content:flex-start!important;text-align:justify}
.va-Top{align-items:flex-start!important}
.va-Center{align-items:center!important}
.va-Bottom{align-items:flex-end!important}
/* рамка по Border.type (JS: класс на корне .c). Цвет — inline border-color из BorderColor, иначе --border */
.bd-None{border-style:none!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented{border-width:1px!important;border-style:solid!important}
.bd-SingleLine,.bd-Frame,.bd-Single3D,.bd-Rounded,.bd-Embossed,.bd-Indented,.bd-Double,.bd-Overline,.bd-Underline{border-color:var(--border)}
.bd-Single3D{box-shadow:inset 1px 1px 0 #fff}
.bd-Double{border-width:3px!important;border-style:double!important}
.bd-Rounded{border-radius:4px!important}
.bd-Overline{border-style:none!important;border-top-style:solid!important;border-top-width:1px!important}
.bd-Underline{border-style:none!important;border-bottom-style:solid!important;border-bottom-width:1px!important}
.bd-Embossed{box-shadow:inset -1px -1px 0 #fff,inset 1px 1px 0 #D9D3B6}
.bd-Indented{box-shadow:inset 1px 1px 0 #D9D3B6,inset -1px -1px 0 #fff}
/* прозрачный фон (TransparentBack=true) */
.tr-bg{background:transparent!important}
/* картинка-заглушка 16×16 (Picture.refGuid задан, пикселей нет) */
.pic16{display:inline-flex;flex:none;width:16px;height:16px;align-items:center;justify-content:center}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `ChoiceField.ChoiceButton` | `false` | кнопки ▾ нет |
| `ChoiceField.ChoiceButton` | `true` / нет тега | `<span class="tb dd">` + `icon-drop` |
| `ChoiceField.ClearButton` | `true` | после ▾ `<span class="tb">` + `icon-clear` |
| `ChoiceField.ClearButton` | нет | нет |
| `ChoiceField.OpenButton` | `true` | последней `<span class="tb">` + `icon-open` |
| `ChoiceField.OpenButton` | нет | нет |
| `ChoiceField.SpinButton` | `true` | последней `<span class="tb spin">` (▲▼) |
| `ChoiceField.SpinButton` | нет | нет |
| `ChoiceField.ChoiceList` | `<ChoiceList>` | не отображать: список виден только раскрытым; поле показывает выбранное значение (данные). Элементы — в инспекторе |
| `ChoiceField.ChoiceButtonPicture` | `<ChoiceButtonPicture …/>` | в `.tb.dd` вместо ▾ — `icon-picture` 12×12 |
| `ChoiceField.ChoiceButtonPicture` | нет | ▾ |
| `ChoiceField.Picture` | `<Picture kind="1" refGuid="…"/>` (картинка из конфигурации) | вставить `<span class="pic16">` + `icon-picture` (§0.5) перед `.val` — пикселей картинки в Form.xml нет |
| `ChoiceField.Picture` | `<Picture>true</Picture>` без refGuid | то же, значок `icon-picture` |
| `ChoiceField.Picture` | нет тега | картинки нет |
| `ChoiceField.ChoiceListWidth` | любое значение | не отображать: размер раскрытого списка; в статике список закрыт |
| `ChoiceField.ChoiceListHeight` | любое значение | не отображать: размер раскрытого списка; в статике список закрыт |
| `ChoiceField.ReadOnly` | `true` | класс `.ro` (как InputField.ReadOnly) |
| `ChoiceField.ReadOnly` | нет | белое |
| `ChoiceField.Font` | `<Font … bold="true"/>` | класс `.b` на `.val` |
| `ChoiceField.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.val` (подчёркнутый и зачёркнутый вместе — оба класса) |
| `ChoiceField.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.val` по §1 (Font.face, Font.size, Font.scale) |
| `ChoiceField.Font` | нет тега | шрифт формы `var(--font)` |
| `ChoiceField.FieldBackColor` | `<FieldBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `ChoiceField.FieldBackColor` | `<FieldBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ChoiceField.FieldBackColor` | `<FieldBackColor kind="style"/>` без value / нет тега | белый |
| `ChoiceField.FieldTextColor` | `<FieldTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.val` |
| `ChoiceField.FieldTextColor` | `<FieldTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ChoiceField.FieldTextColor` | `<FieldTextColor kind="style"/>` без value / нет тега | чёрный |
| `ChoiceField.ButtonBackColor` | `<ButtonBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.tb` |
| `ChoiceField.ButtonBackColor` | `<ButtonBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ChoiceField.ButtonBackColor` | `<ButtonBackColor kind="style"/>` без value / нет тега | градиент кнопки поля |
| `ChoiceField.ButtonTextColor` | `<ButtonTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.tb` |
| `ChoiceField.ButtonTextColor` | `<ButtonTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ChoiceField.ButtonTextColor` | `<ButtonTextColor kind="style"/>` без value / нет тега | `var(--tb-ink)` |
| `ChoiceField.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `ChoiceField.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ChoiceField.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `ChoiceField.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `ChoiceField.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `ChoiceField.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `ChoiceField.BorderStyle` | нет тега | рамка 1 px `var(--border)`, скругление 2 px |
| `ChoiceField.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на кнопке `.tb` (`background:transparent!important`) |
| `ChoiceField.TransparentBack` | нет тега / `false` | градиент кнопки |
| `ChoiceField.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: как InputField: фон `var(--form-bg)`, текст серый, кнопка `opacity:.45` |
| `ChoiceField.Enabled` | нет тега / `true` | без изменений |
| `ChoiceField.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `ChoiceField.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `ChoiceField.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-ChoiceField` | как поле ввода с ▾ | `data-xml="ChoiceField.ChoiceButton"`, `data-state="normal"` |
| readonly | `.t-ChoiceField.ro` | как InputField.ro | `data-xml="ChoiceField.ReadOnly"`, `data-state="readonly"` |
| disabled | `.t-ChoiceField.dis` | как InputField.dis | `data-xml="ChoiceField.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Значение пустое и в designer, и в runtime (список — данные).
- Узкое поле: правила как у InputField.

## §19. Индикатор — `ProgressBar`

### Структура HTML

```html
<div class="c t-ProgressBar [ps-Broken|ps-BrokenTilt] [vert] [dis] [bd-<type>] [hid]" style="left;top;width;height[;background;border-color]">
  <div class="prg-fill"></div>
  [<span class="prg-pct">38%</span>]          <!-- ShowPercent -->
</div>
```

### CSS

Правила из `theme.css` (раздел 14), вставлены без изменений:

```css
/* ---------- 14. Индикатор — ProgressBar ---------- */
.t-ProgressBar{border:1px solid var(--border);background:#fff;padding:1px;position:relative}
.prg-fill{width:38%;height:100%;background:linear-gradient(180deg,#B9D88E 0%,#67A631 55%,#8CC152 100%);border:0}
.t-ProgressBar.ps-Broken .prg-fill{background:repeating-linear-gradient(90deg,#67A631 0 8px,transparent 8px 10px)}
.t-ProgressBar.ps-BrokenTilt .prg-fill{background:repeating-linear-gradient(115deg,#67A631 0 8px,transparent 8px 11px)}
.t-ProgressBar.vert .prg-fill{width:100%;height:38%;position:absolute;left:1px;right:1px;bottom:1px;width:auto}
.t-ProgressBar .prg-pct{position:absolute;left:0;right:0;top:0;bottom:0;display:flex;align-items:center;justify-content:center;font:var(--font);color:var(--text)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `ProgressBar.ShowPercent` | `true` | поверх полосы `<span class="prg-pct">38%</span>` по центру (образец) |
| `ProgressBar.ShowPercent` | нет | без текста |
| `ProgressBar.ShowStyle` | `Broken` / `BrokenTilt` | класс `.ps-Broken` / `.ps-BrokenTilt` (блоки 8 px через 2/3 px) |
| `ProgressBar.ShowStyle` | `Smooth` / нет | сплошная зелёная полоса |
| `ProgressBar.Orientation` | `Vertical` | класс `.vert`: заполнение снизу вверх, 38 % высоты |
| `ProgressBar.Orientation` | `Horizontal`/`Auto`/нет | слева направо |
| `ProgressBar.MinValue` | любое значение | не отображать: предел значения; доля образца фиксирована 38 % |
| `ProgressBar.MaxValue` | любое значение | не отображать: предел значения; без текущего значения (данных) доля образца фиксирована 38 % |
| `ProgressBar.Step` | любое значение | не отображать: шаг изменения — поведение |
| `ProgressBar.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `ProgressBar.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ProgressBar.BackColor` | `<BackColor kind="style"/>` без value / нет тега | белый |
| `ProgressBar.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `ProgressBar.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ProgressBar.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `ProgressBar.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `ProgressBar.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `ProgressBar.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `ProgressBar.BorderStyle` | нет тега | рамка 1 px `var(--border)` |
| `ProgressBar.Font` | `<Font … bold="true"/>` | класс `.b` на `.prg-pct` |
| `ProgressBar.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.prg-pct` (подчёркнутый и зачёркнутый вместе — оба класса) |
| `ProgressBar.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.prg-pct` по §1 (Font.face, Font.size, Font.scale) |
| `ProgressBar.Font` | нет тега | шрифт формы `var(--font)` |
| `ProgressBar.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: заполнение `filter:grayscale(1);opacity:.5` |
| `ProgressBar.Enabled` | нет тега / `true` | без изменений |
| `ProgressBar.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `ProgressBar.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `ProgressBar.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-ProgressBar` | заполнение 38 % | `data-xml="ProgressBar.ShowStyle"`, `data-state="normal"` |
| disabled | `.dis .prg-fill` | `grayscale`, `opacity:.5` | `data-xml="ProgressBar.Enabled"`, `data-state="normal"` |

### Особые случаи

- Высота < 8 px — только рамка.
- Процент ShowPercent рисуется поверх заполнения чёрным.

## §20. Полоса регулирования — `TrackBar`

### Структура HTML

```html
<div class="c t-TrackBar [vert] [dis] [tr-bg] [bd-<type>] [hid]" style="left;top;width;height[;background]">
  [<div class="trk-ticks top"><i></i>…</div>]               <!-- MarkingAppearance=TopLeft|Both -->
  <div class="trk-line" [style="border-color"]><span class="trk-thumb" [style="background"]></span></div>
  [<div class="trk-ticks"><i></i>…</div>]                   <!-- BottomRight (по умолчанию) | Both -->
</div>
```

### CSS

Правила из `theme.css` (раздел 15), вставлены без изменений:

```css
/* ---------- 15. Полоса регулирования — TrackBar ---------- */
.t-TrackBar{background:var(--form-bg);border:none;display:flex;flex-direction:column;justify-content:center;padding:2px 6px;position:relative}
.trk-ticks{display:flex;justify-content:space-between;align-items:flex-end;height:6px}
.trk-ticks i{width:1px;height:4px;background:#8C8670}
.trk-ticks.top{align-items:flex-start;order:-1}
.trk-line{position:relative;height:4px;background:#fff;border:1px solid var(--border);margin:4px 0}
.trk-thumb{position:absolute;left:2px;top:-6px;width:9px;height:14px;background:linear-gradient(180deg,var(--btn-from),var(--btn-to));border:1px solid var(--check-border);border-radius:1px 1px 4px 4px}
.t-TrackBar.vert{flex-direction:row;padding:6px 2px}
.t-TrackBar.vert .trk-line{width:4px;height:auto;margin:0 4px}
.t-TrackBar.vert .trk-ticks{flex-direction:column;width:6px;height:auto}
.t-TrackBar.vert .trk-ticks i{width:4px;height:1px}
.t-TrackBar.vert .trk-thumb{left:-6px;top:2px;width:14px;height:9px;border-radius:1px 4px 4px 1px}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `TrackBar.Orientation` | `Vertical` | класс `.vert`: линия вертикальная по центру, ползунок вверху, деления справа |
| `TrackBar.Orientation` | `Horizontal`/`Auto`/нет | горизонтально |
| `TrackBar.MarkingAppearance` | `DontShow` | `.trk-ticks` не выводить |
| `TrackBar.MarkingAppearance` | `TopLeft` | `.trk-ticks.top` (над линией) |
| `TrackBar.MarkingAppearance` | `Both` | два блока делений: над и под линией |
| `TrackBar.MarkingAppearance` | `BottomRight` / нет | под линией |
| `TrackBar.MarkingStep` | `N` | см. MaxValue (шаг делений) |
| `TrackBar.MarkingStep` | нет | 1 |
| `TrackBar.Step` | любое значение | не отображать: шаг перемещения — поведение |
| `TrackBar.LargeStep` | любое значение | не отображать: шаг клавиш PageUp/PageDown — поведение |
| `TrackBar.MinValue` | `N` | см. MaxValue |
| `TrackBar.MinValue` | нет | 0 |
| `TrackBar.MaxValue` | `N` | число делений `.trk-ticks i` = (MaxValue − MinValue) / MarkingStep + 1, но не более 21; ползунок у минимума |
| `TrackBar.MaxValue` | нет | 10 (11 делений при шаге 1) |
| `TrackBar.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `TrackBar.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TrackBar.BackColor` | `<BackColor kind="style"/>` без value / нет тега | `var(--form-bg)` |
| `TrackBar.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на делений `.trk-ticks i` (background) |
| `TrackBar.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TrackBar.TextColor` | `<TextColor kind="style"/>` без value / нет тега | `#8C8670` |
| `TrackBar.ButtonBackColor` | `<ButtonBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на `.trk-thumb` |
| `TrackBar.ButtonBackColor` | `<ButtonBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TrackBar.ButtonBackColor` | `<ButtonBackColor kind="style"/>` без value / нет тега | градиент кнопки |
| `TrackBar.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на `.trk-line` |
| `TrackBar.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TrackBar.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `TrackBar.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `TrackBar.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `TrackBar.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `TrackBar.BorderStyle` | нет тега | рамки вокруг нет |
| `TrackBar.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `TrackBar.TransparentBack` | нет тега / `false` | `var(--form-bg)` |
| `TrackBar.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: ползунок и деления `opacity:.5` |
| `TrackBar.Enabled` | нет тега / `true` | без изменений |
| `TrackBar.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `TrackBar.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `TrackBar.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-TrackBar` | ползунок у минимума | `data-xml="TrackBar.MaxValue"`, `data-state="normal"` |
| disabled | `.t-TrackBar.dis` | `opacity:.5` | `data-xml="TrackBar.Enabled"`, `data-state="normal"` |

### Особые случаи

- Число делений > 21 — рисовать 21.
- Ползунок всегда у минимума (значения нет).

## §21. Список значений — `ListBox`

### Структура HTML

```html
<div class="c t-ListBox [dis] [bd-<type>] [hid]" style="left;top;width;height[;background;color;border-color]">
  <div class="lb-row [sel]" style="top:Npx">[<span class="box"></span>][<!-- icon-item -->]<span class="cap">Элемент списка 1</span></div> …   <!-- designer: 3 строки-образца -->
</div>
```

### CSS

Правила из `theme.css` (раздел 12), вставлены без изменений:

```css
/* ---------- 12. Документы и прочие поля ---------- */
.t-SpreadsheetDocumentField,.t-TextDocumentField,.t-HTMLDocumentField,.t-GraphicalSchemaField,.t-ListBox,
.t-Chart,.t-PivotChart,.t-GanttChart,.t-Dendrogram,.t-GeographicalSchemaField,.t-PDFDocumentField,.t-ActiveXControl,.t-UnknownControl{background:#fff;border:1px solid var(--border);color:#8C8775}
.t-SpreadsheetDocumentField.grid{background-color:#fff;
  background-image:linear-gradient(#E3E0D3 1px,transparent 1px),linear-gradient(90deg,#E3E0D3 1px,transparent 1px);background-size:64px 17px;background-position:20px 16px}
.t-SpreadsheetDocumentField .shd-h{position:absolute;left:0;right:var(--sb);top:0;height:16px;background:linear-gradient(180deg,var(--head-from),var(--head-to));border-bottom:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-v{position:absolute;left:0;top:0;bottom:var(--sb);width:20px;background:linear-gradient(90deg,var(--head-from),var(--head-to));border-right:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-g{position:absolute;left:0;top:0;right:var(--sb);height:9px;border-bottom:1px solid var(--sep);background:var(--form-bg)}
.sdoc-v{position:absolute;top:0;right:0;bottom:var(--sb);width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between;font-size:7px}
.sdoc-h{position:absolute;left:0;right:var(--sb);bottom:0;height:var(--sb);background:var(--scroll-bg);border-top:1px solid var(--scroll-border);display:flex;align-items:stretch;justify-content:space-between;font-size:7px}
.sdoc-v .sbb,.sdoc-h .sbb{flex:none;width:var(--sb);height:var(--sb);display:flex;align-items:center;justify-content:center;background:var(--cmdbar-bg);border:0;color:var(--scroll-arrow)}
.sdoc-c{position:absolute;right:0;bottom:0;width:var(--sb);height:var(--sb);background:var(--cmdbar-bg)}
.t-TextDocumentField .txt-ph{position:absolute;left:3px;top:2px;color:#8C8775}
.t-HTMLDocumentField .html-ph{position:absolute;left:0;right:0;top:0;bottom:0;display:flex;align-items:center;justify-content:center;color:#B3AC86}
.t-GraphicalSchemaField.gs-grid{background-image:radial-gradient(#D7D2BC 1px,transparent 1px);background-size:10px 10px}
.t-ListBox{color:var(--text)}
.t-ListBox .lb-row{position:absolute;left:0;right:0;height:17px;display:flex;align-items:center;gap:3px;padding-left:3px;white-space:nowrap}
.t-ListBox .lb-row.sel{background:var(--sel-bg);color:var(--sel-text)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `ListBox.ShowCheckBox` | `true` | в строках-образцах перед текстом `.box` 13×13 |
| `ListBox.ShowCheckBox` | нет | без флажков |
| `ListBox.ShowPicture` | `true` | в строках-образцах перед текстом `icon-item` |
| `ListBox.ShowPicture` | нет | без значков |
| `ListBox.FieldBackColor` | `<FieldBackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `ListBox.FieldBackColor` | `<FieldBackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ListBox.FieldBackColor` | `<FieldBackColor kind="style"/>` без value / нет тега | белый |
| `ListBox.FieldTextColor` | `<FieldTextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.lb-row` |
| `ListBox.FieldTextColor` | `<FieldTextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ListBox.FieldTextColor` | `<FieldTextColor kind="style"/>` без value / нет тега | чёрный |
| `ListBox.ReadOnly` | `<ReadOnly>true</ReadOnly>` / нет тега | не отображать: вид списка не меняется |
| `ListBox.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `ListBox.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `ListBox.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `ListBox.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `ListBox.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `ListBox.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `ListBox.BorderStyle` | нет тега | рамка 1 px `var(--border)` |
| `ListBox.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: строки `var(--text-dis)`, выделение `#C9C4AB` |
| `ListBox.Enabled` | нет тега / `true` | без изменений |
| `ListBox.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `ListBox.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `ListBox.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal / selected | `.lb-row.sel` | первая строка выделена `var(--sel-bg)` | `data-xml="ListBox.Visible"`, `data-state="selected"` |
| disabled | `.t-ListBox.dis` | серые строки | `data-xml="ListBox.Enabled"`, `data-state="disabled"` |

### Особые случаи

- Строки-образцы: 3 шт. («Элемент списка 1…3»), первая выделена; если высота меньше — сколько поместится.

## §22. Разделитель — `Splitter`

### Структура HTML

```html
<div class="c t-Splitter [hor] [colored] [bd-<type>] [dis] [hid]" style="left;top;width;height[;--spl-bg]"></div>   <!-- ручка рисуется ::after -->
```

### CSS

Правила из `theme.css` (раздел 16), вставлены без изменений:

```css
/* ---------- 16. Разделитель — Splitter ---------- */
.t-Splitter{background:transparent;border:none}
.t-Splitter::after{content:"";position:absolute;left:50%;top:50%;width:2px;height:14px;margin:-7px 0 0 -1px;border-left:1px dotted #C9C2A0;border-right:1px dotted #C9C2A0}
.t-Splitter.hor::after{width:14px;height:2px;margin:-1px 0 0 -7px;border:0;border-top:1px dotted #C9C2A0;border-bottom:1px dotted #C9C2A0}
.t-Splitter.colored{background:var(--spl-bg)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Splitter.Orientation` | `Vertical` / нет тега | по центру вертикальная «ручка» 2×14 px из двух точечных линий `#C9C2A0` (`::after`) |
| `Splitter.Orientation` | `Horizontal` | класс `.hor`: ручка 14×2 px |
| `Splitter.Orientation` | `Auto` | по пропорциям: ширина < высоты → Vertical, иначе Horizontal |
| `Splitter.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT (класс `.colored`, `--spl-bg`) |
| `Splitter.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Splitter.BackColor` | `<BackColor kind="style"/>` без value / нет тега | прозрачный |
| `Splitter.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT (вместе с BorderStyle) |
| `Splitter.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `Splitter.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | рамки нет |
| `Splitter.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `Splitter.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `Splitter.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `Splitter.BorderStyle` | нет тега | рамки нет |
| `Splitter.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: ручка `opacity:.4` |
| `Splitter.Enabled` | нет тега / `true` | без изменений |
| `Splitter.Tooltip` | `<Tooltip>` есть | не отображать: на холсте подсказки нет (в 1С она всплывает при наведении). Текст выбранного языка (`Item language=lang`, иначе пусто) кладётся в атрибут `title=` ROOT вместо имени элемента; имя остаётся в инспекторе |
| `Splitter.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `Splitter.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-Splitter` | прозрачный, ручка по центру | `data-xml="Splitter.Orientation"`, `data-state="normal"` |
| disabled | `.t-Splitter.dis` | ручка `opacity:.4` | `data-xml="Splitter.Enabled"`, `data-state="normal"` |

### Особые случаи

- Разделитель шире 8 px или выше 8 px по обеим осям — ориентация по пропорциям.
- Ручку не рисовать, если длина < 20 px.

## §23. Поле текстового документа — `TextDocumentField`

### Структура HTML

```html
<div class="c t-TextDocumentField [dis] [bd-<type>] [hid]" style="left;top;width;height[;background;border-color]">
  <span class="txt-ph" [style="color;font-family"]>Текст</span>              <!-- designer -->
  <div class="sdoc-v" style="bottom:0"><span class="sbb">…</span><span class="sbb">…</span></div>
</div>
```

### CSS

Правила из `theme.css` (раздел 12), вставлены без изменений:

```css
/* ---------- 12. Документы и прочие поля ---------- */
.t-SpreadsheetDocumentField,.t-TextDocumentField,.t-HTMLDocumentField,.t-GraphicalSchemaField,.t-ListBox,
.t-Chart,.t-PivotChart,.t-GanttChart,.t-Dendrogram,.t-GeographicalSchemaField,.t-PDFDocumentField,.t-ActiveXControl,.t-UnknownControl{background:#fff;border:1px solid var(--border);color:#8C8775}
.t-SpreadsheetDocumentField.grid{background-color:#fff;
  background-image:linear-gradient(#E3E0D3 1px,transparent 1px),linear-gradient(90deg,#E3E0D3 1px,transparent 1px);background-size:64px 17px;background-position:20px 16px}
.t-SpreadsheetDocumentField .shd-h{position:absolute;left:0;right:var(--sb);top:0;height:16px;background:linear-gradient(180deg,var(--head-from),var(--head-to));border-bottom:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-v{position:absolute;left:0;top:0;bottom:var(--sb);width:20px;background:linear-gradient(90deg,var(--head-from),var(--head-to));border-right:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-g{position:absolute;left:0;top:0;right:var(--sb);height:9px;border-bottom:1px solid var(--sep);background:var(--form-bg)}
.sdoc-v{position:absolute;top:0;right:0;bottom:var(--sb);width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between;font-size:7px}
.sdoc-h{position:absolute;left:0;right:var(--sb);bottom:0;height:var(--sb);background:var(--scroll-bg);border-top:1px solid var(--scroll-border);display:flex;align-items:stretch;justify-content:space-between;font-size:7px}
.sdoc-v .sbb,.sdoc-h .sbb{flex:none;width:var(--sb);height:var(--sb);display:flex;align-items:center;justify-content:center;background:var(--cmdbar-bg);border:0;color:var(--scroll-arrow)}
.sdoc-c{position:absolute;right:0;bottom:0;width:var(--sb);height:var(--sb);background:var(--cmdbar-bg)}
.t-TextDocumentField .txt-ph{position:absolute;left:3px;top:2px;color:#8C8775}
.t-HTMLDocumentField .html-ph{position:absolute;left:0;right:0;top:0;bottom:0;display:flex;align-items:center;justify-content:center;color:#B3AC86}
.t-GraphicalSchemaField.gs-grid{background-image:radial-gradient(#D7D2BC 1px,transparent 1px);background-size:10px 10px}
.t-ListBox{color:var(--text)}
.t-ListBox .lb-row{position:absolute;left:0;right:0;height:17px;display:flex;align-items:center;gap:3px;padding-left:3px;white-space:nowrap}
.t-ListBox .lb-row.sel{background:var(--sel-bg);color:var(--sel-text)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `TextDocumentField.Font` | `<Font … bold="true"/>` | класс `.b` на `.txt-ph` (текст-образец) |
| `TextDocumentField.Font` | `italic="true"` / `underline="true"` / `strikeout="true"` | классы `.f-i` / `.f-u` / `.f-s` на `.txt-ph` (текст-образец) (подчёркнутый и зачёркнутый вместе — оба класса) |
| `TextDocumentField.Font` | `face="…"`, `size="…"`, `scale="…"` | inline `font-family` / `font-size` на `.txt-ph` (текст-образец) по §1 (Font.face, Font.size, Font.scale) |
| `TextDocumentField.Font` | нет тега | шрифт формы `var(--font)` |
| `TextDocumentField.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `TextDocumentField.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TextDocumentField.BackColor` | `<BackColor kind="style"/>` без value / нет тега | белый |
| `TextDocumentField.TextColor` | `<TextColor kind="rgb" value="#RRGGBB"/>` | inline `color:#RRGGBB` на `.txt-ph` |
| `TextDocumentField.TextColor` | `<TextColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TextDocumentField.TextColor` | `<TextColor kind="style"/>` без value / нет тега | `#8C8775` |
| `TextDocumentField.ReadOnly` | `<ReadOnly>true</ReadOnly>` / нет тега | не отображать: вид поля текстового документа не меняется |
| `TextDocumentField.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `TextDocumentField.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `TextDocumentField.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `TextDocumentField.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `TextDocumentField.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `TextDocumentField.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `TextDocumentField.BorderStyle` | нет тега | рамка 1 px `var(--border)` |
| `TextDocumentField.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: фон `var(--form-bg)`, текст-образец серый |
| `TextDocumentField.Enabled` | нет тега / `true` | без изменений |
| `TextDocumentField.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `TextDocumentField.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-TextDocumentField` | белое поле, полоса | `data-xml="TextDocumentField.Visible"`, `data-state="normal"` |
| disabled | `.dis` | фон формы | `data-xml="TextDocumentField.Enabled"`, `data-state="normal"` |

### Особые случаи

- Высота < 40 px — без полосы прокрутки.

## §24. Поле HTML-документа — `HTMLDocumentField`

### Структура HTML

```html
<div class="c t-HTMLDocumentField [bd-<type>] [hid]" style="left;top;width;height[;border-color]">
  <span class="html-ph">HTML</span>     <!-- designer -->
</div>
```

### CSS

Правила из `theme.css` (раздел 12), вставлены без изменений:

```css
/* ---------- 12. Документы и прочие поля ---------- */
.t-SpreadsheetDocumentField,.t-TextDocumentField,.t-HTMLDocumentField,.t-GraphicalSchemaField,.t-ListBox,
.t-Chart,.t-PivotChart,.t-GanttChart,.t-Dendrogram,.t-GeographicalSchemaField,.t-PDFDocumentField,.t-ActiveXControl,.t-UnknownControl{background:#fff;border:1px solid var(--border);color:#8C8775}
.t-SpreadsheetDocumentField.grid{background-color:#fff;
  background-image:linear-gradient(#E3E0D3 1px,transparent 1px),linear-gradient(90deg,#E3E0D3 1px,transparent 1px);background-size:64px 17px;background-position:20px 16px}
.t-SpreadsheetDocumentField .shd-h{position:absolute;left:0;right:var(--sb);top:0;height:16px;background:linear-gradient(180deg,var(--head-from),var(--head-to));border-bottom:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-v{position:absolute;left:0;top:0;bottom:var(--sb);width:20px;background:linear-gradient(90deg,var(--head-from),var(--head-to));border-right:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-g{position:absolute;left:0;top:0;right:var(--sb);height:9px;border-bottom:1px solid var(--sep);background:var(--form-bg)}
.sdoc-v{position:absolute;top:0;right:0;bottom:var(--sb);width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between;font-size:7px}
.sdoc-h{position:absolute;left:0;right:var(--sb);bottom:0;height:var(--sb);background:var(--scroll-bg);border-top:1px solid var(--scroll-border);display:flex;align-items:stretch;justify-content:space-between;font-size:7px}
.sdoc-v .sbb,.sdoc-h .sbb{flex:none;width:var(--sb);height:var(--sb);display:flex;align-items:center;justify-content:center;background:var(--cmdbar-bg);border:0;color:var(--scroll-arrow)}
.sdoc-c{position:absolute;right:0;bottom:0;width:var(--sb);height:var(--sb);background:var(--cmdbar-bg)}
.t-TextDocumentField .txt-ph{position:absolute;left:3px;top:2px;color:#8C8775}
.t-HTMLDocumentField .html-ph{position:absolute;left:0;right:0;top:0;bottom:0;display:flex;align-items:center;justify-content:center;color:#B3AC86}
.t-GraphicalSchemaField.gs-grid{background-image:radial-gradient(#D7D2BC 1px,transparent 1px);background-size:10px 10px}
.t-ListBox{color:var(--text)}
.t-ListBox .lb-row{position:absolute;left:0;right:0;height:17px;display:flex;align-items:center;gap:3px;padding-left:3px;white-space:nowrap}
.t-ListBox .lb-row.sel{background:var(--sel-bg);color:var(--sel-text)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `HTMLDocumentField.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `HTMLDocumentField.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `HTMLDocumentField.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `HTMLDocumentField.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `HTMLDocumentField.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `HTMLDocumentField.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `HTMLDocumentField.BorderStyle` | нет тега | рамка 1 px `var(--border)` |
| `HTMLDocumentField.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `HTMLDocumentField.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.t-HTMLDocumentField` | белое поле, подпись «HTML» | `data-xml="HTMLDocumentField.Visible"`, `data-state="normal"` |

### Особые случаи

- Содержимое (страница) не загружается — превью офлайн.

## §25. Поле графической схемы — `GraphicalSchemaField`

### Структура HTML

```html
<div class="c t-GraphicalSchemaField gs-grid [tr-bg] [dis] [bd-<type>] [hid]" style="left;top;width;height[;background-color;border-color]"></div>
```

### CSS

Правила из `theme.css` (раздел 12), вставлены без изменений:

```css
/* ---------- 12. Документы и прочие поля ---------- */
.t-SpreadsheetDocumentField,.t-TextDocumentField,.t-HTMLDocumentField,.t-GraphicalSchemaField,.t-ListBox,
.t-Chart,.t-PivotChart,.t-GanttChart,.t-Dendrogram,.t-GeographicalSchemaField,.t-PDFDocumentField,.t-ActiveXControl,.t-UnknownControl{background:#fff;border:1px solid var(--border);color:#8C8775}
.t-SpreadsheetDocumentField.grid{background-color:#fff;
  background-image:linear-gradient(#E3E0D3 1px,transparent 1px),linear-gradient(90deg,#E3E0D3 1px,transparent 1px);background-size:64px 17px;background-position:20px 16px}
.t-SpreadsheetDocumentField .shd-h{position:absolute;left:0;right:var(--sb);top:0;height:16px;background:linear-gradient(180deg,var(--head-from),var(--head-to));border-bottom:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-v{position:absolute;left:0;top:0;bottom:var(--sb);width:20px;background:linear-gradient(90deg,var(--head-from),var(--head-to));border-right:1px solid var(--sep)}
.t-SpreadsheetDocumentField .shd-g{position:absolute;left:0;top:0;right:var(--sb);height:9px;border-bottom:1px solid var(--sep);background:var(--form-bg)}
.sdoc-v{position:absolute;top:0;right:0;bottom:var(--sb);width:var(--sb);background:var(--scroll-bg);border-left:1px solid var(--scroll-border);display:flex;flex-direction:column;justify-content:space-between;font-size:7px}
.sdoc-h{position:absolute;left:0;right:var(--sb);bottom:0;height:var(--sb);background:var(--scroll-bg);border-top:1px solid var(--scroll-border);display:flex;align-items:stretch;justify-content:space-between;font-size:7px}
.sdoc-v .sbb,.sdoc-h .sbb{flex:none;width:var(--sb);height:var(--sb);display:flex;align-items:center;justify-content:center;background:var(--cmdbar-bg);border:0;color:var(--scroll-arrow)}
.sdoc-c{position:absolute;right:0;bottom:0;width:var(--sb);height:var(--sb);background:var(--cmdbar-bg)}
.t-TextDocumentField .txt-ph{position:absolute;left:3px;top:2px;color:#8C8775}
.t-HTMLDocumentField .html-ph{position:absolute;left:0;right:0;top:0;bottom:0;display:flex;align-items:center;justify-content:center;color:#B3AC86}
.t-GraphicalSchemaField.gs-grid{background-image:radial-gradient(#D7D2BC 1px,transparent 1px);background-size:10px 10px}
.t-ListBox{color:var(--text)}
.t-ListBox .lb-row{position:absolute;left:0;right:0;height:17px;display:flex;align-items:center;gap:3px;padding-left:3px;white-space:nowrap}
.t-ListBox .lb-row.sel{background:var(--sel-bg);color:var(--sel-text)}
```

### Свойства → правила

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `GraphicalSchemaField.ReadOnly` | `<ReadOnly>true</ReadOnly>` / нет тега | не отображать: вид поля схемы не меняется |
| `GraphicalSchemaField.BackColor` | `<BackColor kind="rgb" value="#RRGGBB"/>` | inline `background:#RRGGBB` на ROOT |
| `GraphicalSchemaField.BackColor` | `<BackColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `GraphicalSchemaField.BackColor` | `<BackColor kind="style"/>` без value / нет тега | белый |
| `GraphicalSchemaField.BorderColor` | `<BorderColor kind="rgb" value="#RRGGBB"/>` | inline `border-color:#RRGGBB` на ROOT |
| `GraphicalSchemaField.BorderColor` | `<BorderColor kind="style" … value="#RRGGBB"/>` | то же значение `value` (стилевой цвет с запасным value) |
| `GraphicalSchemaField.BorderColor` | `<BorderColor kind="style"/>` без value / нет тега | `var(--border)` |
| `GraphicalSchemaField.BorderStyle` | `<BorderStyle type="None"/>` | класс `.bd-None` на ROOT (рамки нет) |
| `GraphicalSchemaField.BorderStyle` | `type="SingleLine"` / `"Frame"` | класс `.bd-SingleLine` / `.bd-Frame`: `border:1px solid` цвет рамки (BorderColor, иначе `var(--border)`) |
| `GraphicalSchemaField.BorderStyle` | `type` = `Single3D` / `Double` / `Rounded` / `Overline` / `Underline` / `Embossed` / `Indented` | класс `.bd-<type>` (правила в theme.css, раздел 1) |
| `GraphicalSchemaField.BorderStyle` | нет тега | рамка 1 px `var(--border)` |
| `GraphicalSchemaField.TransparentBack` | `<TransparentBack>true</TransparentBack>` | добавить класс `.tr-bg` на ROOT (`background:transparent!important`) |
| `GraphicalSchemaField.TransparentBack` | нет тега / `false` | белый с точечной сеткой (`.gs-grid`) |
| `GraphicalSchemaField.Enabled` | `<Enabled>false</Enabled>` | добавить класс `.dis` на ROOT: сетка `opacity:.5` |
| `GraphicalSchemaField.Enabled` | нет тега / `true` | без изменений |
| `GraphicalSchemaField.Visible` | `<Visible>false</Visible>` | элемент не выводится; в режиме «показывать скрытые» выводится с классом `.hid` на ROOT (`opacity:.35; outline:1px dashed #888`) |
| `GraphicalSchemaField.Visible` | нет тега / `true` | выводить как обычно |

### Состояния

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | `.gs-grid` | точечная сетка | `data-xml="GraphicalSchemaField.Visible"`, `data-state="normal"` |
| disabled | `.dis` | `opacity:.5` | `data-xml="GraphicalSchemaField.Enabled"`, `data-state="normal"` |

### Особые случаи

- Элементы схемы не рисуются — только поле с сеткой.

## §26. Что не охвачено и что мы предлагаем изменить

**Изменения в `form_render.js`, без которых часть правил не заработает** (CSS для них уже в `theme.css`):

1. `inputField()`: читать `ClearButton`/`OpenButton` независимо от `ChoiceButton`; рисовать кнопки значками §0.5 (`icon-choice` вместо текста «...», `icon-clear` вместо «×»); добавлять `.ro`, `.dis`, `DropListButton`, `SpinButton`, `Mask`, `Format`, `PasswordMode`, `MultiLine`; в runtime не выводить путь к данным курсивом (класс `.ph` больше не нужен).
2. Общая функция `fontCls(e, tag)` → классы `.b .f-i .f-u .f-s` + inline `font-family/size` (§1) — вызывать для `Font`, `HeaderFont`, `FooterFont`, `TextFont` во всех классах (сейчас читается только `bold` у трёх классов).
3. Общая функция `colorOf(e, tag)` должна понимать `kind="style"` с `value` и без (§1, §0.6) и применяться ко всем `*Color` по таблицам классов (сейчас — только у надписей).
4. `ctl()`: ставить `.dis` при `Enabled=false`, `.tr-bg` при `TransparentBack=true`, `.bd-<type>` по `BorderStyle`, `.ha-*`/`.va-*` по выравниванию — для всех классов, где это есть в таблицах.
5. `tabsHtml()`: убрать ужатие закладок (`max-width:92px`, `flex-shrink`), добавить `.pscroll`, когда сумма ширин > ширины панели; поддержать `btabs`/`vtabs`/`multi` по `ShowTabs`; страница без закладок — `.no-tabs`.
6. `tableBox()`: `.vl`/`.no-hl` по линиям; значки и узлы дерева в строке-образце; `Editor=CheckBox` → `.box`; `FooterText`; `HeaderHeight`/`FooterHeight`; вертикальная полоса `.vsb`; выделенная строка `.sel` у основного списка формы.
7. `commandBar()`: наборы автозаполнения по месту панели (форма списка / форма элемента / над таблицей / нижняя полоса) — §13, `CommandBar.AutoFill`; разделители между текстовыми кнопками нижней полосы; значки по таблице §0.5 вместо угадывания по словам.
8. `ICON.*`: заменить набор значков на §0.5 (имена совпадают по смыслу: `cal`→`icon-calendar`, `calc`→`icon-calc`, `mag`→`icon-open` и т. д.); «пейзаж» `ICON.landscape` больше не используется.
9. Переключатель: выбирать первый в группе (по общему `DataPath`) в режиме designer.
10. PictureDecoration без картинки — `.pic-box` (пустая рамка), с картинкой — `.pic-box.pic-has`.

**Чего нет в каталоге, но видно в 1С:**

- Заголовок окна и его кнопки (свернуть/развернуть/закрыть) — системные, в превью только `.form-title` (вне холста).
- Табличное поле без шапки (дерево групп в 01): в Form.xml это, по-видимому, свойство шапки таблицы, которого нет в каталоге; предлагаем добавить чтение `<Header>false</Header>` у Table и не рисовать `.th`.
- «»» в конце командной панели, когда кнопки не помещаются; служебный маркер Конфигуратора (`icon-cfg-marker`) оставить только в режиме designer.
- Плашка вокруг надписи «Валюта: …» в 06 — это рамка надписи (`BorderStyle`), правило уже есть (§4).

**Предложения по упрощению:**

- Тему `xp` (запасную палитру) можно не развивать: снимки платформы 8.2/8.3 дают только палитру `v82`; переменные `xp` оставить как есть.
- Режим runtime отличать от designer только образцами значений и `<<Имя>>` у пустых надписей; все остальные правила одинаковы.

