# Каталог свойств потока обычных форм 1С 8.2 (610 свойств)

Автоматически синхронизировано с `1c-ordinary-forms-research/catalog.json` и `docs/coverage/data/properties.json`.

> **Запись этих свойств экспериментальна.** Статус `rw` означает, что `pack --experimental` умеет записать свойство, а не что 1С откроет результат. Без флага `pack` пишет только текст модулей. Правила и известные дефекты: [`experimental-form-editing.md`](experimental-form-editing.md).

> **Как читать эту карту (уточнено 28.10.2026).** Это каталог свойств платформы, которые `oneform` умеет читать из потока и записывать в него: статус `rw` ставится по каталогу и офлайн-тестам (`extract` → `pack` → чтение), а **не по проверке в живой 1С**. В живой 1С прогнано 40 свойств на 188 элементах обработки (`e2e_sweep.py`) плюс структурные правки (элементы, привязки, панели, таблицы); для остальных `rw`-свойств (примерно 440 из 481) живой проверки нет. «100 %» ниже — доля сериализуемых свойств каталога, которые поток может нести, а не доля того, что `extract` показывает в `Form.xml` или `pack` принимает от агента: картинки, форматы колонок, шрифты и цвета шапок таблиц `extract` не показывает (сохраняются нетронутыми, `docs/todo.md`, B9). Что агент реально может править — `docs/agent-guide.md` и `docs/xml-reference.md`.

## 1. Итоговая сводка

| Статус | Кол-во | % от всех (610) | % от сериализуемых в `FormStream.bin` (482) |
|---|---:|---:|---:|
| **🟢 Поток читает и пишет (`rw`: `extract` + `pack` + read-back, по каталогу и тестам)** | **481** | **78.9%** | **99.8%** |
| **🟡 Чтение (`ro`)** | **1** | **0.2%** | **0.2%** |
| **⚙️ Только рантайм 1С (`runtime`)** | **128** | **21.0%** | — |
| **⚪ Не покрыто (`unk`)** | **0** | **0.0%** | **0.0%** |
| **ИТОГО сериализуемых свойств каталога, которые поток несёт (`rw` + `ro`)** | **482 / 482** | — | **100.0%** |

> Что значит `rw`: `pack` умеет записать свойство и проверить его чтением из потока; `extract` выводит его в `Form.xml`, когда значение отличается от значения свежедобавленного элемента того же типа (разреженный вывод). На корпусе УТ 10.3 (1464 форм) встречаются 126 из 156 тегов; остальные 30 относятся к типам или значениям, которых в корпусе нет. Список тегов по типам элементов: `docs/xml-reference.md`. Бинарные записи, которые `oneform` ещё не умеет разбирать, не показываются в XML и при правке сохраняются нетронутыми.

## 2. Сводка по классам объектов и элементов управления

| Объект (`Form.xml` / `API 1C`) | Всего | 🟢 R/W | 🟡 R/O | ⚙️ Runtime | ⚪ Unk | Покрытие сериализуемых |
|---|---:|---:|---:|---:|---:|---:|
| `InputField` (`TextBox`) | 45 | 43 | 0 | 2 | 0 | **100.0%** (43/43) |
| `TableColumn` (`TableBoxColumn`) | 44 | 42 | 0 | 2 | 0 | **100.0%** (42/42) |
| `Table` (`TableBox`) | 46 | 40 | 0 | 6 | 0 | **100.0%** (40/40) |
| `SpreadsheetDocument` (`SpreadsheetDocument`) | 53 | 27 | 0 | 26 | 0 | **100.0%** (27/27) |
| `ChoiceField` (`ComboBox`) | 21 | 21 | 0 | 0 | 0 | **100.0%** (21/21) |
| `Form` (`Form`) | 38 | 21 | 0 | 17 | 0 | **100.0%** (21/21) |
| `CalendarField` (`CalendarBox`) | 21 | 19 | 0 | 2 | 0 | **100.0%** (19/19) |
| `LabelDecoration` (`Label`) | 19 | 19 | 0 | 0 | 0 | **100.0%** (19/19) |
| `Panel` (`Panel`) | 18 | 17 | 0 | 1 | 0 | **100.0%** (17/17) |
| `PictureField` (`PictureBox`) | 17 | 17 | 0 | 0 | 0 | **100.0%** (17/17) |
| `Button` (`Button`) | 16 | 15 | 0 | 1 | 0 | **100.0%** (15/15) |
| `CheckBox` (`CheckBox`) | 15 | 15 | 0 | 0 | 0 | **100.0%** (15/15) |
| `CommandBarButton` (`CommandBarButton`) | 15 | 15 | 0 | 0 | 0 | **100.0%** (15/15) |
| `RadioButton` (`RadioButton`) | 15 | 15 | 0 | 0 | 0 | **100.0%** (15/15) |
| `TrackBar` (`TrackBar`) | 15 | 15 | 0 | 0 | 0 | **100.0%** (15/15) |
| `CommandBar` (`CommandBar`) | 14 | 14 | 0 | 0 | 0 | **100.0%** (14/14) |
| `ProgressBar` (`ProgressBar`) | 12 | 12 | 0 | 0 | 0 | **100.0%** (12/12) |
| `Extension for controls located in a form` (`Extension for controls located in a form`) | 11 | 10 | 0 | 1 | 0 | **100.0%** (10/10) |
| `ListBox` (`ListBox`) | 10 | 9 | 0 | 1 | 0 | **100.0%** (9/9) |
| `Panel controls extension` (`Panel controls extension`) | 9 | 9 | 0 | 0 | 0 | **100.0%** (9/9) |
| `UsualGroup` (`GroupBox`) | 9 | 9 | 0 | 0 | 0 | **100.0%** (9/9) |
| `Font` (`Font`) | 8 | 8 | 0 | 0 | 0 | **100.0%** (8/8) |
| `SpreadsheetDocumentField` (`SpreadsheetDocumentField`) | 8 | 8 | 0 | 0 | 0 | **100.0%** (8/8) |
| `Extension for controls located in a spreadsheet document field` (`Extension for controls located in a spreadsheet document field`) | 8 | 7 | 0 | 1 | 0 | **100.0%** (7/7) |
| `Splitter` (`Splitter`) | 6 | 6 | 0 | 0 | 0 | **100.0%** (6/6) |
| `GraphicalSchemaField` (`GraphicalSchemaField`) | 6 | 5 | 0 | 1 | 0 | **100.0%** (5/5) |
| `PanelPage` (`PanelPage`) | 7 | 5 | 0 | 2 | 0 | **100.0%** (5/5) |
| `Color` (`Color`) | 4 | 4 | 0 | 0 | 0 | **100.0%** (4/4) |
| `Shortcut` (`Shortcut`) | 4 | 4 | 0 | 0 | 0 | **100.0%** (4/4) |
| `TextDocumentField` (`TextDocumentField`) | 5 | 4 | 0 | 1 | 0 | **100.0%** (4/4) |
| `ValueListItem` (`ValueListItem`) | 4 | 4 | 0 | 0 | 0 | **100.0%** (4/4) |
| `Border` (`Border`) | 3 | 3 | 0 | 0 | 0 | **100.0%** (3/3) |
| `HTMLDocumentField` (`HTMLDocumentField`) | 4 | 3 | 0 | 1 | 0 | **100.0%** (3/3) |
| `NumberQualifiers` (`NumberQualifiers`) | 3 | 3 | 0 | 0 | 0 | **100.0%** (3/3) |
| `TypeDescription` (`TypeDescription`) | 4 | 3 | 0 | 1 | 0 | **100.0%** (3/3) |
| `Picture` (`Picture`) | 2 | 1 | 1 | 0 | 0 | **100.0%** (2/2) |
| `StringQualifiers` (`StringQualifiers`) | 2 | 2 | 0 | 0 | 0 | **100.0%** (2/2) |
| `TextDocument` (`TextDocument`) | 6 | 2 | 0 | 4 | 0 | **100.0%** (2/2) |
| `ValueList` (`ValueList`) | 2 | 2 | 0 | 0 | 0 | **100.0%** (2/2) |
| `DateQualifiers` (`DateQualifiers`) | 1 | 1 | 0 | 0 | 0 | **100.0%** (1/1) |
| `Extension for controls located in a table box` (`Extension for controls located in a table box`) | 2 | 1 | 0 | 1 | 0 | **100.0%** (1/1) |
| `System enumeration text box extension` (`System enumeration text box extension`) | 1 | 1 | 0 | 0 | 0 | **100.0%** (1/1) |
| `BinaryDataQualifiers` (`BinaryDataQualifiers`) | 2 | 0 | 0 | 2 | 0 | 100% (Runtime) |
| `CellAppearance` (`CellAppearance`) | 21 | 0 | 0 | 21 | 0 | 100% (Runtime) |
| `CommandBarButtons` (`CommandBarButtons`) | 1 | 0 | 0 | 1 | 0 | 100% (Runtime) |
| `Controls` (`Controls`) | 1 | 0 | 0 | 1 | 0 | 100% (Runtime) |
| `DateAppearance` (`DateAppearance`) | 6 | 0 | 0 | 6 | 0 | 100% (Runtime) |
| `GeographicalSchemaField` (`GeographicalSchemaField`) | 5 | 0 | 0 | 5 | 0 | 100% (Runtime) |
| `GeographicalSchemaShowedArea` (`GeographicalSchemaShowedArea`) | 4 | 0 | 0 | 4 | 0 | 100% (Runtime) |
| `PeriodAppearance` (`PeriodAppearance`) | 7 | 0 | 0 | 7 | 0 | 100% (Runtime) |
| `RowAppearance` (`RowAppearance`) | 5 | 0 | 0 | 5 | 0 | 100% (Runtime) |
| `TableBoxColumns` (`TableBoxColumns`) | 1 | 0 | 0 | 1 | 0 | 100% (Runtime) |
| `ValueTable` (`ValueTable`) | 2 | 0 | 0 | 2 | 0 | 100% (Runtime) |
| `ValueTree` (`ValueTree`) | 2 | 0 | 0 | 2 | 0 | 100% (Runtime) |

