# Карта покрытия каталога свойств обычных форм 1С 8.2 (610 свойств)

Полная сводка покрытия каталога `1c-ordinary-forms-research/catalog.json` (610 свойств по 55 классам объектов обычных форм 1С 8.2) для операций `extract` (`unpack`) и `pack`.

## 1. Общая статистика по каталогу (610 свойств)

| Статус | Кол-во | Доля от всех (610) | Доля от сериализуемых (529) | Описание |
|---|---:|---:|---:|---|
| **🟢 R/W (`rw`)** | **364** | **59.7%** | **68.8%** | Поддерживаются в `Form.xml` и пакуются в `FormStream.bin` с read-back проверкой |
| **🟡 R/O (`ro`)** | **5** | **0.8%** | **0.9%** | Извлекаются при `extract` (`Width`/`Height` формы, `Picture` надписи, `fixed`/`nonNegative`) |
| **⚙️ Runtime (`runtime`)** | **81** | **13.3%** | — | Существуют только в памяти 1С:Предприятие во время работы формы (`ТекущаяСтрока`, `ОформлениеСтроки` и т.д.) |
| **⚪ Не покрыто (`unk`)** | **160** | **26.2%** | **30.2%** | Редкие/оформительские свойства (параметры печати `SpreadsheetDocument`, `Shortcut`, картинки кнопок) |

---

## 2. Взаимосвязанные и несовместимые свойства (Coupled Properties)

При установке свойств в Конфигураторе и проверке в `1С:Предприятие` (`oneform explore probe`) зафиксированы следующие зависимости между слотами в `FormStream.bin`:

| Элемент | Свойство А | Связанное свойство / Слот Б | Поведение 1С и реализация в `oneform` |
|---|---|---|---|
| `InputField` | `MultiLine`, `PasswordMode`, `ExtendedEdit`, `Mask` | `info[3]` (`9a7643d2-...` `[3..6]`) ↔ `info[2][0][26, 5, 24, 21]` | **Двойное хранение (Mirror Slots):** 1С хранит эти 4 свойства одновременно в расширении строкового типа `info[3]` и в базовом блоке `info[2][0]`. `oneform pack` синхронно патчит и проверяет оба места. |
| `InputField` | `TextEdit` (`РедактированиеТекста`) | `info[5]` ↔ `info[2][0][12]` (инвертированный флаг: `0`=Да, `1`=Нет) | **Инвертированное двойное хранение:** при отключении `TextEdit` (`false`) оба слота (`info[5]` и `info[2][0][12]`) обязаны быть равны `1`. |
| `InputField` | `ValueType` (`ТипЗначения`) | `info[1]` ↔ `info[3]` (расширения типов) | При смене типа с `String` на `Number`/`Date`/`Ref` расширение `9a7643d2` в `info[3]` относится только к строковой ветке; `oneform` проверяет наличие `9a7643d2` перед записью зеркала. |
| `Table` | `Header`, `Footer`, `VerticalLines`, `HorizontalLines`, `ReadOnly`, `UseAlternationRowColor`, `EnableStartDrag`, `AutoInsertNewRow`, `ChangeSettingOfColumns`, `ChangePositionOfColumns` | `info[2][1][1]` (единая 32-битная маска флагов) | **Битовая коалесценция:** все 10 свойств живут в одном 32-битном числе `info[2][1][1]` (причём бит 10 `ReadOnly` инвертирован: `0`=true, `1`=false). `oneform` объединяет правки всех 10 флагов в единый атомарный `Piece`. |
| `Table` | `FixedLeft` (`ФиксацияСлева`) и `FixedRight` (`ФиксацияСправа`) | `info[2][1][11]` и `info[2][1][12]` vs число колонок | **Взаимное ограничение в рантайме:** сумма `FixedLeft + FixedRight` не может превышать общее число видимых колонок таблицы; 1С автоматически зажимает `FixedRight` до `Columns.Count - FixedLeft`. |
| `TableColumn` | `ShowHierarchy`, `CheckBoxThreeState`, `ShowInHeader`, `ShowInFooter`, `ChangeSetting`, `ChangePosition` | `col_props[9]` (единая 32-битная маска флагов колонки) | **Битовая коалесценция колонки:** биты `0, 3, 11, 12, 22, 23` хранятся в одном атоме `col_props[9]` (`12590592` по умолчанию) и патчатся единым `Piece`. |
| `TableColumn` | `ReadOnly` (`ТолькоПросмотр`) и `Editor` (`ЭлементУправления`) | `col_props[26]` и `col_props[38]` (GUID редактора) | Если `col_props[38]` равен `00000000-0000-0000-0000-000000000000`, колонка принудительно переходит в режим `ReadOnly = true` независимо от `col_props[26]`. |
| `Panel` | `Width` / `Height` панели и `ShowTabs` | Границы вкладок `<Page>` и матрица привязок `info[1][base+8..]` | Изменение габаритов `Panel` или положения корешков `ShowTabs` автоматически пересчитывает `Left`/`Top`/`Width`/`Height` всех вложенных страниц `<Page>`. |

---

## 3. Сводная таблица покрытия по объектам

