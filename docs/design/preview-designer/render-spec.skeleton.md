# render-spec.md — как собрать каждую часть формы 1С в HTML/CSS

> Это **скелет**. Скопируйте файл как `render-spec.md`, заполните все разделы и **не меняйте заголовки и порядок разделов**
> (по ним мы сверяем работу). Пояснения в цитатах `>` удалите. Каждый ID из `02-property-catalog.md` должен встретиться
> в таблице своего класса: либо с правилом, либо со словами `не отображать` и причиной.

Язык: русский. Формулировки правил — «если в XML …, то в HTML …».

## §0. Константы темы

> Все числа и цвета, которые используются дальше. Продублируйте их в `theme.css` как CSS-переменные.
> Что зафиксировать — см. таблицы в `03-global-theme.md`.

### 0.1 Шрифты

| Роль | font-family | размер | начертание | где применяется |
|---|---|---|---|---|
| Текст формы | … | … | … | … |
| Кнопка | … | … | … | … |
| Таблица (ячейки) | … | … | … | … |
| Шапка таблицы | … | … | … | … |
| Закладка | … | … | … | … |
| Заголовок группы | … | … | … | … |
| Подсказка | … | … | … | … |

### 0.2 Цвета (имя CSS-переменной → значение)

| Переменная | Значение | Роль |
|---|---|---|
| `--form-bg` | … | фон формы |
| … | … | … |

### 0.3 Рамки, тени, скругления

| Что | CSS |
|---|---|
| Рамка поля | … |
| … | … |

### 0.4 Размеры

| Что | Значение |
|---|---|
| Ширина кнопки выбора в поле | … |
| … | … |

### 0.5 Значки

> Для каждого значка: имя, размер, inline-SVG (или data-URI) целиком.

| Имя | Размер | SVG |
|---|---|---|
| `icon-choice` («…») | … | … |

### 0.6 Правила для отсутствующих свойств

> Что делать, если тега нет в XML (по умолчанию). Общий принцип и исключения.

### 0.7 Общие состояния

> hover / focus / pressed / disabled / read-only / selected — как они выглядят **в 1С** и какие из них вы предлагаете показывать в статичном превью (в превью нет мыши; важны disabled, read-only, selected, checked).

### 0.8 Языки и длинные подписи

> Подписи ru/ro: обрезка (`text-overflow`), перенос, минимальная ширина кнопки.

## §1. Общие структуры