## 3. Взаимозависимые и несовместимые свойства (подтверждено в 1С 8.3.27)

| Элемент / Группа | Свойства | Поведение в 1С и обработка в `oneform` |
|---|---|---|
| **`Form` (`Форма`)** | `Width` (`Ширина`) и `Height` (`Высота`) | Хранятся одновременно в `root[1][3..=4]`, дублируются в `root[1][11..=12]` и определяют правые (`edge = 3`) и нижние (`edge = 4`) границы страниц корневой панели `root[1][2][1][1]`. При изменении `<Width>`/`<Height>` формы синхронно обновляются все три структуры и пересчитываются `DerivedPage` корневых страниц (`panel_id = 0`). |
| **`Button` + `Form`** | `DefaultButton` (`КнопкаПоУмолчанию`) | Двойная привязка: флаг `info[1][5] = 1` на самой кнопке **и** ID кнопки в заголовке формы `root[1][1][2]`. При установке `<DefaultButton>true</DefaultButton>` синхронно обновляются оба слота. |
| **`InputField` (`ПолеВвода`)** | `MultiLine`, `PasswordMode`, `ExtendedEdit`, `Mask` | Дублируются в строковом расширении `9a7643d2-19e9-45e2-8893-280bc9195a97` (`info[3]`) и в базовом блоке `info[2][0][26, 5, 24, 21]`. Записываются синхронно в оба места; `info[3]` коалесцируется в один `Piece`. |
| **`InputField` (`ПолеВвода`)** | `TextEdit` (`РедактированиеТекста`) | Хранится инвертированно (`0 = Да`, `1 = Нет`) одновременно в `info[5]` и `info[2][0][12]`. Патчится и проверяется в обоих слотах. |
| **`Table` (`ТабличноеПоле`)** | `Header`, `Footer`, `VerticalLines`, `HorizontalLines`, `UseAlternationRowColor`, `ChoiceMode`, `ReadOnly` (инв.), `EnableStartDrag`, `AutoInsertNewRow`, `EnableDrag`, `ChangeSettingOfColumns`, `ChangePositionOfColumns` | Все 12 флагов упакованы в единую 32-битную маску `info[2][1][1]`. Коалесцируются в один атомарный `Piece`. |
| **`Table` (`ТабличноеПоле`)** | `FixedLeft` + `FixedRight` | В рантайме 1С `ФиксацияСправа` автоматически зажимается платформой до `Columns.Count - FixedLeft`. |
| **`TableColumn` (`КолонкаТабличногоПоля`)** | `ShowHierarchy` (0), `CheckBoxThreeState` (3), `ShowInHeader` (11), `ShowInFooter` (12), `MarkNegatives` (15), `ChangeVisible` (18, инв.), `AutoMarkIncomplete` (19), `AutoCellHeight` (20), `Hyperlink` (21), `ChangeSetting` (22), `ChangePosition` (23) | Все 11 флагов упакованы в 32-битную маску `col_props[9]`. Коалесцируются в один `Piece`. |
| **`TableColumn` (`КолонкаТабличногоПоля`)** | `ReadOnly` / `Location` (`col_props[26]`) | Слот `col_props[26]` хранит код положения (`0 = НоваяКолонка`, `1 = НаСледующейСтроке`, `2 = ВТойЖеЯчейке`) и совмещён с декодером `ReadOnly`. |
| **`Form` (`Форма`)** | `WindowState` (`root[6]`) и `Allow*State` (`root[1][5..=8]`) | При установке `WindowState = Float (2)` в рантайме 1С платформа автоматически включает `AllowDockedState`, `AllowFloatState`, `AllowAutohideState` и `AllowJoinWindow`. |

## 4. Детальная карта по каждому объекту