| Объект (`XML` / `API 1C`) | Всего | 🟢 R/W | 🟡 R/O | ⚙️ Runtime | ⚪ Unk | Покрытие сериализуемых |
|---|---:|---:|---:|---:|---:|---:|
| `Table` (`TableBox`) | 46 | **38** | 0 | 6 | 2 | **95.0%** |
| `InputField` (`TextBox`) | 45 | **38** | 0 | 1 | 6 | **86.4%** |
| `TableColumn` (`TableBoxColumn`) | 44 | **30** | 0 | 1 | 13 | **69.8%** |
| `ChoiceField` (`ComboBox`) | 21 | **17** | 0 | 0 | 4 | **81.0%** |
| `CalendarField` (`CalendarBox`) | 21 | **17** | 0 | 2 | 2 | **89.5%** |
| `LabelDecoration` (`Label`) | 19 | **15** | 1 | 0 | 3 | **84.2%** |
| `CheckBox` (`CheckBox`) | 15 | **15** | 0 | 0 | 0 | **100.0%** |
| `RadioButton` (`RadioButton`) | 15 | **15** | 0 | 0 | 0 | **100.0%** |
| `TrackBar` (`TrackBar`) | 15 | **15** | 0 | 0 | 0 | **100.0%** |
| `Panel` (`Panel`) | 18 | **14** | 0 | 1 | 3 | **82.4%** |
| `CommandBar` (`CommandBar`) | 14 | **14** | 0 | 0 | 0 | **100.0%** |
| `PictureField` (`PictureBox`) | 17 | **13** | 0 | 0 | 4 | **76.5%** |
| `Button` (`Button`) | 16 | **12** | 0 | 0 | 4 | **75.0%** |
| `ProgressBar` (`ProgressBar`) | 12 | **12** | 0 | 0 | 0 | **100.0%** |
| `CommandBarButton` (`CommandBarButton`) | 15 | **10** | 0 | 0 | 5 | **66.7%** |
| `ListBox` (`ListBox`) | 10 | **9** | 0 | 1 | 0 | **100.0%** |
| `UsualGroup` (`GroupBox`) | 9 | **9** | 0 | 0 | 0 | **100.0%** |
| `Panel controls extension` (`Panel controls extension`) | 9 | **7** | 0 | 0 | 2 | **77.8%** |
| `SpreadsheetDocumentField` (`SpreadsheetDocumentField`) | 8 | **7** | 0 | 0 | 1 | **87.5%** |
| `Extension for controls located in a form` (`Extension for controls located in a form`) | 11 | **6** | 0 | 1 | 4 | **60.0%** |
| `Splitter` (`Splitter`) | 6 | **6** | 0 | 0 | 0 | **100.0%** |
| `Extension for controls located in a spreadsheet document field` (`Extension for controls located in a spreadsheet document field`) | 8 | **5** | 0 | 1 | 2 | **71.4%** |
| `GraphicalSchemaField` (`GraphicalSchemaField`) | 6 | **5** | 0 | 1 | 0 | **100.0%** |
| `Font` (`Font`) | 8 | **4** | 0 | 0 | 4 | **50.0%** |
| `PanelPage` (`PanelPage`) | 7 | **4** | 0 | 2 | 1 | **80.0%** |
| `TextDocumentField` (`TextDocumentField`) | 5 | **4** | 0 | 1 | 0 | **100.0%** |
| `Color` (`Color`) | 4 | **4** | 0 | 0 | 0 | **100.0%** |
| `TypeDescription` (`TypeDescription`) | 4 | **3** | 0 | 0 | 1 | **75.0%** |
| `HTMLDocumentField` (`HTMLDocumentField`) | 4 | **3** | 0 | 1 | 0 | **100.0%** |
| `Border` (`Border`) | 3 | **3** | 0 | 0 | 0 | **100.0%** |
| `ValueListItem` (`ValueListItem`) | 4 | **2** | 0 | 0 | 2 | **50.0%** |
| `NumberQualifiers` (`NumberQualifiers`) | 3 | **2** | 1 | 0 | 0 | **100.0%** |
| `ValueList` (`ValueList`) | 2 | **2** | 0 | 0 | 0 | **100.0%** |
| `Form` (`Form`) | 38 | **1** | 2 | 12 | 23 | **11.5%** |
| `StringQualifiers` (`StringQualifiers`) | 2 | **1** | 1 | 0 | 0 | **100.0%** |
| `Extension for controls located in a table box` (`Extension for controls located in a table box`) | 2 | **1** | 0 | 1 | 0 | **100.0%** |
| `DateQualifiers` (`DateQualifiers`) | 1 | **1** | 0 | 0 | 0 | **100.0%** |
| `SpreadsheetDocument` (`SpreadsheetDocument`) | 53 | **0** | 0 | 1 | 52 | **0.0%** |
| `CellAppearance` (`CellAppearance`) | 21 | **0** | 0 | 21 | 0 | **100% (RT)** |
| `PeriodAppearance` (`PeriodAppearance`) | 7 | **0** | 0 | 7 | 0 | **100% (RT)** |
| `TextDocument` (`TextDocument`) | 6 | **0** | 0 | 0 | 6 | **0.0%** |
| `DateAppearance` (`DateAppearance`) | 6 | **0** | 0 | 6 | 0 | **100% (RT)** |
| `GeographicalSchemaField` (`GeographicalSchemaField`) | 5 | **0** | 0 | 2 | 3 | **0.0%** |
| `RowAppearance` (`RowAppearance`) | 5 | **0** | 0 | 5 | 0 | **100% (RT)** |
| `Shortcut` (`Shortcut`) | 4 | **0** | 0 | 0 | 4 | **0.0%** |
| `GeographicalSchemaShowedArea` (`GeographicalSchemaShowedArea`) | 4 | **0** | 0 | 0 | 4 | **0.0%** |
| `BinaryDataQualifiers` (`BinaryDataQualifiers`) | 2 | **0** | 0 | 0 | 2 | **0.0%** |
| `Picture` (`Picture`) | 2 | **0** | 0 | 0 | 2 | **0.0%** |
| `ValueTable` (`ValueTable`) | 2 | **0** | 0 | 2 | 0 | **100% (RT)** |
| `ValueTree` (`ValueTree`) | 2 | **0** | 0 | 2 | 0 | **100% (RT)** |
| `System enumeration text box extension` (`System enumeration text box extension`) | 1 | **0** | 0 | 0 | 1 | **0.0%** |
| `CommandBarButtons` (`CommandBarButtons`) | 1 | **0** | 0 | 1 | 0 | **100% (RT)** |
| `TableBoxColumns` (`TableBoxColumns`) | 1 | **0** | 0 | 1 | 0 | **100% (RT)** |
| `Controls` (`Controls`) | 1 | **0** | 0 | 1 | 0 | **100% (RT)** |

---

## 4. Детальная карта свойств по приоритетным элементам