> Правила для `Font.*`, `Color.*`, `Border.*`, `Picture.*`, `Shortcut.*`: они применяются во всех классах. Здесь — единый перевод в CSS (например, `Font@bold=true` → `font-weight:bold`; `Color@kind=style` без `value` → …).

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Font.face` | … | … |
| `Font.size` | … | … |
| `Font.bold` | … | … |
| `Font.italic` | … | … |
| `Font.underline` | … | … |
| `Font.strikeout` | … | … |
| `Font.scale` | … | … |
| `Font.source` | … | … |
| `Color.kind` | … | … |
| `Color.value` | … | … |
| `Color.ref` | … | … |
| `Border.type` | … | … |
| `Border.code` | … | … |
| `Picture.kind` | … | … |
| `Picture.refGuid` | … | … |
| `Shortcut.key` | … | … |
| `Shortcut.mods` | … | … |

## §2. Форма (корень Form.xml) — `Form`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Form.Width` | … | … |
| `Form.Height` | … | … |
| `Form.Caption` | … | … |
| `Form.ShowTabs` | … | … |
| `Form.CaptionPicture` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §3. Поле ввода — `InputField`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `InputField.ChoiceButton` | … | … |
| `InputField.ClearButton` | … | … |
| `InputField.OpenButton` | … | … |
| `InputField.DropListButton` | … | … |
| `InputField.SpinButton` | … | … |
| `InputField.AutoMarkIncomplete` | … | … |
| `InputField.ReadOnly` | … | … |
| `InputField.Enabled` | … | … |
| `InputField.MultiLine` | … | … |
| `InputField.PasswordMode` | … | … |
| `InputField.MarkNegatives` | … | … |
| `InputField.HorizontalAlign` | … | … |
| `InputField.VerticalAlign` | … | … |
| `InputField.VerticalScrollBar` | … | … |
| `InputField.HorizontalScrollBar` | … | … |
| `InputField.ChoiceListMode` | … | … |
| `InputField.Format` | … | … |
| `InputField.Mask` | … | … |
| `InputField.ValueType` | … | … |
| `InputField.Font` | … | … |
| `InputField.TextColor` | … | … |
| `InputField.BackColor` | … | … |
| `InputField.FieldBackColor` | … | … |
| `InputField.FieldTextColor` | … | … |
| `InputField.ButtonBackColor` | … | … |
| `InputField.ButtonTextColor` | … | … |
| `InputField.BorderColor` | … | … |
| `InputField.BorderStyle` | … | … |
| `InputField.TransparentBack` | … | … |
| `InputField.Picture` | … | … |
| `InputField.ChoiceButtonPicture` | … | … |
| `InputField.Tooltip` | … | … |
| `InputField.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §4. Надпись — `LabelDecoration`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `LabelDecoration.Caption` | … | … |
| `LabelDecoration.HorizontalAlign` | … | … |
| `LabelDecoration.VerticalAlign` | … | … |
| `LabelDecoration.Hyperlink` | … | … |
| `LabelDecoration.TextColor` | … | … |
| `LabelDecoration.BackColor` | … | … |
| `LabelDecoration.TransparentBack` | … | … |
| `LabelDecoration.Font` | … | … |
| `LabelDecoration.Picture` | … | … |
| `LabelDecoration.PictureSize` | … | … |
| `LabelDecoration.LabelPictureLocation` | … | … |
| `LabelDecoration.BorderStyle` | … | … |
| `LabelDecoration.BorderColor` | … | … |
| `LabelDecoration.Format` | … | … |
| `LabelDecoration.MarkNegatives` | … | … |
| `LabelDecoration.ScrollingText` | … | … |
| `LabelDecoration.Enabled` | … | … |
| `LabelDecoration.Tooltip` | … | … |
| `LabelDecoration.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §5. Кнопка (самостоятельная) — `Button`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Button.Caption` | … | … |
| `Button.Visible` | … | … |
| `Button.Enabled` | … | … |
| `Button.Picture` | … | … |
| `Button.PictureLocation` | … | … |
| `Button.PictureSize` | … | … |
| `Button.Font` | … | … |
| `Button.ButtonBackColor` | … | … |
| `Button.ButtonTextColor` | … | … |
| `Button.BorderColor` | … | … |
| `Button.HorizontalAlign` | … | … |
| `Button.VerticalAlign` | … | … |
| `Button.MultiLine` | … | … |
| `Button.DefaultButton` | … | … |
| `Button.MenuMode` | … | … |
| `Button.Shortcut` | … | … |
| `Button.TransparentBack` | … | … |
| `Button.Tooltip` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §6. Флажок — `CheckBox`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `CheckBox.Caption` | … | … |
| `CheckBox.Font` | … | … |
| `CheckBox.TitleLocation` | … | … |
| `CheckBox.ThreeState` | … | … |
| `CheckBox.Enabled` | … | … |
| `CheckBox.TextColor` | … | … |
| `CheckBox.BackColor` | … | … |
| `CheckBox.TransparentBack` | … | … |
| `CheckBox.BorderStyle` | … | … |
| `CheckBox.BorderColor` | … | … |
| `CheckBox.FieldBackColor` | … | … |
| `CheckBox.FieldTextColor` | … | … |
| `CheckBox.HorizontalAlign` | … | … |
| `CheckBox.VerticalAlign` | … | … |
| `CheckBox.Tooltip` | … | … |
| `CheckBox.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §7. Переключатель — `RadioButton`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `RadioButton.Caption` | … | … |
| `RadioButton.SelectionValue` | … | … |
| `RadioButton.Font` | … | … |
| `RadioButton.TitleLocation` | … | … |
| `RadioButton.Enabled` | … | … |
| `RadioButton.TextColor` | … | … |
| `RadioButton.BackColor` | … | … |
| `RadioButton.TransparentBack` | … | … |
| `RadioButton.BorderStyle` | … | … |
| `RadioButton.BorderColor` | … | … |
| `RadioButton.FieldBackColor` | … | … |
| `RadioButton.FieldTextColor` | … | … |
| `RadioButton.HorizontalAlign` | … | … |
| `RadioButton.VerticalAlign` | … | … |
| `RadioButton.Tooltip` | … | … |
| `RadioButton.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §8. Табличное поле — `Table`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Table.Footer` | … | … |
| `Table.HeaderHeight` | … | … |
| `Table.FooterHeight` | … | … |
| `Table.HeaderFont` | … | … |
| `Table.FooterFont` | … | … |
| `Table.HeaderBackColor` | … | … |
| `Table.HeaderTextColor` | … | … |
| `Table.FooterBackColor` | … | … |
| `Table.FooterTextColor` | … | … |
| `Table.Font` | … | … |
| `Table.BackColor` | … | … |
| `Table.TextColor` | … | … |
| `Table.FieldBackColor` | … | … |
| `Table.FieldTextColor` | … | … |
| `Table.BorderColor` | … | … |
| `Table.BorderStyle` | … | … |
| `Table.HorizontalLines` | … | … |
| `Table.VerticalLines` | … | … |
| `Table.UseAlternationRowColor` | … | … |
| `Table.AlternationRowBackColor` | … | … |
| `Table.SelectionBackColor` | … | … |
| `Table.SelectionTextColor` | … | … |
| `Table.HorizontalScrollBar` | … | … |
| `Table.VerticalScrollBar` | … | … |
| `Table.InitialTreeView` | … | … |
| `Table.SelectionMode` | … | … |
| `Table.RowSelectionMode` | … | … |
| `Table.FixedLeft` | … | … |
| `Table.FixedRight` | … | … |
| `Table.ReadOnly` | … | … |
| `Table.Enabled` | … | … |
| `Table.TransparentBack` | … | … |
| `Table.Tooltip` | … | … |
| `Table.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §9. Колонка табличного поля — `TableColumn`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `TableColumn.Header` | … | … |
| `TableColumn.Width` | … | … |
| `TableColumn.Visible` | … | … |
| `TableColumn.ReadOnly` | … | … |
| `TableColumn.Editor` | … | … |
| `TableColumn.ValueType` | … | … |
| `TableColumn.HeaderFont` | … | … |
| `TableColumn.HeaderTextColor` | … | … |
| `TableColumn.HeaderBackColor` | … | … |
| `TableColumn.HeaderHorizontalAlign` | … | … |
| `TableColumn.HorizontalAlignInColumn` | … | … |
| `TableColumn.HeaderPicture` | … | … |
| `TableColumn.RowsPictures` | … | … |
| `TableColumn.FooterText` | … | … |
| `TableColumn.FooterFont` | … | … |
| `TableColumn.FooterBackColor` | … | … |
| `TableColumn.FooterTextColor` | … | … |
| `TableColumn.FieldBackColor` | … | … |
| `TableColumn.FieldTextColor` | … | … |
| `TableColumn.TextFont` | … | … |
| `TableColumn.Hyperlink` | … | … |
| `TableColumn.MarkNegatives` | … | … |
| `TableColumn.CheckBoxThreeState` | … | … |
| `TableColumn.ShowHierarchy` | … | … |
| `TableColumn.Enabled` | … | … |
| `TableColumn.Tooltip` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §10. Панель (закладки) — `Panel`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Panel.ShowTabs` | … | … |
| `Panel.BackColor` | … | … |
| `Panel.TextColor` | … | … |
| `Panel.BorderColor` | … | … |
| `Panel.BorderStyle` | … | … |
| `Panel.Font` | … | … |
| `Panel.TransparentBack` | … | … |
| `Panel.Picture` | … | … |
| `Panel.PanelPictureLocation` | … | … |
| `Panel.PictureSize` | … | … |
| `Panel.ScrollPageMode` | … | … |
| `Panel.SeparateByPage` | … | … |
| `Panel.Enabled` | … | … |
| `Panel.Tooltip` | … | … |
| `Panel.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §11. Страница панели — `PanelPage`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `PanelPage.Title` | … | … |
| `PanelPage.Visible` | … | … |
| `PanelPage.Enabled` | … | … |
| `PanelPage.TitlePicture` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §12. Группа (рамка) — `UsualGroup`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `UsualGroup.Caption` | … | … |
| `UsualGroup.BorderStyle` | … | … |
| `UsualGroup.Font` | … | … |
| `UsualGroup.TextColor` | … | … |
| `UsualGroup.BackColor` | … | … |
| `UsualGroup.BorderColor` | … | … |
| `UsualGroup.TransparentBack` | … | … |
| `UsualGroup.Enabled` | … | … |
| `UsualGroup.Tooltip` | … | … |
| `UsualGroup.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §13. Командная панель — `CommandBar`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `CommandBar.AutoFill` | … | … |
| `CommandBar.HorizontalAlign` | … | … |
| `CommandBar.Orientation` | … | … |
| `CommandBar.BackColor` | … | … |
| `CommandBar.ButtonBackColor` | … | … |
| `CommandBar.ButtonTextColor` | … | … |
| `CommandBar.Font` | … | … |
| `CommandBar.BorderColor` | … | … |
| `CommandBar.BorderStyle` | … | … |
| `CommandBar.TransparentBack` | … | … |
| `CommandBar.Secondary` | … | … |
| `CommandBar.Enabled` | … | … |
| `CommandBar.Tooltip` | … | … |
| `CommandBar.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §14. Кнопка командной панели — `CommandBarButton`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `CommandBarButton.kind` | … | … |
| `CommandBarButton.Caption` | … | … |
| `CommandBarButton.representation` | … | … |
| `CommandBarButton.checked` | … | … |
| `CommandBarButton.action` | … | … |
| `CommandBarButton.Picture` | … | … |
| `CommandBarButton.DefaultButton` | … | … |
| `CommandBarButton.Enabled` | … | … |
| `CommandBarButton.Tooltip` | … | … |
| `CommandBarButton.Shortcut` | … | … |
| `CommandBarButton.ButtonOrder` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §15. Картинка (декорация) — `PictureDecoration`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `PictureDecoration.Picture` | … | … |
| `PictureDecoration.PictureSize` | … | … |
| `PictureDecoration.Zoomable` | … | … |
| `PictureDecoration.Hyperlink` | … | … |
| `PictureDecoration.NonselectedPictureText` | … | … |
| `PictureDecoration.UseContextMenu` | … | … |
| `PictureDecoration.BackColor` | … | … |
| `PictureDecoration.BorderColor` | … | … |
| `PictureDecoration.BorderStyle` | … | … |
| `PictureDecoration.TransparentBack` | … | … |
| `PictureDecoration.TextColor` | … | … |
| `PictureDecoration.Font` | … | … |
| `PictureDecoration.Enabled` | … | … |
| `PictureDecoration.Tooltip` | … | … |
| `PictureDecoration.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §16. Поле табличного документа — `SpreadsheetDocumentField`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `SpreadsheetDocumentField.ShowGrid` | … | … |
| `SpreadsheetDocumentField.ShowHeaders` | … | … |
| `SpreadsheetDocumentField.ShowGroups` | … | … |
| `SpreadsheetDocumentField.FixedBackground` | … | … |
| `SpreadsheetDocumentField.HorizontalScrollBar` | … | … |
| `SpreadsheetDocumentField.VerticalScrollBar` | … | … |
| `SpreadsheetDocumentField.ReadOnly` | … | … |
| `SpreadsheetDocumentField.BorderColor` | … | … |
| `SpreadsheetDocumentField.BorderStyle` | … | … |
| `SpreadsheetDocumentField.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §17. Календарь — `CalendarField`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `CalendarField.FieldBackColor` | … | … |
| `CalendarField.FieldTextColor` | … | … |
| `CalendarField.ButtonBackColor` | … | … |
| `CalendarField.SelectionBackColor` | … | … |
| `CalendarField.SelectionTextColor` | … | … |
| `CalendarField.HolidayColor` | … | … |
| `CalendarField.ShowCurrentDate` | … | … |
| `CalendarField.CalendarNavigation` | … | … |
| `CalendarField.SelectionMode` | … | … |
| `CalendarField.Font` | … | … |
| `CalendarField.BorderColor` | … | … |
| `CalendarField.BorderStyle` | … | … |
| `CalendarField.TransparentBack` | … | … |
| `CalendarField.Enabled` | … | … |
| `CalendarField.Tooltip` | … | … |
| `CalendarField.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §18. Поле выбора — `ChoiceField`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `ChoiceField.ChoiceButton` | … | … |
| `ChoiceField.ClearButton` | … | … |
| `ChoiceField.OpenButton` | … | … |
| `ChoiceField.SpinButton` | … | … |
| `ChoiceField.ChoiceList` | … | … |
| `ChoiceField.ChoiceButtonPicture` | … | … |
| `ChoiceField.Picture` | … | … |
| `ChoiceField.ChoiceListWidth` | … | … |
| `ChoiceField.ChoiceListHeight` | … | … |
| `ChoiceField.ReadOnly` | … | … |
| `ChoiceField.Font` | … | … |
| `ChoiceField.FieldBackColor` | … | … |
| `ChoiceField.FieldTextColor` | … | … |
| `ChoiceField.ButtonBackColor` | … | … |
| `ChoiceField.ButtonTextColor` | … | … |
| `ChoiceField.BorderColor` | … | … |
| `ChoiceField.BorderStyle` | … | … |
| `ChoiceField.TransparentBack` | … | … |
| `ChoiceField.Enabled` | … | … |
| `ChoiceField.Tooltip` | … | … |
| `ChoiceField.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §19. Индикатор — `ProgressBar`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `ProgressBar.ShowPercent` | … | … |
| `ProgressBar.ShowStyle` | … | … |
| `ProgressBar.Orientation` | … | … |
| `ProgressBar.MinValue` | … | … |
| `ProgressBar.MaxValue` | … | … |
| `ProgressBar.Step` | … | … |
| `ProgressBar.BackColor` | … | … |
| `ProgressBar.BorderColor` | … | … |
| `ProgressBar.BorderStyle` | … | … |
| `ProgressBar.Font` | … | … |
| `ProgressBar.Enabled` | … | … |
| `ProgressBar.Tooltip` | … | … |
| `ProgressBar.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §20. Полоса регулирования — `TrackBar`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `TrackBar.Orientation` | … | … |
| `TrackBar.MarkingAppearance` | … | … |
| `TrackBar.MarkingStep` | … | … |
| `TrackBar.Step` | … | … |
| `TrackBar.LargeStep` | … | … |
| `TrackBar.MinValue` | … | … |
| `TrackBar.MaxValue` | … | … |
| `TrackBar.BackColor` | … | … |
| `TrackBar.TextColor` | … | … |
| `TrackBar.ButtonBackColor` | … | … |
| `TrackBar.BorderColor` | … | … |
| `TrackBar.BorderStyle` | … | … |
| `TrackBar.TransparentBack` | … | … |
| `TrackBar.Enabled` | … | … |
| `TrackBar.Tooltip` | … | … |
| `TrackBar.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §21. Список значений — `ListBox`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `ListBox.ShowCheckBox` | … | … |
| `ListBox.ShowPicture` | … | … |
| `ListBox.FieldBackColor` | … | … |
| `ListBox.FieldTextColor` | … | … |
| `ListBox.ReadOnly` | … | … |
| `ListBox.BorderColor` | … | … |
| `ListBox.BorderStyle` | … | … |
| `ListBox.Enabled` | … | … |
| `ListBox.Tooltip` | … | … |
| `ListBox.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §22. Разделитель — `Splitter`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `Splitter.Orientation` | … | … |
| `Splitter.BackColor` | … | … |
| `Splitter.BorderColor` | … | … |
| `Splitter.BorderStyle` | … | … |
| `Splitter.Enabled` | … | … |
| `Splitter.Tooltip` | … | … |
| `Splitter.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §23. Поле текстового документа — `TextDocumentField`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `TextDocumentField.Font` | … | … |
| `TextDocumentField.BackColor` | … | … |
| `TextDocumentField.TextColor` | … | … |
| `TextDocumentField.ReadOnly` | … | … |
| `TextDocumentField.BorderColor` | … | … |
| `TextDocumentField.BorderStyle` | … | … |
| `TextDocumentField.Enabled` | … | … |
| `TextDocumentField.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §24. Поле HTML-документа — `HTMLDocumentField`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `HTMLDocumentField.BorderColor` | … | … |
| `HTMLDocumentField.BorderStyle` | … | … |
| `HTMLDocumentField.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §25. Поле графической схемы — `GraphicalSchemaField`

### Структура HTML

> Точные теги и классы. Пример: `<div class="c t-InputField"><span class="val">…</span><span class="tb">…</span></div>`.
> Какие классы обязательны, какие добавляются по свойствам.

```html
…
```

### CSS

> Точные правила для этого класса (все селекторы и все свойства). Ссылайтесь на переменные из §0.

```css
…
```

### Свойства → правила

> Одна строка на каждый ID. В «условие» — значение в XML; в «действие» — что делать: CSS-свойство / добавить класс /
> атрибут / вложенный SVG. Если свойство не отображается — `не отображать` и причина. Если тега нет — отдельная строка «нет тега».

| ID | условие | действие в HTML/CSS |
|---|---|---|
| `GraphicalSchemaField.ReadOnly` | … | … |
| `GraphicalSchemaField.BackColor` | … | … |
| `GraphicalSchemaField.BorderColor` | … | … |
| `GraphicalSchemaField.BorderStyle` | … | … |
| `GraphicalSchemaField.TransparentBack` | … | … |
| `GraphicalSchemaField.Enabled` | … | … |
| `GraphicalSchemaField.Visible` | … | … |

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

## §26. Что не охвачено и что мы предлагаем изменить

> Свободный текст: свойства, которых нет в каталоге, но которые видны в 1С; предложения по упрощению.