### `InputField` (`TextBox`) — 43 R/W, 0 R/O, 2 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `АвтоВыборНезаполненного` | `AutoChoiceIncomplete` | `AutoChoiceIncomplete` | `info[2][0][36]` | 🟢 R/W |
| `АвтоОтметкаНезаполненного` | `AutoMarkIncomplete` | `AutoMarkIncomplete` | `info[2][0][35]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ЦветФонаКнопки` | `ButtonBackColor` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ЦветТекстаКнопки` | `ButtonTextColor` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `КнопкаВыбора` | `ChoiceButton` | `ChoiceButton` | `info[2][0][7]` | 🟢 R/W |
| `КартинкаКнопкиВыбора` | `ChoiceButtonPicture` | `ChoiceButtonPicture` | `info[2][0][29]` | 🟢 R/W |
| `ВыборНезаполненного` | `ChoiceIncomplete` | `AutoChoiceIncomplete` | `info[2][0][36]` | 🟢 R/W |
| `КнопкаСпискаВыбора` | `ChoiceListButton` | `DropListButton` | `info[2][0][6]` | 🟢 R/W |
| `ВысотаСпискаВыбора` | `ChoiceListHeight` | `ChoiceListHeight` | `info[2][0][31]` | 🟢 R/W |
| `ШиринаСпискаВыбора` | `ChoiceListWidth` | `ChoiceListWidth` | `info[2][0][32]` | 🟢 R/W |
| `ВыбиратьТип` | `ChooseType` | `ChooseType` | `info[6]` | 🟢 R/W |
| `КнопкаОчистки` | `ClearButton` | `ClearButton` | `info[2][0][8]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `РасширенноеРедактирование` | `ExtendedEdit` | `ExtendedEdit` | `info[3] 9a7643d2[6] + info[2][0][24]` | 🟢 R/W |
| `ЦветФонаПоля` | `FieldBackColor` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `ЦветТекстаПоля` | `FieldTextColor` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `Формат` | `Format` | `Format` | `info[2][0][34]` | 🟢 R/W |
| `ГоризонтальноеПоложение` | `HorizontalAlign` | `HorizontalAlign` | `info[2][0][17]` | 🟢 R/W |
| `РежимВыбораНезаполненного` | `IncompleteChoiceMode` | `AutoChoiceIncomplete` | `info[2][0][36]` | 🟢 R/W |
| `РежимВыбораИзСписка` | `ListChoiceMode` | `ChoiceListMode` | `info[9]` | 🟢 R/W |
| `ОтметкаНезаполненного` | `MarkIncomplete` | `AutoMarkIncomplete` | `info[2][0][35]` | 🟢 R/W |
| `ВыделятьОтрицательные` | `MarkNegatives` | `MarkNegatives` | `info[2][0][27]` | 🟢 R/W |
| `Маска` | `Mask` | `Mask` | `info[3] 9a7643d2[4] + info[2][0][21]` | 🟢 R/W |
| `МаксимальноеЗначение` | `MaxValue` | `MaxValue` | `info[2][0][20]` | 🟢 R/W |
| `МинимальноеЗначение` | `MinValue` | `MinValue` | `info[2][0][19]` | 🟢 R/W |
| `МногострочныйРежим` | `MultiLine` | `MultiLine` | `info[3] 9a7643d2[3] + info[2][0][26]` | 🟢 R/W |
| `КнопкаОткрытия` | `OpenButton` | `OpenButton` | `info[2][0][10]` | 🟢 R/W |
| `РежимПароля` | `PasswordMode` | `PasswordMode` | `info[3] 9a7643d2[5] + info[2][0][5]` | 🟢 R/W |
| `Картинка` | `Picture` | `Picture` | `info[2][0][28]` | 🟢 R/W |
| `ТолькоПросмотр` | `ReadOnly` | `ReadOnly` | `info[2][0][13]` | 🟢 R/W |
| `КартинкаКнопкиВыбора` | `SelButtonPicture` | `ChoiceButtonPicture` | `info[2][0][29]` | 🟢 R/W |
| `СочетаниеКлавиш` | `Shortcut` | `Shortcut` | `info[2][0][33]` | 🟢 R/W |
| `КнопкаРегулирования` | `SpinButton` | `SpinButton` | `info[2][0][9]` | 🟢 R/W |
| `РедактированиеТекста` | `TextEdit` | `TextEdit` | `info[5] + info[2][0][12]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `ЭлементСвязиПоТипу` | `TypeLinkItem` | `TypeLink` | `info[8]` | 🟢 R/W |
| `ОграничениеТипа` | `TypeRestriction` | `ValueType` | `info[1]` | 🟢 R/W |
| `ВертикальноеПоложение` | `VerticalAlign` | `VerticalAlign` | `info[2][0][18]` | 🟢 R/W |
| `АвтоПереносСтрок` | `Wrap` | `AutoCellHeight` | `info[2][0][4]` | 🟢 R/W |
| `СписокВыбора` | `ChoiceList` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВыделенныйТекст` | `SelectedText` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `TableColumn` (`TableBoxColumn`) — 42 R/W, 0 R/O, 2 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `АвтоВысотаЯчейки` | `AutoCellHeight` | `AutoCellHeight` | `col_props[9] bit 20` | 🟢 R/W |
| `АвтоОтметкаНезаполненного` | `AutoMarkIncomplete` | `AutoMarkIncomplete` | `col_props[9] bit 19` | 🟢 R/W |
| `ВысотаЯчейки` | `CellHeight` | `CellHeight` | `col_props[37]` | 🟢 R/W |
| `ИзменятьПозицию` | `ChangePosition` | `ChangePosition` | `col_props[9] bit 23` | 🟢 R/W |
| `ИзменятьНастройку` | `ChangeSetting` | `ChangeSetting` | `col_props[9] bit 22` | 🟢 R/W |
| `ИзменятьВидимость` | `ChangeVisible` | `ChangeVisible` | `col_props[9] bit 18 (inv)` | 🟢 R/W |
| `ТриСостоянияФлажка` | `CheckBoxThreeState` | `CheckBoxThreeState` | `col_props[9] bit 3` | 🟢 R/W |
| `ДанныеФлажка` | `CheckData` | `CheckData` | `col_props[7]` | 🟢 R/W |
| `Данные` | `Data` | `DataPath` | `wrapper[2]` | 🟢 R/W |
| `РежимРедактирования` | `EditMode` | `EditMode` | `col_props[36]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ЦветФонаПоля` | `FieldBackColor` | `FieldBackColor` | `col_props[16]` | 🟢 R/W |
| `ЦветТекстаПоля` | `FieldTextColor` | `FieldTextColor` | `col_props[17]` | 🟢 R/W |
| `ЦветФонаПодвала` | `FooterBackColor` | `FooterBackColor` | `col_props[20]` | 🟢 R/W |
| `ШрифтПодвала` | `FooterFont` | `FooterFont` | `col_props[24]` | 🟢 R/W |
| `ГоризонтальноеПоложениеВПодвале` | `FooterHorizontalAlign` | `FooterHorizontalAlign` | `col_props[29]` | 🟢 R/W |
| `КартинкаПодвала` | `FooterPicture` | `FooterPicture` | `col_props[12]` | 🟢 R/W |
| `ТекстПодвала` | `FooterText` | `FooterText` | `col_props[2]` | 🟢 R/W |
| `ЦветТекстаПодвала` | `FooterTextColor` | `FooterTextColor` | `col_props[21]` | 🟢 R/W |
| `Формат` | `Format` | `Format` | `col_props[34]` | 🟢 R/W |
| `ЦветФонаШапки` | `HeaderBackColor` | `HeaderBackColor` | `col_props[18]` | 🟢 R/W |
| `ШрифтШапки` | `HeaderFont` | `HeaderFont` | `col_props[23]` | 🟢 R/W |
| `ГоризонтальноеПоложениеВШапке` | `HeaderHorizontalAlign` | `HeaderHorizontalAlign` | `col_props[27]` | 🟢 R/W |
| `КартинкаШапки` | `HeaderPicture` | `HeaderPicture` | `col_props[11]` | 🟢 R/W |
| `ТекстШапки` | `HeaderText` | `Header` | `col_props[1]` | 🟢 R/W |
| `ЦветТекстаШапки` | `HeaderTextColor` | `HeaderTextColor` | `col_props[19]` | 🟢 R/W |
| `ПодсказкаВШапке` | `HeaderToolTip` | `Tooltip` | `col_props[3]` | 🟢 R/W |
| `ГоризонтальноеПоложениеВКолонке` | `HorizontalAlignInColumn` | `HorizontalAlignInColumn` | `col_props[28]` | 🟢 R/W |
| `Гиперссылка` | `Hyperlink` | `Hyperlink` | `col_props[9] bit 21` | 🟢 R/W |
| `Положение` | `Location` | `Location` | `col_props[26]` | 🟢 R/W |
| `ВыделятьОтрицательные` | `MarkNegatives` | `MarkNegatives` | `col_props[9] bit 15` | 🟢 R/W |
| `Имя` | `Name` | `name` | `col_props[30]` | 🟢 R/W |
| `ТолькоПросмотр` | `ReadOnly` | `ReadOnly` | `col_props[26]` | 🟢 R/W |
| `КартинкиСтрок` | `RowsPictures` | `RowsPictures` | `col_props[10]` | 🟢 R/W |
| `ОтображатьИерархию` | `ShowHierarchy` | `ShowHierarchy` | `col_props[9] bit 0` | 🟢 R/W |
| `ОтображатьВПодвале` | `ShowInFooter` | `ShowInFooter` | `col_props[9] bit 12` | 🟢 R/W |
| `ОтображатьВШапке` | `ShowInHeader` | `ShowInHeader` | `col_props[9] bit 11` | 🟢 R/W |
| `ИзменениеРазмера` | `SizeChange` | `SizeChange` | `col_props[33]` | 🟢 R/W |
| `ПропускатьПриВводе` | `SkipOnInput` | `SkipOnInput` | `meta[4]` | 🟢 R/W |
| `ШрифтТекста` | `TextFont` | `TextFont` | `col_props[22]` | 🟢 R/W |
| `Видимость` | `Visible` | `Visible` | `col_props[25]` | 🟢 R/W |
| `Ширина` | `Width` | `Width` | `col_props[4]` | 🟢 R/W |
| `ЭлементУправления` | `Control` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ДополнительнаяКартинкаШапки` | `HeaderAdditionalPicture` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Table` (`TableBox`) — 40 R/W, 0 R/O, 6 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФонаЧередованияСтрок` | `AlternationRowBackColor` | `AlternationRowBackColor` | `info[2][1][8]` | 🟢 R/W |
| `АвтоВводНовойСтроки` | `AutoInsertNewRow` | `AutoInsertNewRow` | `info[2][1][1] bit 15` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ИзменятьПозициюКолонок` | `ChangePositionOfColumns` | `ChangePositionOfColumns` | `info[2][1][1] bit 26` | 🟢 R/W |
| `ИзменятьСоставСтрок` | `ChangeRowSet` | `ChangeRowSet` | `info[2][1][22]` | 🟢 R/W |
| `ИзменятьНастройкуКолонок` | `ChangeSettingOfColumns` | `ChangeSettingOfColumns` | `info[2][1][1] bit 25` | 🟢 R/W |
| `РежимВыбора` | `ChoiceMode` | `ChoiceMode` | `info[2][1][1] bit 9` | 🟢 R/W |
| `РазрешитьПеретаскивание` | `EnableDrag` | `EnableDrag` | `info[2][1][1] bit 20` | 🟢 R/W |
| `РазрешитьНачалоПеретаскивания` | `EnableStartDrag` | `EnableStartDrag` | `info[2][1][1] bit 14` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ЦветФонаПоля` | `FieldBackColor` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `ФиксацияСлева` | `FixedLeft` | `FixedLeft` | `info[2][1][11]` | 🟢 R/W |
| `ФиксацияСправа` | `FixedRight` | `FixedRight` | `info[2][1][12]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `Подвал` | `Footer` | `Footer` | `info[2][1][1] bit 1` | 🟢 R/W |
| `ЦветФонаПодвала` | `FooterBackColor` | `FooterBackColor` | `info[2][1][4]` | 🟢 R/W |
| `ШрифтПодвала` | `FooterFont` | `FooterFont` | `info[2][1][19]` | 🟢 R/W |
| `ВысотаПодвала` | `FooterHeight` | `FooterHeight` | `info[2][1][17]` | 🟢 R/W |
| `ЦветТекстаПодвала` | `FooterTextColor` | `FooterTextColor` | `info[2][1][5]` | 🟢 R/W |
| `Шапка` | `Header` | `Header` | `info[2][1][1] bit 0` | 🟢 R/W |
| `ЦветФонаШапки` | `HeaderBackColor` | `HeaderBackColor` | `info[2][1][2]` | 🟢 R/W |
| `ШрифтШапки` | `HeaderFont` | `HeaderFont` | `info[2][1][18]` | 🟢 R/W |
| `ВысотаШапки` | `HeaderHeight` | `HeaderHeight` | `info[2][1][16]` | 🟢 R/W |
| `ЦветТекстаШапки` | `HeaderTextColor` | `HeaderTextColor` | `info[2][1][3]` | 🟢 R/W |
| `ГоризонтальныеЛинии` | `HorizontalLines` | `HorizontalLines` | `info[2][1][1] bit 3` | 🟢 R/W |
| `ГоризонтальнаяПолосаПрокрутки` | `HorizontalScrollBar` | `HorizontalScrollBar` | `info[2][1][9]` | 🟢 R/W |
| `НачальноеОтображениеСписка` | `InitialListView` | `InitialListView` | `info[2][1][20]` | 🟢 R/W |
| `НачальноеОтображениеДерева` | `InitialTreeView` | `InitialTreeView` | `info[2][1][21]` | 🟢 R/W |
| `Вывод` | `Output` | `Output` | `info[2][1][24]` | 🟢 R/W |
| `ТолькоПросмотр` | `ReadOnly` | `ReadOnly` | `info[2][1][1] bit 10` | 🟢 R/W |
| `РежимВводаСтрок` | `RowInputMode` | `RowInputMode` | `info[2][1][13]` | 🟢 R/W |
| `РежимВыделенияСтроки` | `RowSelectionMode` | `RowSelectionMode` | `info[2][1][14]` | 🟢 R/W |
| `ЦветФонаВыделения` | `SelectionBackColor` | `SelectionBackColor` | `info[2][1][6]` | 🟢 R/W |
| `РежимВыделения` | `SelectionMode` | `SelectionMode` | `info[2][1][15]` | 🟢 R/W |
| `ЦветТекстаВыделения` | `SelectionTextColor` | `SelectionTextColor` | `info[2][1][7]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ЧередованиеЦветовСтрок` | `UseAlternationRowColor` | `UseAlternationRowColor` | `info[2][1][1] bit 4` | 🟢 R/W |
| `ВертикальныеЛинии` | `VerticalLines` | `VerticalLines` | `info[2][1][1] bit 2` | 🟢 R/W |
| `ВертикальнаяПолосаПрокрутки` | `VerticalScrollBar` | `VerticalScrollBar` | `info[2][1][10]` | 🟢 R/W |
| `Колонки` | `Columns` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ТекущаяКолонка` | `CurrentColumn` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ТекущиеДанные` | `CurrentData` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ТекущийРодитель` | `CurrentParent` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ТекущаяСтрока` | `CurrentRow` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВыделенныеСтроки` | `SelectedRows` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `SpreadsheetDocument` (`SpreadsheetDocument`) — 27 R/W, 0 R/O, 26 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЧерноБелаяПечать` | `BlackAndWhite` | `BlackAndWhite` | `info[14][5]` | 🟢 R/W |
| `ЧерноБелыйПросмотр` | `BlackAndWhiteView` | `BlackAndWhiteView` | `info[21]` | 🟢 R/W |
| `ПолеСнизу` | `BottomMargin` | `BottomMargin` | `info[14][10]` | 🟢 R/W |
| `РазборПоКопиям` | `Collate` | `Collate` | `info[14][6]` | 🟢 R/W |
| `КоличествоЭкземпляров` | `Copies` | `Copies` | `info[14][7]` | 🟢 R/W |
| `ДвусторонняяПечать` | `DuplexPrinting` | `DuplexPrinting` | `info[14][16]` | 🟢 R/W |
| `АвтоМасштаб` | `FitToPage` | `FitToPage` | `info[14][4]` | 🟢 R/W |
| `ФиксированныйФон` | `FixedBackground` | `FixedBackground` | `info[14][20]` | 🟢 R/W |
| `РазмерКолонтитулаСнизу` | `FooterSize` | `FooterSize` | `info[14][15]` | 🟢 R/W |
| `РазмерКолонтитулаСверху` | `HeaderSize` | `HeaderSize` | `info[14][14]` | 🟢 R/W |
| `ПолеСлева` | `LeftMargin` | `LeftMargin` | `info[14][12]` | 🟢 R/W |
| `Вывод` | `Output` | `Output` | `info[28]` | 🟢 R/W |
| `ОриентацияСтраницы` | `PageOrientation` | `PageOrientation` | `info[14][9]` | 🟢 R/W |
| `ЭкземпляровНаСтранице` | `PerPage` | `PerPage` | `info[14][8]` | 🟢 R/W |
| `ТочностьПечати` | `PrintAccuracy` | `PrintAccuracy` | `info[14][17]` | 🟢 R/W |
| `МасштабПечати` | `PrintScale` | `PrintScale` | `info[14][3]` | 🟢 R/W |
| `Защита` | `Protection` | `Protection` | `info[13]` | 🟢 R/W |
| `ТолькоПросмотр` | `ReadOnly` | `ReadOnly` | `info[12]` | 🟢 R/W |
| `ПолеСправа` | `RightMargin` | `RightMargin` | `info[14][13]` | 🟢 R/W |
| `СохранятьСвойстваОтображения` | `SaveViewProperties` | `SaveViewProperties` | `info[14][19]` | 🟢 R/W |
| `ОтображатьСетку` | `ShowGrid` | `ShowGrid` | `info[18]` | 🟢 R/W |
| `ОтображатьГруппировки` | `ShowGroups` | `ShowGroups` | `info[20]` | 🟢 R/W |
| `ОтображатьЗаголовки` | `ShowHeaders` | `ShowHeaders` | `info[19]` | 🟢 R/W |
| `НаправлениеПерехода` | `StepDirection` | `StepDirection` | `info[14][18]` | 🟢 R/W |
| `ПолеСверху` | `TopMargin` | `TopMargin` | `info[14][11]` | 🟢 R/W |
| `ИтогиСнизу` | `TotalsBelow` | `TotalsBelow` | `info[23]` | 🟢 R/W |
| `ИтогиСправа` | `TotalsRight` | `TotalsRight` | `info[22]` | 🟢 R/W |
| `Области` | `Areas` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ФоноваяКартинка` | `BackgroundPicture` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ТекущаяОбласть` | `CurrentArea` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Рисунки` | `Drawings` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВстроенныеТаблицы` | `EmbeddedTables` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ФиксацияСлева` | `FixedLeft` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ФиксацияСверху` | `FixedTop` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `НижнийКолонтитул` | `Footer` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВерхнийКолонтитул` | `Header` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `КодЯзыка` | `LanguageCode` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВысотаСтраницы` | `PageHeight` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЧередованиеРасположенияСтраниц` | `PagePlacementAlternation` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `РазмерСтраницы` | `PageSize` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ШиринаСтраницы` | `PageWidth` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Параметры` | `Parameters` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ОбластьПечати` | `PrintArea` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `КлючПараметровПечати` | `PrintParametersKey` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ИмяПринтера` | `PrinterName` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ПовторятьПриПечатиКолонки` | `RepeatOnColumnPrint` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ПовторятьПриПечатиСтроки` | `RepeatOnRowPrint` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВыделенныеОбласти` | `SelectedAreas` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВысотаТаблицы` | `TableHeight` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ШиринаТаблицы` | `TableWidth` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Макет` | `Template` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ИспользуемоеИмяФайла` | `UsedFileName` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `КлючСохраненияПоложенияОкна` | `WindowOptionsKey` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `ChoiceField` (`ComboBox`) — 21 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ЦветФонаКнопки` | `ButtonBackColor` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ЦветТекстаКнопки` | `ButtonTextColor` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `КнопкаВыбора` | `ChoiceButton` | `ChoiceButton` | `info[1][7]` | 🟢 R/W |
| `КартинкаКнопкиВыбора` | `ChoiceButtonPicture` | `ChoiceButtonPicture` | `info[1][29]` | 🟢 R/W |
| `СписокВыбора` | `ChoiceList` | `ChoiceList` | `info[1][26]` | 🟢 R/W |
| `ВысотаСпискаВыбора` | `ChoiceListHeight` | `ChoiceListHeight` | `info[1][31]` | 🟢 R/W |
| `ШиринаСпискаВыбора` | `ChoiceListWidth` | `ChoiceListWidth` | `info[1][32]` | 🟢 R/W |
| `КнопкаОчистки` | `ClearButton` | `ClearButton` | `info[1][8]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ЦветФонаПоля` | `FieldBackColor` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `ЦветТекстаПоля` | `FieldTextColor` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `КнопкаОткрытия` | `OpenButton` | `OpenButton` | `info[1][10]` | 🟢 R/W |
| `Картинка` | `Picture` | `Picture` | `info[1][28]` | 🟢 R/W |
| `ТолькоПросмотр` | `ReadOnly` | `ReadOnly` | `info[1][13]` | 🟢 R/W |
| `КартинкаКнопкиВыбора` | `SelButtonPicture` | `ChoiceButtonPicture` | `info[1][29]` | 🟢 R/W |
| `КнопкаРегулирования` | `SpinButton` | `SpinButton` | `info[1][9]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |

### `Form` (`Form`) — 21 R/W, 0 R/O, 17 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `РазрешитьСостояниеПрячущееся` | `AllowAutohideState` | `AllowAutohideState` | `root[1][7]` | 🟢 R/W |
| `РазрешитьЗакрытие` | `AllowClose` | `AllowClose` | `root[18]` | 🟢 R/W |
| `РазрешитьСостояниеПрикрепленное` | `AllowDockedState` | `AllowDockedState` | `root[1][6]` | 🟢 R/W |
| `РазрешитьСостояниеСвободное` | `AllowFloatState` | `AllowFloatState` | `root[1][8]` | 🟢 R/W |
| `РазрешитьСоединятьОкно` | `AllowJoinWindow` | `AllowJoinWindow` | `root[10]` | 🟢 R/W |
| `РазрешитьСостояниеОбычное` | `AllowNormalState` | `AllowNormalState` | `root[1][5]` | 🟢 R/W |
| `АвтоЗаголовок` | `AutoTitle` | `AutoTitle` | `root[5]` | 🟢 R/W |
| `Заголовок` | `Caption` | `Caption` | `root[1][1][0]` | 🟢 R/W |
| `КартинкаЗаголовка` | `CaptionPicture` | `CaptionPicture` | `root[13][2]` | 🟢 R/W |
| `ИзменятьСпособОтображенияОкна` | `ChangeWindowAppearanceMode` | `ChangeWindowAppearanceMode` | `root[16]` | 🟢 R/W |
| `СоединяемоеОкно` | `ConnectableWindow` | `ConnectableWindow` | `root[9]` | 🟢 R/W |
| `РежимРабочегоСтола` | `DesktopMode` | `DesktopMode` | `root[17]` | 🟢 R/W |
| `ПоведениеКлавишиEnter` | `EnterKeyBehavior` | `EnterKeyBehavior` | `root[19]` | 🟢 R/W |
| `Высота` | `Height` | `Height` | `root[1][4], root[1][12], root[1][2][1]` | 🟢 R/W |
| `ИзменениеРазмера` | `SizeChange` | `SizeChange` | `root[8]` | 🟢 R/W |
| `Стиль` | `Style` | `Style` | `root[12]` | 🟢 R/W |
| `Ширина` | `Width` | `Width` | `root[1][3], root[1][11], root[1][2][1]` | 🟢 R/W |
| `СпособОтображенияОкна` | `WindowAppearanceMode` | `WindowAppearanceMode` | `root[14]` | 🟢 R/W |
| `ПоложениеПрикрепленногоОкна` | `WindowDockLocation` | `WindowDockLocation` | `root[1][9]` | 🟢 R/W |
| `ПоложениеОкна` | `WindowLocation` | `WindowLocation` | `root[15]` | 🟢 R/W |
| `СостояниеОкна` | `WindowState` | `WindowState` | `root[6]` | 🟢 R/W |
| `<Имя реквизита>` | `<Attribute name>` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `НачальноеЗначениеВыбора` | `ChoiceInitialValue` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `РежимВыбора` | `ChoiceMode` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЗакрыватьПриВыборе` | `CloseOnChoice` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЗакрыватьПриЗакрытииВладельца` | `CloseOnOwnerClose` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЭлементыФормы` | `Controls` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ТекущийЭлемент` | `CurrentControl` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВладелецФормы` | `FormOwner` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `МодальныйРежим` | `ModalMode` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Модифицированность` | `Modified` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `МножественныйВыбор` | `MultipleChoice` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Панель` | `Panel` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ТолькоПросмотр` | `ReadOnly` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЭтаФорма` | `ThisForm` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `КлючУникальности` | `UniqueKey` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `КлючСохраненияПоложенияОкна` | `WindowOptionsKey` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ИмяСохраненияПоложенияОкна` | `WindowOptionsName` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `CalendarField` (`CalendarBox`) — 19 R/W, 0 R/O, 2 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `НачалоПериодаОтображения` | `BeginOfDisplayPeriod` | `BeginOfDisplayPeriod` | `info[1][5]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ЦветФонаКнопки` | `ButtonBackColor` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ЦветТекстаКнопки` | `ButtonTextColor` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `ПеремещениеПоКалендарю` | `CalendarNavigation` | `CalendarNavigation` | `info[1][8]` | 🟢 R/W |
| `РазрешитьПеретаскивание` | `EnableDrag` | `EnableDrag` | `info[1][11]` | 🟢 R/W |
| `РазрешитьНачалоПеретаскивания` | `EnableStartDrag` | `EnableStartDrag` | `info[1][10]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `КонецПериодаОтображения` | `EndOfDisplayPeriod` | `EndOfDisplayPeriod` | `info[1][6]` | 🟢 R/W |
| `ЦветФонаПоля` | `FieldBackColor` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `ЦветТекстаПоля` | `FieldTextColor` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `ЦветВыходныхДней` | `HolidayColor` | `HolidayColor` | `info[1][4]` | 🟢 R/W |
| `ЦветФонаВыделения` | `SelectionBackColor` | `SelectionBackColor` | `info[1][2]` | 🟢 R/W |
| `РежимВыделения` | `SelectionMode` | `SelectionMode` | `info[1][9]` | 🟢 R/W |
| `ЦветТекстаВыделения` | `SelectionTextColor` | `SelectionTextColor` | `info[1][3]` | 🟢 R/W |
| `ОтображатьТекущуюДату` | `ShowCurrentDate` | `ShowCurrentDate` | `info[1][7]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ТекущаяДата` | `CurrentDate` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВыделенныеДаты` | `SelectedDates` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `LabelDecoration` (`Label`) — 19 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Заголовок` | `Caption` | `Caption` | `info[1][2]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `Формат` | `Format` | `Format` | `info[1][10]` | 🟢 R/W |
| `ГоризонтальноеПоложение` | `HorizontalAlign` | `HorizontalAlign` | `info[1][3]` | 🟢 R/W |
| `Гиперссылка` | `Hyperlink` | `Hyperlink` | `info[1][5]` | 🟢 R/W |
| `ПоложениеКартинкиНадписи` | `LabelPictureLocation` | `LabelPictureLocation` | `info[1][12][6]` | 🟢 R/W |
| `ВыделятьОтрицательные` | `MarkNegatives` | `MarkNegatives` | `info[1][6]` | 🟢 R/W |
| `Картинка` | `Picture` | `Picture` | `info[1][12][2]` | 🟢 R/W |
| `РазмерКартинки` | `PictureSize` | `PictureSize` | `info[1][13]` | 🟢 R/W |
| `БегущаяСтрока` | `ScrollingText` | `ScrollingText` | `info[1][9]` | 🟢 R/W |
| `СочетаниеКлавиш` | `Shortcut` | `Shortcut` | `info[1][8]` | 🟢 R/W |
| `ЦветТекста` | `TextColor` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `ВертикальноеПоложение` | `VerticalAlign` | `VerticalAlign` | `info[1][4]` | 🟢 R/W |