### `InputField` (Всего: 45 | 🟢 R/W: 38 | 🟡 R/O: 0 | ⚙️ Runtime: 1 | ⚪ Unk: 6)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `AutoChoiceIncomplete` | `АвтоВыборНезаполненного` | `AutoChoiceIncomplete` | `info[2][0][36]` | 🟢 R/W |
| `AutoMarkIncomplete` | `АвтоОтметкаНезаполненного` | `AutoMarkIncomplete` | `info[2][0][35]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ButtonBackColor` | `ЦветФонаКнопки` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ButtonTextColor` | `ЦветТекстаКнопки` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `ChoiceButton` | `КнопкаВыбора` | `ChoiceButton` | `info[2][0][7]` | 🟢 R/W |
| `ChoiceIncomplete` | `ВыборНезаполненного` | `AutoChoiceIncomplete` | `info[2][0][36]` | 🟢 R/W |
| `ChoiceListButton` | `КнопкаСпискаВыбора` | `DropListButton` | `info[2][0][6]` | 🟢 R/W |
| `ChoiceListWidth` | `ШиринаСпискаВыбора` | `ChoiceListWidth` | `info[2][0][32]` | 🟢 R/W |
| `ChooseType` | `ВыбиратьТип` | `ChooseType` | `info[6]` | 🟢 R/W |
| `ClearButton` | `КнопкаОчистки` | `ClearButton` | `info[2][0][8]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ExtendedEdit` | `РасширенноеРедактирование` | `ExtendedEdit` | `info[3] 9a7643d2[6] + info[2][0][24]` | 🟢 R/W |
| `FieldBackColor` | `ЦветФонаПоля` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `FieldTextColor` | `ЦветТекстаПоля` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `Format` | `Формат` | `Format` | `info[2][0][34]` | 🟢 R/W |
| `HorizontalAlign` | `ГоризонтальноеПоложение` | `HorizontalAlign` | `info[2][0][17]` | 🟢 R/W |
| `IncompleteChoiceMode` | `РежимВыбораНезаполненного` | `AutoChoiceIncomplete` | `info[2][0][36]` | 🟢 R/W |
| `ListChoiceMode` | `РежимВыбораИзСписка` | `ChoiceListMode` | `info[9]` | 🟢 R/W |
| `MarkIncomplete` | `ОтметкаНезаполненного` | `AutoMarkIncomplete` | `info[2][0][35]` | 🟢 R/W |
| `MarkNegatives` | `ВыделятьОтрицательные` | `MarkNegatives` | `info[2][0][27]` | 🟢 R/W |
| `Mask` | `Маска` | `Mask` | `info[3] 9a7643d2[4] + info[2][0][21]` | 🟢 R/W |
| `MaxValue` | `МаксимальноеЗначение` | `MaxValue` | `info[2][0][20]` | 🟢 R/W |
| `MinValue` | `МинимальноеЗначение` | `MinValue` | `info[2][0][19]` | 🟢 R/W |
| `MultiLine` | `МногострочныйРежим` | `MultiLine` | `info[3] 9a7643d2[3] + info[2][0][26]` | 🟢 R/W |
| `OpenButton` | `КнопкаОткрытия` | `OpenButton` | `info[2][0][10]` | 🟢 R/W |
| `PasswordMode` | `РежимПароля` | `PasswordMode` | `info[3] 9a7643d2[5] + info[2][0][5]` | 🟢 R/W |
| `ReadOnly` | `ТолькоПросмотр` | `ReadOnly` | `info[2][0][13]` | 🟢 R/W |
| `SpinButton` | `КнопкаРегулирования` | `SpinButton` | `info[2][0][9]` | 🟢 R/W |
| `TextEdit` | `РедактированиеТекста` | `TextEdit` | `info[5] + info[2][0][12]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `TypeLinkItem` | `ЭлементСвязиПоТипу` | `TypeLink` | `info[8]` | 🟢 R/W |
| `TypeRestriction` | `ОграничениеТипа` | `ValueType` | `info[1]` | 🟢 R/W |
| `VerticalAlign` | `ВертикальноеПоложение` | `VerticalAlign` | `info[2][0][18]` | 🟢 R/W |
| `Wrap` | `АвтоПереносСтрок` | `AutoCellHeight` | `info[2][0][4]` | 🟢 R/W |
| `ChoiceButtonPicture` | `КартинкаКнопкиВыбора` | `ChoiceButtonPicture` | `` | ⚪ Unk |
| `ChoiceList` | `СписокВыбора` | `ChoiceList` | `Не исследован` | ⚪ Unk |
| `ChoiceListHeight` | `ВысотаСпискаВыбора` | `ChoiceListHeight` | `Не исследован` | ⚪ Unk |
| `Picture` | `Картинка` | `Picture` | `Не исследован` | ⚪ Unk |
| `SelButtonPicture` | `КартинкаКнопкиВыбора` | `SelButtonPicture` | `Не исследован` | ⚪ Unk |
| `Shortcut` | `СочетаниеКлавиш` | `Shortcut` | `Не исследован` | ⚪ Unk |
| `SelectedText` | `ВыделенныйТекст` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Table` (Всего: 46 | 🟢 R/W: 38 | 🟡 R/O: 0 | ⚙️ Runtime: 6 | ⚪ Unk: 2)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `AlternationRowBackColor` | `ЦветФонаЧередованияСтрок` | `AlternationRowBackColor` | `info[2][1][8]` | 🟢 R/W |
| `AutoInsertNewRow` | `АвтоВводНовойСтроки` | `AutoInsertNewRow` | `info[2][1][1] bit 15` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ChangePositionOfColumns` | `ИзменятьПозициюКолонок` | `ChangePositionOfColumns` | `info[2][1][1] bit 26` | 🟢 R/W |
| `ChangeRowSet` | `ИзменятьСоставСтрок` | `ChangeRowSet` | `info[2][1][22]` | 🟢 R/W |
| `ChangeSettingOfColumns` | `ИзменятьНастройкуКолонок` | `ChangeSettingOfColumns` | `info[2][1][1] bit 25` | 🟢 R/W |
| `EnableStartDrag` | `РазрешитьНачалоПеретаскивания` | `EnableStartDrag` | `info[2][1][1] bit 14` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `FieldBackColor` | `ЦветФонаПоля` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `FixedLeft` | `ФиксацияСлева` | `FixedLeft` | `info[2][1][11]` | 🟢 R/W |
| `FixedRight` | `ФиксацияСправа` | `FixedRight` | `info[2][1][12]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `Footer` | `Подвал` | `Footer` | `info[2][1][1] bit 1` | 🟢 R/W |
| `FooterBackColor` | `ЦветФонаПодвала` | `FooterBackColor` | `info[2][1][4]` | 🟢 R/W |
| `FooterFont` | `ШрифтПодвала` | `FooterFont` | `info[2][1][19]` | 🟢 R/W |
| `FooterHeight` | `ВысотаПодвала` | `FooterHeight` | `info[2][1][17]` | 🟢 R/W |
| `FooterTextColor` | `ЦветТекстаПодвала` | `FooterTextColor` | `info[2][1][5]` | 🟢 R/W |
| `Header` | `Шапка` | `Header` | `info[2][1][1] bit 0` | 🟢 R/W |
| `HeaderBackColor` | `ЦветФонаШапки` | `HeaderBackColor` | `info[2][1][2]` | 🟢 R/W |
| `HeaderFont` | `ШрифтШапки` | `HeaderFont` | `info[2][1][18]` | 🟢 R/W |
| `HeaderHeight` | `ВысотаШапки` | `HeaderHeight` | `info[2][1][16]` | 🟢 R/W |
| `HeaderTextColor` | `ЦветТекстаШапки` | `HeaderTextColor` | `info[2][1][3]` | 🟢 R/W |
| `HorizontalLines` | `ГоризонтальныеЛинии` | `HorizontalLines` | `info[2][1][1] bit 3` | 🟢 R/W |
| `HorizontalScrollBar` | `ГоризонтальнаяПолосаПрокрутки` | `HorizontalScrollBar` | `info[2][1][9]` | 🟢 R/W |
| `InitialListView` | `НачальноеОтображениеСписка` | `InitialListView` | `info[2][1][20]` | 🟢 R/W |
| `InitialTreeView` | `НачальноеОтображениеДерева` | `InitialTreeView` | `info[2][1][21]` | 🟢 R/W |
| `Output` | `Вывод` | `Output` | `info[2][1][24]` | 🟢 R/W |
| `ReadOnly` | `ТолькоПросмотр` | `ReadOnly` | `info[2][1][1] bit 10` | 🟢 R/W |
| `RowInputMode` | `РежимВводаСтрок` | `RowInputMode` | `info[2][1][13]` | 🟢 R/W |
| `RowSelectionMode` | `РежимВыделенияСтроки` | `RowSelectionMode` | `info[2][1][14]` | 🟢 R/W |
| `SelectionBackColor` | `ЦветФонаВыделения` | `SelectionBackColor` | `info[2][1][6]` | 🟢 R/W |
| `SelectionMode` | `РежимВыделения` | `SelectionMode` | `info[2][1][15]` | 🟢 R/W |
| `SelectionTextColor` | `ЦветТекстаВыделения` | `SelectionTextColor` | `info[2][1][7]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `UseAlternationRowColor` | `ЧередованиеЦветовСтрок` | `UseAlternationRowColor` | `info[2][1][1] bit 4` | 🟢 R/W |
| `VerticalLines` | `ВертикальныеЛинии` | `VerticalLines` | `info[2][1][1] bit 2` | 🟢 R/W |
| `VerticalScrollBar` | `ВертикальнаяПолосаПрокрутки` | `VerticalScrollBar` | `info[2][1][10]` | 🟢 R/W |
| `ChoiceMode` | `РежимВыбора` | `ChoiceMode` | `Не исследован` | ⚪ Unk |
| `EnableDrag` | `РазрешитьПеретаскивание` | `EnableDrag` | `Не исследован` | ⚪ Unk |
| `Columns` | `Колонки` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `CurrentColumn` | `ТекущаяКолонка` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `CurrentData` | `ТекущиеДанные` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `CurrentParent` | `ТекущийРодитель` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `CurrentRow` | `ТекущаяСтрока` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `SelectedRows` | `ВыделенныеСтроки` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `TableColumn` (Всего: 44 | 🟢 R/W: 30 | 🟡 R/O: 0 | ⚙️ Runtime: 1 | ⚪ Unk: 13)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `CellHeight` | `ВысотаЯчейки` | `CellHeight` | `col_props[37]` | 🟢 R/W |
| `ChangePosition` | `ИзменятьПозицию` | `ChangePosition` | `col_props[9] bit 23` | 🟢 R/W |
| `ChangeSetting` | `ИзменятьНастройку` | `ChangeSetting` | `col_props[9] bit 22` | 🟢 R/W |
| `CheckBoxThreeState` | `ТриСостоянияФлажка` | `CheckBoxThreeState` | `col_props[9] bit 3` | 🟢 R/W |
| `Data` | `Данные` | `DataPath` | `wrapper[2]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `FieldBackColor` | `ЦветФонаПоля` | `FieldBackColor` | `col_props[16]` | 🟢 R/W |
| `FieldTextColor` | `ЦветТекстаПоля` | `FieldTextColor` | `col_props[17]` | 🟢 R/W |
| `FooterBackColor` | `ЦветФонаПодвала` | `FooterBackColor` | `col_props[20]` | 🟢 R/W |
| `FooterFont` | `ШрифтПодвала` | `FooterFont` | `col_props[24]` | 🟢 R/W |
| `FooterHorizontalAlign` | `ГоризонтальноеПоложениеВПодвале` | `FooterHorizontalAlign` | `col_props[29]` | 🟢 R/W |
| `FooterText` | `ТекстПодвала` | `FooterText` | `col_props[2]` | 🟢 R/W |
| `FooterTextColor` | `ЦветТекстаПодвала` | `FooterTextColor` | `col_props[21]` | 🟢 R/W |
| `Format` | `Формат` | `Format` | `col_props[34]` | 🟢 R/W |
| `HeaderBackColor` | `ЦветФонаШапки` | `HeaderBackColor` | `col_props[18]` | 🟢 R/W |
| `HeaderFont` | `ШрифтШапки` | `HeaderFont` | `col_props[23]` | 🟢 R/W |
| `HeaderHorizontalAlign` | `ГоризонтальноеПоложениеВШапке` | `HeaderHorizontalAlign` | `col_props[27]` | 🟢 R/W |
| `HeaderText` | `ТекстШапки` | `Header` | `col_props[1]` | 🟢 R/W |
| `HeaderTextColor` | `ЦветТекстаШапки` | `HeaderTextColor` | `col_props[19]` | 🟢 R/W |
| `HeaderToolTip` | `ПодсказкаВШапке` | `Tooltip` | `col_props[3]` | 🟢 R/W |
| `HorizontalAlignInColumn` | `ГоризонтальноеПоложениеВКолонке` | `HorizontalAlignInColumn` | `col_props[28]` | 🟢 R/W |
| `Name` | `Имя` | `name` | `col_props[30]` | 🟢 R/W |
| `ReadOnly` | `ТолькоПросмотр` | `ReadOnly` | `col_props[26]` | 🟢 R/W |
| `ShowHierarchy` | `ОтображатьИерархию` | `ShowHierarchy` | `col_props[9] bit 0` | 🟢 R/W |
| `ShowInFooter` | `ОтображатьВПодвале` | `ShowInFooter` | `col_props[9] bit 12` | 🟢 R/W |
| `ShowInHeader` | `ОтображатьВШапке` | `ShowInHeader` | `col_props[9] bit 11` | 🟢 R/W |
| `SkipOnInput` | `ПропускатьПриВводе` | `SkipOnInput` | `meta[4]` | 🟢 R/W |
| `TextFont` | `ШрифтТекста` | `TextFont` | `col_props[22]` | 🟢 R/W |
| `Visible` | `Видимость` | `Visible` | `col_props[25]` | 🟢 R/W |
| `Width` | `Ширина` | `Width` | `col_props[4]` | 🟢 R/W |
| `AutoCellHeight` | `АвтоВысотаЯчейки` | `AutoCellHeight` | `Не исследован` | ⚪ Unk |
| `AutoMarkIncomplete` | `АвтоОтметкаНезаполненного` | `AutoMarkIncomplete` | `Не исследован` | ⚪ Unk |
| `ChangeVisible` | `ИзменятьВидимость` | `ChangeVisible` | `Не исследован` | ⚪ Unk |
| `CheckData` | `ДанныеФлажка` | `CheckData` | `Не исследован` | ⚪ Unk |
| `EditMode` | `РежимРедактирования` | `EditMode` | `Не исследован` | ⚪ Unk |
| `FooterPicture` | `КартинкаПодвала` | `FooterPicture` | `Не исследован` | ⚪ Unk |
| `HeaderAdditionalPicture` | `ДополнительнаяКартинкаШапки` | `HeaderAdditionalPicture` | `Не исследован` | ⚪ Unk |
| `HeaderPicture` | `КартинкаШапки` | `HeaderPicture` | `` | ⚪ Unk |
| `Hyperlink` | `Гиперссылка` | `Hyperlink` | `Не исследован` | ⚪ Unk |
| `Location` | `Положение` | `Location` | `Не исследован` | ⚪ Unk |
| `MarkNegatives` | `ВыделятьОтрицательные` | `MarkNegatives` | `Не исследован` | ⚪ Unk |
| `RowsPictures` | `КартинкиСтрок` | `RowsPictures` | `Не исследован` | ⚪ Unk |
| `SizeChange` | `ИзменениеРазмера` | `SizeChange` | `Не исследован` | ⚪ Unk |
| `Control` | `ЭлементУправления` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Button` (Всего: 16 | 🟢 R/W: 12 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 4)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ButtonBackColor` | `ЦветФонаКнопки` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ButtonTextColor` | `ЦветТекстаКнопки` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `Caption` | `Заголовок` | `Caption` | `info[1][2]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `HorizontalAlign` | `ГоризонтальноеПоложение` | `HorizontalAlign` | `info[1][3]` | 🟢 R/W |
| `MultiLine` | `МногострочныйРежим` | `MultiLine` | `info[1][10]` | 🟢 R/W |
| `PictureLocation` | `ПоложениеКартинки` | `PictureLocation` | `info[1][6]` | 🟢 R/W |
| `PictureSize` | `РазмерКартинки` | `PictureSize` | `info[1][7]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `VerticalAlign` | `ВертикальноеПоложение` | `VerticalAlign` | `info[1][4]` | 🟢 R/W |
| `Buttons` | `Кнопки` | `Buttons` | `Не исследован` | ⚪ Unk |
| `MenuMode` | `РежимМеню` | `MenuMode` | `Не исследован` | ⚪ Unk |
| `Picture` | `Картинка` | `Picture` | `Не исследован` | ⚪ Unk |
| `Shortcut` | `СочетаниеКлавиш` | `Shortcut` | `Не исследован` | ⚪ Unk |

