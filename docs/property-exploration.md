# Property exploration: observed slot map (tier 1 + runtime sweep)

Input for future `ordinary-form-patching` deltas. Every entry below was produced by a real
experiment gated through both the 1C Designer (`dump -> load`) and 1C:Enterprise (`/Execute`
runtime introspection of `ЭлементыФормы.*`) on platform 8.3.27.2342 — see
`tests/explore/<id>/manifest.json` and OpenSpec changes `form-property-exploration-corpus` and
`explore-runtime-property-sweep`.

Observation channels per experiment:

- `observed.json` / `net-diff.json` — raw property-tree diff (mutated vs platform-normalized), net of the baseline normalization.
- `decoded-diff.json` — semantic diff through the offline `oneform` decoder: fires when the slot is an already-known property.
- `runtime-diff.json` — runtime property diff captured from `ЭлементыФормы` inside `ПриОткрытии()` in `1cv8.exe ENTERPRISE /Execute`, net of the baseline runtime snapshot.
- `manifest.json` — outcome (`accept` / `reject` / `silent-drop`), control survival, and Designer/Runtime run summaries.

## Confirmed mappings (verified by the 1C:Enterprise runtime channel)

### 1. Universal & shared appearance-block properties (`ControlBase`)

Every control carries an 18-element base appearance node (`{16, Enabled, ..., TransparentBack, ...}`):
- `InputField`: `info[2][0][0]`
- `Button`, `LabelDecoration`: `info[1][0]`
- `CheckBox`: `info[1][0][0]`
- `Table`: `info[2][0]`

| Slot | Controls | 1C Runtime Property | Values (`bin` -> `1C`) | Evidence |
|---|---|---|---|---|
| `geom[5]` | all controls | `Видимость` (`Visible`) | `1 -> 0` => `Да -> Нет` | `smoke-visible-off` |
| `<base>[1]` (`info[2][0][0][1]`, `info[1][0][1]`, `info[1][0][0][1]`, `info[2][0][1]`) | InputField, Button, CheckBox, Table, LabelDecoration | `Доступность` (`Enabled`) | `1 -> 0` => `Да -> Нет` | `inputfield-base-01`, `button-base-enabled`, `checkbox-base-enabled`, `table-base-enabled`, `label-base-enabled` |
| `<base>[5]` (`info[2][0][0][5]`) | InputField (and shared `<base>`) | `ПрозрачныйФон` (`TransparentBack`) | `0 -> 1` => `Нет -> Да` | `inputfield-base-05` |

### 2. `InputField` (`ПолеВвода`) properties

| Slot | 1C Runtime Property | Values (`bin` -> `1C`) | Evidence |
|---|---|---|---|
| `info[2][0][2]` | `ГоризонтальнаяПолосаПрокрутки` (`HorizontalScrollBar`) | `0 -> 1` => `Не использовать -> Использовать всегда` | `inputfield-flag-00` |
| `info[2][0][3]` | `ВертикальнаяПолосаПрокрутки` (`VerticalScrollBar`) | `0 -> 1` => `Не использовать -> Использовать всегда` | `inputfield-flag-01` |
| `info[2][0][4]` | `АвтоПереносСтрок` (`AutoCellHeight` / word wrap) | `1 -> 0` => `Да -> Нет` | `inputfield-flag-02` |
| `info[2][0][6]` | `КнопкаСпискаВыбора` (`DropListButton`) | `1 -> 0` => `Да -> Нет` | `inputfield-flag-04` |
| `info[2][0][7]` | `КнопкаВыбора` (`ChoiceButton`) | `1 -> 0` => `Да -> Нет` | `inputfield-flag-05` (also in `decoded-diff`) |
| `info[2][0][8]` | `КнопкаОчистки` (`ClearButton`) | `1 -> 0` => `Да -> Нет` | `inputfield-flag-06` (also in `decoded-diff`) |
| `info[2][0][9]` | `КнопкаРегулирования` (`SpinButton`) | `1 -> 0` => `Да -> Нет` | `inputfield-flag-07` |
| `info[2][0][10]` | `КнопкаОткрытия` (`OpenButton`) | `1 -> 0` => `Да -> Нет` | `inputfield-flag-08` |
| `info[2][0][13]` | `ТолькоПросмотр` (`ReadOnly`) | `0 -> 1` => `Нет -> Да` | `inputfield-flag-11` |
| `info[2][0][27]` | `ВыделятьОтрицательные` (`MarkNegatives`) | `0 -> 1` => `Нет -> Да` | `inputfield-flag-21` |
| `info[2][0][35]` | `АвтоОтметкаНезаполненного` (`AutoMarkIncomplete`) | `1 -> 0` => `Да -> Нет` | `inputfield-flag-25` |
| `info[2][0][36]` | `АвтоВыборНезаполненного` (`AutoChoiceIncomplete`) | `1 -> 0` => `Да -> Нет` | `inputfield-flag-26` |
| `info[9]` | `РежимВыбораИзСписка` (`ChoiceListMode`) | `0 -> 1` => `Нет -> Да` | `inputfield-tail-09` |

### 3. `CheckBox` (`Флажок`) properties

| Slot | 1C Runtime Property | Values (`bin` -> `1C`) | Evidence |
|---|---|---|---|
| `info[1][0][3]` | `ПоложениеЗаголовка` (`TitleLocation`) | `1 -> 0` => `Текст справа -> Текст слева` | `checkbox-flag-01` |
| `info[1][2]` | `ТриСостояния` (`ThreeState`) | `0 -> 1` => `Нет -> Да` | `checkbox-flag-06` |

### 4. `Table` (`ТабличноеПоле`) properties

| Slot | 1C Runtime Property | Values (`bin` -> `1C`) | Evidence |
|---|---|---|---|
| `info[2][1][15]` | `РежимВыделения` (`SelectionMode`) | `0 -> 1` => `Одиночный -> Множественный` | `table-flag-04` |
| `info[2][1][21]` | `НачальноеОтображениеДерева` (`InitialTreeView`) | `0 -> 1` => `Не раскрывать -> Раскрывать верхний уровень` | `table-flag-07` |

## Platform-forced slots (normalization reverts the mutation)

These slots are read-only/computed for the current control state; the platform flips them back
during dump/load. They are NOT writable properties under the tested conditions.

| Slot | Control | Attempted | Result |
|---|---|---|---|
| `info[1][5]` | Button | 0 -> 1 | reverted to 0 (`button-flag-03`) |
| `info[1][0][8]` | CheckBox | 1 -> 0 | reverted to 1 (`checkbox-flag-05`) |
| `info[2][0][0][14]`, `info[2][0][0][16]` | InputField | 0 -> 1 | reverted to 0 |
| `info[2][0][22]`, `info[2][0][40]`, `info[2][0][41]`, `info[8][0]` | InputField | toggled | reverted by normalization |

## Rejected slots (invalid enum/flag or structural sub-block header)

| Slot | Control | Attempted | Result |
|---|---|---|---|
| `geom[12]`, `geom[14]`, `geom[15]`, `geom[16]`, `geom[18]`, `geom[19]` | InputField | toggled | rejected by Designer (`geom` binding/anchor headers) |
| `info[2][0][0][13]` | InputField | 0 -> 1 | rejected by Designer |
| `info[2][0][25]` | InputField | 0 -> 1 | rejected by Designer |
| `info[8][1]` | InputField | 0 -> 1 | rejected by Designer |

## Structural markers (not properties)

| Slot | Controls | Evidence |
|---|---|---|
| `info[0]` | Button, CheckBox | 38-row rewrite of the whole control record |