### `Panel` (`Panel`) — 17 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `АвтоПорядокОбхода` | `AutoTabOrder` | `AutoTabOrder` | `info[1][base+5]` | 🟢 R/W |
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `Страницы` | `Pages` | `Pages` | `info[1][base+3]` | 🟢 R/W |
| `ПоложениеКартинкиПанели` | `PanelPictureLocation` | `PanelPictureLocation` | `info[1][base][7]` | 🟢 R/W |
| `Картинка` | `Picture` | `Picture` | `info[1][base][2]` | 🟢 R/W |
| `РазмерКартинки` | `PictureSize` | `PictureSize` | `info[1][base][6]` | 🟢 R/W |
| `РежимПрокручиваемыхСтраниц` | `ScrollPageMode` | `ScrollPageMode` | `info[1][base+6]` | 🟢 R/W |
| `РаспределятьПоСтраницам` | `SeparateByPage` | `SeparateByPage` | `info[1][base+4]` | 🟢 R/W |
| `ОтображениеЗакладок` | `ShowTabs` | `ShowTabs` | `info[1][base+1]` | 🟢 R/W |
| `ПорядокОбхода` | `TabOrder` | `TabOrder` | `geom[22]` | 🟢 R/W |
| `ЦветТекста` | `TextColor` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `ТекущаяСтраница` | `CurrentPage` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `PictureField` (`PictureBox`) — 17 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `РазрешитьПеретаскивание` | `EnableDrag` | `EnableDrag` | `info[1][9]` | 🟢 R/W |
| `РазрешитьНачалоПеретаскивания` | `EnableStartDrag` | `EnableStartDrag` | `info[1][8]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `Гиперссылка` | `Hyperlink` | `Hyperlink` | `info[1][3]` | 🟢 R/W |
| `ТекстНевыбраннойКартинки` | `NonselectedPictureText` | `NonselectedPictureText` | `info[1][10]` | 🟢 R/W |
| `Картинка` | `Picture` | `Picture` | `info[1][4][2]` | 🟢 R/W |
| `РазмерКартинки` | `PictureSize` | `PictureSize` | `info[1][2]` | 🟢 R/W |
| `СочетаниеКлавиш` | `Shortcut` | `Shortcut` | `info[1][5]` | 🟢 R/W |
| `ЦветТекста` | `TextColor` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `ИспользоватьКонтекстноеМеню` | `UseContextMenu` | `UseContextMenu` | `info[1][7]` | 🟢 R/W |
| `Масштабировать` | `Zoomable` | `Zoomable` | `info[1][6]` | 🟢 R/W |

### `Button` (`Button`) — 15 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ЦветФонаКнопки` | `ButtonBackColor` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ЦветТекстаКнопки` | `ButtonTextColor` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `Заголовок` | `Caption` | `Caption` | `info[1][2]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `ГоризонтальноеПоложение` | `HorizontalAlign` | `HorizontalAlign` | `info[1][3]` | 🟢 R/W |
| `РежимМеню` | `MenuMode` | `MenuMode` | `info[1][5]` | 🟢 R/W |
| `МногострочныйРежим` | `MultiLine` | `MultiLine` | `info[1][10]` | 🟢 R/W |
| `Картинка` | `Picture` | `Picture` | `info[1][8]` | 🟢 R/W |
| `ПоложениеКартинки` | `PictureLocation` | `PictureLocation` | `info[1][6]` | 🟢 R/W |
| `РазмерКартинки` | `PictureSize` | `PictureSize` | `info[1][7]` | 🟢 R/W |
| `СочетаниеКлавиш` | `Shortcut` | `Shortcut` | `info[1][9]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ВертикальноеПоложение` | `VerticalAlign` | `VerticalAlign` | `info[1][4]` | 🟢 R/W |
| `Кнопки` | `Buttons` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `CheckBox` (`CheckBox`) — 15 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Заголовок` | `Caption` | `Caption` | `info[1][0][2]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ЦветФонаПоля` | `FieldBackColor` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `ЦветТекстаПоля` | `FieldTextColor` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `ГоризонтальноеПоложение` | `HorizontalAlign` | `HorizontalAlign` | `info[1][0][4]` | 🟢 R/W |
| `ЦветТекста` | `TextColor` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `ТриСостояния` | `ThreeState` | `ThreeState` | `info[1][2]` | 🟢 R/W |
| `ПоложениеЗаголовка` | `TitleLocation` | `TitleLocation` | `info[1][0][3]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `ВертикальноеПоложение` | `VerticalAlign` | `VerticalAlign` | `info[1][0][5]` | 🟢 R/W |