### `LabelDecoration` (Всего: 19 | 🟢 R/W: 15 | 🟡 R/O: 1 | ⚙️ Runtime: 0 | ⚪ Unk: 3)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Caption` | `Заголовок` | `Caption` | `info[1][2]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `Format` | `Формат` | `Format` | `info[1][10]` | 🟢 R/W |
| `HorizontalAlign` | `ГоризонтальноеПоложение` | `HorizontalAlign` | `info[1][3]` | 🟢 R/W |
| `Hyperlink` | `Гиперссылка` | `Hyperlink` | `info[1][5]` | 🟢 R/W |
| `MarkNegatives` | `ВыделятьОтрицательные` | `MarkNegatives` | `info[1][6]` | 🟢 R/W |
| `PictureSize` | `РазмерКартинки` | `PictureSize` | `info[1][13]` | 🟢 R/W |
| `TextColor` | `ЦветТекста` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `VerticalAlign` | `ВертикальноеПоложение` | `VerticalAlign` | `info[1][4]` | 🟢 R/W |
| `Picture` | `Картинка` | `Picture` | `info[1][12]` | 🟡 R/O |
| `LabelPictureLocation` | `ПоложениеКартинкиНадписи` | `LabelPictureLocation` | `Не исследован` | ⚪ Unk |
| `ScrollingText` | `БегущаяСтрока` | `ScrollingText` | `Не исследован` | ⚪ Unk |
| `Shortcut` | `СочетаниеКлавиш` | `Shortcut` | `Не исследован` | ⚪ Unk |