### `CommandBarButton` (`CommandBarButton`) — 15 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Действие` | `Action` | `action` | `action[4][1]` | 🟢 R/W |
| `ПорядокКнопок` | `ButtonOrder` | `ButtonOrder` | `b[14]` | 🟢 R/W |
| `ТипКнопки` | `ButtonType` | `kind` | `b[9]` | 🟢 R/W |
| `Кнопки` | `Buttons` | `Buttons` | `X[7] group` | 🟢 R/W |
| `Пометка` | `Check` | `checked` | `b[12]` | 🟢 R/W |
| `КнопкаПоУмолчанию` | `DefaultButton` | `DefaultButton` | `b[13]` | 🟢 R/W |
| `Пояснение` | `Description` | `Description` | `action[4][2][4]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ИзменяетДанные` | `ModifiesData` | `ModifiesData` | `meta[3]` | 🟢 R/W |
| `Имя` | `Name` | `name` | `b[1]` | 🟢 R/W |
| `Картинка` | `Picture` | `Picture` | `action[4][2][5]` | 🟢 R/W |
| `Отображение` | `Representation` | `representation` | `b[10]` | 🟢 R/W |
| `СочетаниеКлавиш` | `Shortcut` | `Shortcut` | `action[4][2][6]` | 🟢 R/W |
| `Текст` | `Text` | `Caption` | `b[4]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `action[4][2][3]` | 🟢 R/W |

### `RadioButton` (`RadioButton`) — 15 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Заголовок` | `Caption` | `Caption` | `info[2][0][2]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ЦветФонаПоля` | `FieldBackColor` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `ЦветТекстаПоля` | `FieldTextColor` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `ГоризонтальноеПоложение` | `HorizontalAlign` | `HorizontalAlign` | `info[2][0][4]` | 🟢 R/W |
| `ВыбираемоеЗначение` | `SelectionValue` | `SelectionValue` | `info[4]` | 🟢 R/W |
| `ЦветТекста` | `TextColor` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `ПоложениеЗаголовка` | `TitleLocation` | `TitleLocation` | `info[2][0][3]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `ВертикальноеПоложение` | `VerticalAlign` | `VerticalAlign` | `info[2][0][5]` | 🟢 R/W |

### `TrackBar` (`TrackBar`) — 15 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ЦветФонаКнопки` | `ButtonBackColor` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `БольшойШаг` | `LargeStep` | `LargeStep` | `info[1][5]` | 🟢 R/W |
| `ОтображениеРазметки` | `MarkingAppearance` | `MarkingAppearance` | `info[1][7]` | 🟢 R/W |
| `ШагРазметки` | `MarkingStep` | `MarkingStep` | `info[1][8]` | 🟢 R/W |
| `МаксимальноеЗначение` | `MaxValue` | `MaxValue` | `info[1][3]` | 🟢 R/W |
| `МинимальноеЗначение` | `MinValue` | `MinValue` | `info[1][2]` | 🟢 R/W |
| `Ориентация` | `Orientation` | `Orientation` | `info[1][6]` | 🟢 R/W |
| `Шаг` | `Step` | `Step` | `info[1][4]` | 🟢 R/W |
| `ЦветТекста` | `TextColor` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |

### `CommandBar` (`CommandBar`) — 14 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `АвтоЗаполнение` | `AutoFill` | `AutoFill` | `info[1][3]` | 🟢 R/W |
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ЦветФонаКнопки` | `ButtonBackColor` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ЦветТекстаКнопки` | `ButtonTextColor` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `Кнопки` | `Buttons` | `Buttons` | `info[1][7]` | 🟢 R/W |
| `ВыравниваниеКнопок` | `ButtonsAlignment` | `HorizontalAlign` | `info[1][4]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `Ориентация` | `Orientation` | `Orientation` | `info[1][2]` | 🟢 R/W |
| `Вспомогательная` | `Secondary` | `Secondary` | `info[1][5]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |

### `ProgressBar` (`ProgressBar`) — 12 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `МаксимальноеЗначение` | `MaxValue` | `MaxValue` | `info[1][3]` | 🟢 R/W |
| `МинимальноеЗначение` | `MinValue` | `MinValue` | `info[1][2]` | 🟢 R/W |
| `Ориентация` | `Orientation` | `Orientation` | `info[1][7]` | 🟢 R/W |
| `ОтображатьПроценты` | `ShowPercent` | `ShowPercent` | `info[1][6]` | 🟢 R/W |
| `СтильОтображения` | `ShowStyle` | `ShowStyle` | `info[1][5]` | 🟢 R/W |
| `Шаг` | `Step` | `Step` | `info[1][4]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |

### `Extension for controls located in a form` (`Extension for controls located in a form`) — 10 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ИсточникДействий` | `ActionSource` | `ActionSource` | `meta[2]` | 🟢 R/W |
| `АвтоКонтекстноеМеню` | `AutoContextMenu` | `AutoContextMenu` | `<base>[16]` | 🟢 R/W |
| `КонтекстноеМеню` | `ContextMenu` | `ContextMenu` | `meta[2]` | 🟢 R/W |
| `Данные` | `Data` | `DataPath` | `root[2][3]` | 🟢 R/W |
| `КнопкаПоУмолчанию` | `DefaultButton` | `DefaultButton` | `info[1][5] + root[1][1][2]` | 🟢 R/W |
| `ПервыйВГруппе` | `FirstInGroup` | `FirstInGroup` | `meta[5]` | 🟢 R/W |
| `ИзменяетДанные` | `ModifiesData` | `ModifiesData` | `meta[3]` | 🟢 R/W |
| `Имя` | `Name` | `@name` | `meta[1]` | 🟢 R/W |
| `ПропускатьПриВводе` | `SkipOnInput` | `SkipOnInput` | `meta[4]` | 🟢 R/W |
| `ТипЗначения` | `ValueType` | `ValueType` | `info[1]` | 🟢 R/W |
| `Значение` | `Value` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `ListBox` (`ListBox`) — 9 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ЦветФонаПоля` | `FieldBackColor` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `ЦветТекстаПоля` | `FieldTextColor` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `ТолькоПросмотр` | `ReadOnly` | `ReadOnly` | `info[1][6]` | 🟢 R/W |
| `ОтображатьПометку` | `ShowCheckBox` | `ShowCheckBox` | `info[1][5]` | 🟢 R/W |
| `ОтображатьКартинку` | `ShowPicture` | `ShowPicture` | `info[1][4]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ТекущаяСтрока` | `CurrentRow` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Panel controls extension` (`Panel controls extension`) — 9 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Свертка` | `Collapse` | `Collapse` | `<base>[14]` | 🟢 R/W |
| `АктивизироватьПоУмолчанию` | `DefaultControl` | `DefaultControlId` | `root[1][2][1] tail[1]` | 🟢 R/W |
| `Высота` | `Height` | `Height` | `geom[4] - geom[2]` | 🟢 R/W |
| `Лево` | `Left` | `Left` | `geom[1]` | 🟢 R/W |
| `ПорядокОбхода` | `TabOrder` | `TabOrder` | `geom[22]` | 🟢 R/W |
| `Верх` | `Top` | `Top` | `geom[2]` | 🟢 R/W |
| `Видимость` | `Visible` | `Visible` | `geom[5]` | 🟢 R/W |
| `Ширина` | `Width` | `Width` | `geom[3] - geom[1]` | 🟢 R/W |
| `ПорядокОтображения` | `ZOrder` | `ZOrder` | `geom[21]` | 🟢 R/W |

### `UsualGroup` (`GroupBox`) — 9 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Заголовок` | `Caption` | `Caption` | `info[1][2]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `ЦветТекста` | `TextColor` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |

### `Font` (`Font`) — 8 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Жирный` | `Bold` | `Font@bold` | `{7,kind,mask,...}` | 🟢 R/W |
| `Наклонный` | `Italic` | `Font@italic` | `{7,1,60,...}` | 🟢 R/W |
| `Имя` | `Name` | `Font@face` | `{7,kind,mask,...}` | 🟢 R/W |
| `Масштаб` | `Scale` | `Font@scale` | `{7,1,60,...,scale}` | 🟢 R/W |
| `Размер` | `Size` | `Font@size` | `{7,kind,mask,...}` | 🟢 R/W |
| `Зачеркивание` | `Strikeout` | `Font@strikeout` | `{7,1,60,...}` | 🟢 R/W |
| `Вид` | `Type` | `Font@source` | `{7,kind,mask,...}` | 🟢 R/W |
| `Подчеркивание` | `Underline` | `Font@underline` | `{7,1,60,...}` | 🟢 R/W |

### `SpreadsheetDocumentField` (`SpreadsheetDocumentField`) — 8 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `РазрешитьПеретаскивание` | `EnableDrag` | `EnableDrag` | `info[26]` | 🟢 R/W |
| `РазрешитьНачалоПеретаскивания` | `EnableStartDrag` | `EnableStartDrag` | `info[25]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ГоризонтальнаяПолосаПрокрутки` | `HorizontalScrollBar` | `HorizontalScrollBar` | `info[31]` | 🟢 R/W |
| `ОтображатьВыделение` | `ShowSelection` | `ShowSelection` | `info[27]` | 🟢 R/W |
| `ВертикальнаяПолосаПрокрутки` | `VerticalScrollBar` | `VerticalScrollBar` | `info[32]` | 🟢 R/W |

### `Extension for controls located in a spreadsheet document field` (`Extension for controls located in a spreadsheet document field`) — 7 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ВстроенВЯчейку` | `EmbeddedIntoCell` | `EmbeddedIntoCell` | `<base>[13]` | 🟢 R/W |
| `Высота` | `Height` | `Height` | `geom[4] - geom[2]` | 🟢 R/W |
| `Лево` | `Left` | `Left` | `geom[1]` | 🟢 R/W |
| `ВыводитьНаПечать` | `Print` | `Print` | `<base>[17]` | 🟢 R/W |
| `Верх` | `Top` | `Top` | `geom[2]` | 🟢 R/W |
| `Видимость` | `Visible` | `Visible` | `geom[5]` | 🟢 R/W |
| `Ширина` | `Width` | `Width` | `geom[3] - geom[1]` | 🟢 R/W |
| `Значение` | `Value` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Splitter` (`Splitter`) — 6 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Ориентация` | `Orientation` | `Orientation` | `info[1][2]` | 🟢 R/W |
| `Подсказка` | `ToolTip` | `Tooltip` | `<base>[12]` | 🟢 R/W |

### `GraphicalSchemaField` (`GraphicalSchemaField`) — 5 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ТолькоПросмотр` | `ReadOnly` | `ReadOnly` | `info[4]` | 🟢 R/W |
| `ПрозрачныйФон` | `Transparent` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `ТекущийЭлемент` | `CurrentItem` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `PanelPage` (`PanelPage`) — 5 R/W, 0 R/O, 2 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Заголовок` | `Caption` | `Title` | `rec[1]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `rec[5]` | 🟢 R/W |
| `Имя` | `Name` | `name` | `rec[6]` | 🟢 R/W |
| `КартинкаЗаголовка` | `TitlePicture` | `TitlePicture` | `rec[2][2]` | 🟢 R/W |
| `Видимость` | `Visible` | `Visible` | `rec[4]` | 🟢 R/W |
| `Раскрыта` | `IsOpen` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Значение` | `Value` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Color` (`Color`) — 4 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Синий` | `B` | `*Color@value` | `{3,kind,{rgb}}` | 🟢 R/W |
| `Зеленый` | `G` | `*Color@value` | `{3,kind,{rgb}}` | 🟢 R/W |
| `Красный` | `R` | `*Color@value` | `{3,kind,{rgb}}` | 🟢 R/W |
| `Вид` | `Type` | `*Color@kind` | `{3,kind,{...}}` | 🟢 R/W |

### `Shortcut` (`Shortcut`) — 4 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Alt` | `Alt` | `Shortcut@alt` | `{key,mods,0} bit 2` | 🟢 R/W |
| `Ctrl` | `Ctrl` | `Shortcut@ctrl` | `{key,mods,0} bit 1` | 🟢 R/W |
| `Клавиша` | `Key` | `Shortcut@key` | `{key,mods,0}` | 🟢 R/W |
| `Shift` | `Shift` | `Shortcut@shift` | `{key,mods,0} bit 0` | 🟢 R/W |