### `CheckBox` (Всего: 15 | 🟢 R/W: 15 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Caption` | `Заголовок` | `Caption` | `info[1][0][2]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `FieldBackColor` | `ЦветФонаПоля` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `FieldTextColor` | `ЦветТекстаПоля` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `HorizontalAlign` | `ГоризонтальноеПоложение` | `HorizontalAlign` | `info[1][0][4]` | 🟢 R/W |
| `TextColor` | `ЦветТекста` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `ThreeState` | `ТриСостояния` | `ThreeState` | `info[1][2]` | 🟢 R/W |
| `TitleLocation` | `ПоложениеЗаголовка` | `TitleLocation` | `info[1][0][3]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `VerticalAlign` | `ВертикальноеПоложение` | `VerticalAlign` | `info[1][0][5]` | 🟢 R/W |

### `RadioButton` (Всего: 15 | 🟢 R/W: 15 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Caption` | `Заголовок` | `Caption` | `info[2][0][2]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `FieldBackColor` | `ЦветФонаПоля` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `FieldTextColor` | `ЦветТекстаПоля` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `HorizontalAlign` | `ГоризонтальноеПоложение` | `HorizontalAlign` | `info[2][0][4]` | 🟢 R/W |
| `SelectionValue` | `ВыбираемоеЗначение` | `SelectionValue` | `info[4]` | 🟢 R/W |
| `TextColor` | `ЦветТекста` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `TitleLocation` | `ПоложениеЗаголовка` | `TitleLocation` | `info[2][0][3]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `VerticalAlign` | `ВертикальноеПоложение` | `VerticalAlign` | `info[2][0][5]` | 🟢 R/W |