### `TextDocumentField` (`TextDocumentField`) — 4 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Доступность` | `Enabled` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Шрифт` | `Font` | `Font` | `<base>[4]` | 🟢 R/W |
| `ВыделенныйТекст` | `SelectedText` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `ValueListItem` (`ValueListItem`) — 4 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Пометка` | `Check` | `ChoiceItem@checked` | `item[2]` | 🟢 R/W |
| `Картинка` | `Picture` | `ChoiceItem@pictureId` | `item[1]` | 🟢 R/W |
| `Представление` | `Presentation` | `ChoiceItem/Presentation` | `item[4]` | 🟢 R/W |
| `Значение` | `Value` | `ChoiceItem@value` | `item[3]` | 🟢 R/W |

### `Border` (`Border`) — 3 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ТипРамки` | `BorderType` | `BorderStyle@code` | `info[1][3][3]` | 🟢 R/W |
| `Вид` | `Type` | `BorderStyle` | `info[1][3][3]` | 🟢 R/W |
| `Толщина` | `Width` | `Width` | `geom[3] - geom[1]` | 🟢 R/W |

### `HTMLDocumentField` (`HTMLDocumentField`) — 3 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Рамка` | `Border` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `ЦветРамки` | `BorderColor` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Вывод` | `Output` | `Output` | `info[6]` | 🟢 R/W |
| `Документ` | `Document` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `NumberQualifiers` (`NumberQualifiers`) — 3 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ДопустимыйЗнак` | `AllowedSign` | `ValueType@nonNegative` | `{"N",d,f,sign}` | 🟢 R/W |
| `Разрядность` | `Digits` | `ValueType@digits` | `{"N",d,f,sign}` | 🟢 R/W |
| `РазрядностьДробнойЧасти` | `FractionDigits` | `ValueType@fractionDigits` | `{"N",d,f,sign}` | 🟢 R/W |

### `TypeDescription` (`TypeDescription`) — 3 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `КвалификаторыДаты` | `DateQualifiers` | `ValueType@kind` | `{"D","D"|"T"}` | 🟢 R/W |
| `КвалификаторыЧисла` | `NumberQualifiers` | `ValueType@digits,fractionDigits` | `{"N",d,f,sign}` | 🟢 R/W |
| `КвалификаторыСтроки` | `StringQualifiers` | `ValueType@length` | `{"S",len,fixed}` | 🟢 R/W |
| `КвалификаторыДвоичныхДанных` | `BinaryDataQualifiers` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Picture` (`Picture`) — 1 R/W, 1 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Вид` | `Type` | `Picture@kind` | `{4,kind,ref,...}` | 🟢 R/W |
| `НаборВариантов` | `VariantSet` | `Picture` | `{8,ver,pic1,pic2,pic3,...}` | 🟡 R/O |

### `StringQualifiers` (`StringQualifiers`) — 2 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ДопустимаяДлина` | `AllowedLength` | `ValueType@fixed` | `{"S",len,fixed}` | 🟢 R/W |
| `Длина` | `Length` | `ValueType@length` | `{"S",len,fixed}` | 🟢 R/W |

### `TextDocument` (`TextDocument`) — 2 R/W, 0 R/O, 4 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Вывод` | `Output` | `Output` | `info[6]` | 🟢 R/W |
| `ТолькоПросмотр` | `ReadOnly` | `ReadOnly` | `info[5]` | 🟢 R/W |
| `РазделительСтрок` | `LineSeparator` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Параметры` | `Parameters` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `КодЯзыкаМакета` | `TemplateLanguageCode` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ИспользуемоеИмяФайла` | `UsedFileName` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `ValueList` (`ValueList`) — 2 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ДоступныеЗначения` | `AvailableValues` | `ChoiceList` | `info[1][26][2][6]` | 🟢 R/W |
| `ТипЗначения` | `ValueType` | `ValueType` | `info[1]` | 🟢 R/W |

### `DateQualifiers` (`DateQualifiers`) — 1 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЧастиДаты` | `DateFractions` | `ValueType@kind` | `{"D","D"|"T"}` | 🟢 R/W |

### `Extension for controls located in a table box` (`Extension for controls located in a table box`) — 1 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ТипЗначения` | `ValueType` | `ValueType` | `info[1]` | 🟢 R/W |
| `Значение` | `Value` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `System enumeration text box extension` (`System enumeration text box extension`) — 1 R/W, 0 R/O, 0 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ДоступныеЗначения` | `AvailableValues` | `ChoiceList` | `info[1][26][2][6]` | 🟢 R/W |

### `BinaryDataQualifiers` (`BinaryDataQualifiers`) — 0 R/W, 0 R/O, 2 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ДопустимаяДлина` | `AllowedLength` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Длина` | `Length` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `CellAppearance` (`CellAppearance`) — 0 R/W, 0 R/O, 21 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Выравнивание` | `Align` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `АвтоВысотаЯчейки` | `AutoCellHeight` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЦветФона` | `BackColor` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ВысотаЯчейки` | `CellHeight` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Флажок` | `Check` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЗначениеФлажка` | `CheckValue` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Шрифт` | `Font` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Гиперссылка` | `Hyperlink` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ОтметкаНезаполненного` | `MarkIncomplete` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Имя` | `Name` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Картинка` | `Picture` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ИндексКартинки` | `PictureIndex` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЗначениеКартинки` | `PictureValue` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ТолькоПросмотр` | `ReadOnly` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ОтображатьФлажок` | `ShowCheckBox` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ОтображатьКартинку` | `ShowPicture` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ОтображатьТекст` | `ShowText` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Текст` | `Text` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЦветТекста` | `TextColor` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Значение` | `Value` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Видимость` | `Visible` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `CommandBarButtons` (`CommandBarButtons`) — 0 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `<Имя кнопки>` | `<Button name>` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Controls` (`Controls`) — 0 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `<Имя элемента управления>` | `<Control name>` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `DateAppearance` (`DateAppearance`) — 0 R/W, 0 R/O, 6 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Дата` | `Date` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Доступность` | `Enabled` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Шрифт` | `Font` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЦветТекста` | `TextColor` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Подсказка` | `ToolTip` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `GeographicalSchemaField` (`GeographicalSchemaField`) — 0 R/W, 0 R/O, 5 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `АктивныйОбъект` | `ActiveObject` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Масштаб` | `Scale` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ПоддержкаМасштаба` | `ScaleKeeping` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ОтображатьКоординаты` | `ShowCoordinates` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ОтображаемаяОбласть` | `ShowedArea` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `GeographicalSchemaShowedArea` (`GeographicalSchemaShowedArea`) — 0 R/W, 0 R/O, 4 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ДолготаВосточнойГраницы` | `EastBorderLongitude` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ШиротаСевернойГраницы` | `NorthBorderLatitude` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ШиротаЮжнойГраницы` | `SouthBorderLatitude` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ДолготаЗападнойГраницы` | `WestBorderLongitude` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `PeriodAppearance` (`PeriodAppearance`) — 0 R/W, 0 R/O, 7 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `НачалоПериода` | `BeginOfPeriod` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Даты` | `Dates` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Доступность` | `Enabled` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `КонецПериода` | `EndOfPeriod` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Шрифт` | `Font` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЦветТекста` | `TextColor` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `RowAppearance` (`RowAppearance`) — 0 R/W, 0 R/O, 5 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `ЦветФона` | `BackColor` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Ячейки` | `Cells` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Шрифт` | `Font` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ДанныеСтроки` | `RowData` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ЦветТекста` | `TextColor` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `TableBoxColumns` (`TableBoxColumns`) — 0 R/W, 0 R/O, 1 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `<Имя колонки>` | `<Column name>` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `ValueTable` (`ValueTable`) — 0 R/W, 0 R/O, 2 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Колонки` | `Columns` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Индексы` | `Indexes` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `ValueTree` (`ValueTree`) — 0 R/W, 0 R/O, 2 Runtime, 0 Unk

| Свойство (RU) | Свойство (EN) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Колонки` | `Columns` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Строки` | `Rows` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