### `ChoiceField` (Всего: 21 | 🟢 R/W: 17 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 4)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ButtonBackColor` | `ЦветФонаКнопки` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ButtonTextColor` | `ЦветТекстаКнопки` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `ChoiceButton` | `КнопкаВыбора` | `ChoiceButton` | `info[1][7]` | 🟢 R/W |
| `ChoiceList` | `СписокВыбора` | `ChoiceList` | `info[1][26]` | 🟢 R/W |
| `ChoiceListWidth` | `ШиринаСпискаВыбора` | `ChoiceListWidth` | `info[1][32]` | 🟢 R/W |
| `ClearButton` | `КнопкаОчистки` | `ClearButton` | `info[1][8]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `FieldBackColor` | `ЦветФонаПоля` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `FieldTextColor` | `ЦветТекстаПоля` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `OpenButton` | `КнопкаОткрытия` | `OpenButton` | `info[1][10]` | 🟢 R/W |
| `ReadOnly` | `ТолькоПросмотр` | `ReadOnly` | `info[1][13]` | 🟢 R/W |
| `SpinButton` | `КнопкаРегулирования` | `SpinButton` | `info[1][9]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `ChoiceButtonPicture` | `КартинкаКнопкиВыбора` | `ChoiceButtonPicture` | `Не исследован` | ⚪ Unk |
| `ChoiceListHeight` | `ВысотаСпискаВыбора` | `ChoiceListHeight` | `Не исследован` | ⚪ Unk |
| `Picture` | `Картинка` | `Picture` | `Не исследован` | ⚪ Unk |
| `SelButtonPicture` | `КартинкаКнопкиВыбора` | `SelButtonPicture` | `Не исследован` | ⚪ Unk |

### `Panel` (Всего: 18 | 🟢 R/W: 14 | 🟡 R/O: 0 | ⚙️ Runtime: 1 | ⚪ Unk: 3)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `AutoTabOrder` | `АвтоПорядокОбхода` | `AutoTabOrder` | `info[1][base+5]` | 🟢 R/W |
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `Pages` | `Страницы` | `Pages` | `info[1][base+3]` | 🟢 R/W |
| `ScrollPageMode` | `РежимПрокручиваемыхСтраниц` | `ScrollPageMode` | `info[1][base+6]` | 🟢 R/W |
| `SeparateByPage` | `РаспределятьПоСтраницам` | `SeparateByPage` | `info[1][base+4]` | 🟢 R/W |
| `ShowTabs` | `ОтображениеЗакладок` | `ShowTabs` | `info[1][base+1]` | 🟢 R/W |
| `TabOrder` | `ПорядокОбхода` | `TabOrder` | `geom[22]` | 🟢 R/W |
| `TextColor` | `ЦветТекста` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `PanelPictureLocation` | `ПоложениеКартинкиПанели` | `PanelPictureLocation` | `Не исследован` | ⚪ Unk |
| `Picture` | `Картинка` | `Picture` | `Не исследован` | ⚪ Unk |
| `PictureSize` | `РазмерКартинки` | `PictureSize` | `Не исследован` | ⚪ Unk |
| `CurrentPage` | `ТекущаяСтраница` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `PanelPage` (Всего: 7 | 🟢 R/W: 4 | 🟡 R/O: 0 | ⚙️ Runtime: 2 | ⚪ Unk: 1)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Caption` | `Заголовок` | `Title` | `rec[1]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `rec[5]` | 🟢 R/W |
| `Name` | `Имя` | `name` | `rec[6]` | 🟢 R/W |
| `Visible` | `Видимость` | `Visible` | `rec[4]` | 🟢 R/W |
| `TitlePicture` | `КартинкаЗаголовка` | `TitlePicture` | `Не исследован` | ⚪ Unk |
| `IsOpen` | `Раскрыта` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Value` | `Значение` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `CommandBar` (Всего: 14 | 🟢 R/W: 14 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `AutoFill` | `АвтоЗаполнение` | `AutoFill` | `info[1][3]` | 🟢 R/W |
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ButtonBackColor` | `ЦветФонаКнопки` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ButtonTextColor` | `ЦветТекстаКнопки` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `Buttons` | `Кнопки` | `Buttons` | `info[1][7]` | 🟢 R/W |
| `ButtonsAlignment` | `ВыравниваниеКнопок` | `HorizontalAlign` | `info[1][4]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `Orientation` | `Ориентация` | `Orientation` | `info[1][2]` | 🟢 R/W |
| `Secondary` | `Вспомогательная` | `Secondary` | `info[1][5]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |

### `CommandBarButton` (Всего: 15 | 🟢 R/W: 10 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 5)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Action` | `Действие` | `action` | `action[4][1]` | 🟢 R/W |
| `ButtonType` | `ТипКнопки` | `kind` | `b[9]` | 🟢 R/W |
| `Buttons` | `Кнопки` | `Buttons` | `X[7] group` | 🟢 R/W |
| `Check` | `Пометка` | `checked` | `b[12]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ModifiesData` | `ИзменяетДанные` | `ModifiesData` | `meta[3]` | 🟢 R/W |
| `Name` | `Имя` | `name` | `b[1]` | 🟢 R/W |
| `Representation` | `Отображение` | `representation` | `b[10]` | 🟢 R/W |
| `Text` | `Текст` | `Caption` | `b[4]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `action[4][2][3]` | 🟢 R/W |
| `ButtonOrder` | `ПорядокКнопок` | `ButtonOrder` | `Не исследован` | ⚪ Unk |
| `DefaultButton` | `КнопкаПоУмолчанию` | `DefaultButton` | `Не исследован` | ⚪ Unk |
| `Description` | `Пояснение` | `Description` | `Не исследован` | ⚪ Unk |
| `Picture` | `Картинка` | `Picture` | `Не исследован` | ⚪ Unk |
| `Shortcut` | `СочетаниеКлавиш` | `Shortcut` | `Не исследован` | ⚪ Unk |

### `UsualGroup` (Всего: 9 | 🟢 R/W: 9 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Caption` | `Заголовок` | `Caption` | `info[1][2]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `TextColor` | `ЦветТекста` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |

### `PictureField` (Всего: 17 | 🟢 R/W: 13 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 4)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `EnableDrag` | `РазрешитьПеретаскивание` | `EnableDrag` | `info[1][9]` | 🟢 R/W |
| `EnableStartDrag` | `РазрешитьНачалоПеретаскивания` | `EnableStartDrag` | `info[1][8]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `NonselectedPictureText` | `ТекстНевыбраннойКартинки` | `NonselectedPictureText` | `info[1][10]` | 🟢 R/W |
| `TextColor` | `ЦветТекста` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `UseContextMenu` | `ИспользоватьКонтекстноеМеню` | `UseContextMenu` | `info[1][12]` | 🟢 R/W |
| `Zoomable` | `Масштабировать` | `Zoomable` | `info[1][6]` | 🟢 R/W |
| `Hyperlink` | `Гиперссылка` | `Hyperlink` | `Не исследован` | ⚪ Unk |
| `Picture` | `Картинка` | `Picture` | `Не исследован` | ⚪ Unk |
| `PictureSize` | `РазмерКартинки` | `PictureSize` | `Не исследован` | ⚪ Unk |
| `Shortcut` | `СочетаниеКлавиш` | `Shortcut` | `Не исследован` | ⚪ Unk |

### `CalendarField` (Всего: 21 | 🟢 R/W: 17 | 🟡 R/O: 0 | ⚙️ Runtime: 2 | ⚪ Unk: 2)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ButtonBackColor` | `ЦветФонаКнопки` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `ButtonTextColor` | `ЦветТекстаКнопки` | `ButtonTextColor` | `<base>[10]` | 🟢 R/W |
| `CalendarNavigation` | `ПеремещениеПоКалендарю` | `CalendarNavigation` | `info[1][8]` | 🟢 R/W |
| `EnableDrag` | `РазрешитьПеретаскивание` | `EnableDrag` | `info[1][11]` | 🟢 R/W |
| `EnableStartDrag` | `РазрешитьНачалоПеретаскивания` | `EnableStartDrag` | `info[1][10]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `FieldBackColor` | `ЦветФонаПоля` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `FieldTextColor` | `ЦветТекстаПоля` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `HolidayColor` | `ЦветВыходныхДней` | `HolidayColor` | `info[1][4]` | 🟢 R/W |
| `SelectionBackColor` | `ЦветФонаВыделения` | `SelectionBackColor` | `info[1][2]` | 🟢 R/W |
| `SelectionMode` | `РежимВыделения` | `SelectionMode` | `info[1][9]` | 🟢 R/W |
| `SelectionTextColor` | `ЦветТекстаВыделения` | `SelectionTextColor` | `info[1][3]` | 🟢 R/W |
| `ShowCurrentDate` | `ОтображатьТекущуюДату` | `ShowCurrentDate` | `info[1][7]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `BeginOfDisplayPeriod` | `НачалоПериодаОтображения` | `BeginOfDisplayPeriod` | `Не исследован` | ⚪ Unk |
| `EndOfDisplayPeriod` | `КонецПериодаОтображения` | `EndOfDisplayPeriod` | `Не исследован` | ⚪ Unk |
| `CurrentDate` | `ТекущаяДата` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `SelectedDates` | `ВыделенныеДаты` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `TrackBar` (Всего: 15 | 🟢 R/W: 15 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `ButtonBackColor` | `ЦветФонаКнопки` | `ButtonBackColor` | `<base>[9]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `LargeStep` | `БольшойШаг` | `LargeStep` | `info[1][5]` | 🟢 R/W |
| `MarkingAppearance` | `ОтображениеРазметки` | `MarkingAppearance` | `info[1][7]` | 🟢 R/W |
| `MarkingStep` | `ШагРазметки` | `MarkingStep` | `info[1][8]` | 🟢 R/W |
| `MaxValue` | `МаксимальноеЗначение` | `MaxValue` | `info[1][3]` | 🟢 R/W |
| `MinValue` | `МинимальноеЗначение` | `MinValue` | `info[1][2]` | 🟢 R/W |
| `Orientation` | `Ориентация` | `Orientation` | `info[1][6]` | 🟢 R/W |
| `Step` | `Шаг` | `Step` | `info[1][4]` | 🟢 R/W |
| `TextColor` | `ЦветТекста` | `TextColor` | `<base>[3]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |

### `ProgressBar` (Всего: 12 | 🟢 R/W: 12 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `MaxValue` | `МаксимальноеЗначение` | `MaxValue` | `info[1][3]` | 🟢 R/W |
| `MinValue` | `МинимальноеЗначение` | `MinValue` | `info[1][2]` | 🟢 R/W |
| `Orientation` | `Ориентация` | `Orientation` | `info[1][7]` | 🟢 R/W |
| `ShowPercent` | `ОтображатьПроценты` | `ShowPercent` | `info[1][6]` | 🟢 R/W |
| `ShowStyle` | `СтильОтображения` | `ShowStyle` | `info[1][5]` | 🟢 R/W |
| `Step` | `Шаг` | `Step` | `info[1][4]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |

### `ListBox` (Всего: 10 | 🟢 R/W: 9 | 🟡 R/O: 0 | ⚙️ Runtime: 1 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `FieldBackColor` | `ЦветФонаПоля` | `FieldBackColor` | `<base>[7]` | 🟢 R/W |
| `FieldTextColor` | `ЦветТекстаПоля` | `FieldTextColor` | `<base>[8]` | 🟢 R/W |
| `ReadOnly` | `ТолькоПросмотр` | `ReadOnly` | `info[1][6]` | 🟢 R/W |
| `ShowCheckBox` | `ОтображатьПометку` | `ShowCheckBox` | `info[1][5]` | 🟢 R/W |
| `ShowPicture` | `ОтображатьКартинку` | `ShowPicture` | `info[1][4]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |
| `CurrentRow` | `ТекущаяСтрока` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Splitter` (Всего: 6 | 🟢 R/W: 6 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `BackColor` | `ЦветФона` | `BackColor` | `<base>[2]` | 🟢 R/W |
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Orientation` | `Ориентация` | `Orientation` | `info[1][2]` | 🟢 R/W |
| `ToolTip` | `Подсказка` | `Tooltip` | `<base>[12]` | 🟢 R/W |

### `SpreadsheetDocumentField` (Всего: 8 | 🟢 R/W: 7 | 🟡 R/O: 0 | ⚙️ Runtime: 0 | ⚪ Unk: 1)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `EnableDrag` | `РазрешитьПеретаскивание` | `EnableDrag` | `info[16]` | 🟢 R/W |
| `EnableStartDrag` | `РазрешитьНачалоПеретаскивания` | `EnableStartDrag` | `info[15]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `HorizontalScrollBar` | `ГоризонтальнаяПолосаПрокрутки` | `HorizontalScrollBar` | `info[31]` | 🟢 R/W |
| `VerticalScrollBar` | `ВертикальнаяПолосаПрокрутки` | `VerticalScrollBar` | `info[32]` | 🟢 R/W |
| `ShowSelection` | `ОтображатьВыделение` | `ShowSelection` | `Не исследован` | ⚪ Unk |

### `TextDocumentField` (Всего: 5 | 🟢 R/W: 4 | 🟡 R/O: 0 | ⚙️ Runtime: 1 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `Font` | `Шрифт` | `Font` | `<base>[4]` | 🟢 R/W |
| `SelectedText` | `ВыделенныйТекст` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `GraphicalSchemaField` (Всего: 6 | 🟢 R/W: 5 | 🟡 R/O: 0 | ⚙️ Runtime: 1 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Enabled` | `Доступность` | `Enabled` | `<base>[1]` | 🟢 R/W |
| `ReadOnly` | `ТолькоПросмотр` | `ReadOnly` | `info[4]` | 🟢 R/W |
| `Transparent` | `ПрозрачныйФон` | `TransparentBack` | `<base>[5]` | 🟢 R/W |
| `CurrentItem` | `ТекущийЭлемент` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `HTMLDocumentField` (Всего: 4 | 🟢 R/W: 3 | 🟡 R/O: 0 | ⚙️ Runtime: 1 | ⚪ Unk: 0)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Border` | `Рамка` | `BorderStyle` | `<base>[11][3]` | 🟢 R/W |
| `BorderColor` | `ЦветРамки` | `BorderColor` | `<base>[6]` | 🟢 R/W |
| `Output` | `Вывод` | `Output` | `info[6]` | 🟢 R/W |
| `Document` | `Документ` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

### `Form` (Всего: 38 | 🟢 R/W: 1 | 🟡 R/O: 2 | ⚙️ Runtime: 12 | ⚪ Unk: 23)

| Свойство (EN) | Свойство (RU) | Тег в `Form.xml` | Слот в `FormStream.bin` | Статус |
|---|---|---|---|---|
| `Caption` | `Заголовок` | `Caption` | `root[1][1][0]` | 🟢 R/W |
| `Height` | `Высота` | `Height` | `root[1][4]` | 🟡 R/O |
| `Width` | `Ширина` | `Width` | `root[1][3]` | 🟡 R/O |
| `AllowAutohideState` | `РазрешитьСостояниеПрячущееся` | `AllowAutohideState` | `Не исследован` | ⚪ Unk |
| `AllowClose` | `РазрешитьЗакрытие` | `AllowClose` | `Не исследован` | ⚪ Unk |
| `AllowDockedState` | `РазрешитьСостояниеПрикрепленное` | `AllowDockedState` | `Не исследован` | ⚪ Unk |
| `AllowFloatState` | `РазрешитьСостояниеСвободное` | `AllowFloatState` | `Не исследован` | ⚪ Unk |
| `AllowJoinWindow` | `РазрешитьСоединятьОкно` | `AllowJoinWindow` | `Не исследован` | ⚪ Unk |
| `AllowNormalState` | `РазрешитьСостояниеОбычное` | `AllowNormalState` | `Не исследован` | ⚪ Unk |
| `AutoTitle` | `АвтоЗаголовок` | `AutoTitle` | `` | ⚪ Unk |
| `CaptionPicture` | `КартинкаЗаголовка` | `CaptionPicture` | `Не исследован` | ⚪ Unk |
| `ChangeWindowAppearanceMode` | `ИзменятьСпособОтображенияОкна` | `ChangeWindowAppearanceMode` | `Не исследован` | ⚪ Unk |
| `CloseOnChoice` | `ЗакрыватьПриВыборе` | `CloseOnChoice` | `` | ⚪ Unk |
| `CloseOnOwnerClose` | `ЗакрыватьПриЗакрытииВладельца` | `CloseOnOwnerClose` | `` | ⚪ Unk |
| `ConnectableWindow` | `СоединяемоеОкно` | `ConnectableWindow` | `Не исследован` | ⚪ Unk |
| `DesktopMode` | `РежимРабочегоСтола` | `DesktopMode` | `Не исследован` | ⚪ Unk |
| `EnterKeyBehavior` | `ПоведениеКлавишиEnter` | `EnterKeyBehavior` | `` | ⚪ Unk |
| `ReadOnly` | `ТолькоПросмотр` | `ReadOnly` | `` | ⚪ Unk |
| `SizeChange` | `ИзменениеРазмера` | `SizeChange` | `Не исследован` | ⚪ Unk |
| `Style` | `Стиль` | `Style` | `Не исследован` | ⚪ Unk |
| `WindowAppearanceMode` | `СпособОтображенияОкна` | `WindowAppearanceMode` | `Не исследован` | ⚪ Unk |
| `WindowDockLocation` | `ПоложениеПрикрепленногоОкна` | `WindowDockLocation` | `Не исследован` | ⚪ Unk |
| `WindowLocation` | `ПоложениеОкна` | `WindowLocation` | `Не исследован` | ⚪ Unk |
| `WindowOptionsKey` | `КлючСохраненияПоложенияОкна` | `WindowOptionsKey` | `` | ⚪ Unk |
| `WindowOptionsName` | `ИмяСохраненияПоложенияОкна` | `WindowOptionsName` | `Не исследован` | ⚪ Unk |
| `WindowState` | `СостояниеОкна` | `WindowState` | `Не исследован` | ⚪ Unk |
| `<Attribute name>` | `<Имя реквизита>` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ChoiceInitialValue` | `НачальноеЗначениеВыбора` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ChoiceMode` | `РежимВыбора` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Controls` | `ЭлементыФормы` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `CurrentControl` | `ТекущийЭлемент` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `FormOwner` | `ВладелецФормы` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ModalMode` | `МодальныйРежим` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Modified` | `Модифицированность` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `MultipleChoice` | `МножественныйВыбор` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `Panel` | `Панель` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `ThisForm` | `ЭтаФорма` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |
| `UniqueKey` | `КлючУникальности` | `-` | `Runtime-only (память 1С)` | ⚙️ Runtime |

