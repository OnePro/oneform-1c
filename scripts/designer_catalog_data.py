#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Curated data for gen_designer_catalog.py (what is visually relevant, how the preview handles it today).

Row DSL, one line per property:  Tag|S|fn|now|ref|default|values
  Tag     XML child tag; '@name' = attribute of the element itself (e.g. '@kind'); '=Tag' = take the line from COMMON
  S       D drawn / P partly drawn / I ignored / S shown only in the inspector
  fn      JS function in assets/form_render.js (or '-')
  now     what the preview does now (short, Russian)
  ref     where the 1C look can be seen (gallery image or standards doc); '-' = no reference
          (the row goes to the table «Нужны эталоны из 1С»). {G01}.. are replaced by gallery image names.
  default what the platform assumes when the tag is absent
  values  optional override of the value domain (otherwise enum values from the research catalog / bool)
No '|' inside the fields."""

GALLERY = {
    'G01': '01-kontr-list-1c.png',
    'G02': '02-org-item-1c.png',
    'G04': '04-nalogi-radio-1c.png',
    'G05': '05-ofor-font-1c.png',
    'G06': '06-putlist-1c.png',
}


def _f(s):
    for k, v in GALLERY.items():
        s = s.replace('{%s}' % k, v)
    return s


# class id -> (class id, catalog owner, XML element, Russian name, short description)
CLASSES = [
    ('Form', None, 'Form', 'Форма (корень Form.xml)', 'Окно формы целиком: размер, заголовок, закладки верхнего уровня.'),
    ('InputField', 'InputField', 'InputField', 'Поле ввода', 'Поле со значками выбора справа. Самый частый видимый элемент.'),
    ('LabelDecoration', 'LabelDecoration', 'LabelDecoration', 'Надпись', 'Текст на форме; может быть гиперссылкой, цветным, с картинкой.'),
    ('Button', 'Button', 'Button', 'Кнопка (самостоятельная)', 'Обычная кнопка на форме, не в командной панели.'),
    ('CheckBox', 'CheckBox', 'CheckBox', 'Флажок', 'Квадратик с подписью.'),
    ('RadioButton', 'RadioButton', 'RadioButton', 'Переключатель', 'Круглая кнопка с подписью.'),
    ('Table', 'Table', 'Table', 'Табличное поле', 'Таблица со столбцами; шапка, подвал, полосы прокрутки.'),
    ('TableColumn', 'TableColumn', 'Column', 'Колонка табличного поля', 'Столбец таблицы (внутри Table/Columns).'),
    ('Panel', 'Panel', 'Panel', 'Панель (закладки)', 'Набор страниц с закладками.'),
    ('PanelPage', 'PanelPage', 'Page', 'Страница панели', 'Одна закладка панели с содержимым.'),
    ('UsualGroup', 'UsualGroup', 'UsualGroup', 'Группа (рамка)', 'Рамка или линия с заголовком вокруг группы элементов.'),
    ('CommandBar', 'CommandBar', 'CommandBar', 'Командная панель', 'Полоса кнопок над формой или таблицей.'),
    ('CommandBarButton', 'CommandBarButton', 'Button@Buttons', 'Кнопка командной панели', 'Кнопка внутри командной панели (Action/Submenu/Separator).'),
    ('PictureDecoration', 'PictureField', 'PictureDecoration', 'Картинка (декорация)', 'Рисунок на форме.'),
    ('SpreadsheetDocumentField', 'SpreadsheetDocumentField', 'SpreadsheetDocumentField', 'Поле табличного документа', 'Поле для печатных форм/отчётов (ячейки, сетка).'),
    ('CalendarField', 'CalendarField', 'CalendarField', 'Календарь', 'Календарь выбора даты.'),
    ('ChoiceField', 'ChoiceField', 'ChoiceField', 'Поле выбора', 'Выпадающий список.'),
    ('ProgressBar', 'ProgressBar', 'ProgressBar', 'Индикатор', 'Полоса прогресса.'),
    ('TrackBar', 'TrackBar', 'TrackBar', 'Полоса регулирования', 'Ползунок.'),
    ('ListBox', 'ListBox', 'ListBox', 'Список значений', 'Список строк с пометками.'),
    ('Splitter', 'Splitter', 'Splitter', 'Разделитель', 'Перетаскиваемая граница между областями.'),
    ('TextDocumentField', 'TextDocumentField', 'TextDocumentField', 'Поле текстового документа', 'Многострочный текст с прокруткой.'),
    ('HTMLDocumentField', 'HTMLDocumentField', 'HTMLDocumentField', 'Поле HTML-документа', 'Встроенный браузер.'),
    ('GraphicalSchemaField', 'GraphicalSchemaField', 'GraphicalSchemaField', 'Поле графической схемы', 'Рисуемая схема.'),
]

COMMON = {
    'Visible': 'Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true',
    'Enabled': 'Enabled|I|-|игнорируется: недоступный элемент выглядит как обычный|-|true',
    'TransparentBack': 'TransparentBack|I|-|игнорируется (фон рисуется по палитре класса)|-|false',
    'BackColor': 'BackColor|I|-|игнорируется|-|-',
    'TextColor': 'TextColor|I|-|игнорируется|-|-',
    'BorderColor': 'BorderColor|I|-|игнорируется (цвет рамки всегда var(--border))|-|-',
    'FieldBackColor': 'FieldBackColor|I|-|игнорируется|-|-',
    'FieldTextColor': 'FieldTextColor|I|-|игнорируется|-|-',
    'ButtonBackColor': 'ButtonBackColor|I|-|игнорируется|-|-',
    'ButtonTextColor': 'ButtonTextColor|I|-|игнорируется|-|-',
    'Font': 'Font|I|-|игнорируется: везде Tahoma 8pt (в inspector только строка «Font»)|-|шрифт формы (Tahoma 8)',
    'BorderStyle': 'BorderStyle|I|-|игнорируется|-|-',
    'Tooltip': 'Tooltip|I|-|не показывается; при наведении виден только name элемента (title=)|-|нет',
    'HorizontalAlign': 'HorizontalAlign|I|-|игнорируется|-|Auto',
    'VerticalAlign': 'VerticalAlign|I|-|игнорируется|-|Top',
    'Picture': 'Picture|I|-|игнорируется (картинки никогда не рисуются)|-|нет',
    'PictureSize': 'PictureSize|I|-|игнорируется|-|-',
    'ReadOnly': 'ReadOnly|I|-|игнорируется|-|false',
}

ROWS = {}

ROWS['Form'] = """
Width|D|draw|ширина #canvas, .formwrap = ширина+2|-|-
Height|D|draw|высота #canvas|-|-
Caption|D|draw|текст `.form-title` выбранного языка, иначе имя формы (Form@name); вне холста, при chrome=0 не выводится|{G01}: рамка окна и заголовок с иконкой, кнопки «развернуть/закрыть»|-
ShowTabs|P|tabsHtml|закладки корня через tabsHtml; проверяется только code≠0, member/type игнорируются|-|None
CaptionPicture|D|draw|значок 16×16 перед текстом `.form-title`|{G01}: иконка формы слева в заголовке окна|нет
"""

ROWS['InputField'] = """
ChoiceButton|P|inputField|кнопка .tb 15px: «...» / календарь (Date) / калькулятор (Number) / «T» (составной тип); если тега нет — рисуется «...» по умолчанию|{G02}: кнопки «...», календарь; {G06}: калькулятор|-
ClearButton|P|inputField|кнопка «×», но читается только если есть ChoiceButton|{G04}: кнопка «х» справа от поля|false
OpenButton|P|inputField|кнопка-лупа, читается только если есть ChoiceButton|{G06}: лупа справа от поля «Организация»|false
DropListButton|I|-|игнорируется (кнопка ▾ у поля со списком не рисуется)|-|false
SpinButton|I|-|игнорируется (стрелки вверх/вниз не рисуются)|-|false
AutoMarkIncomplete|D|inputField|красная точечная линия под полем (.mark)|{G02}: красный пунктир по нижней границе пустого обязательного поля «Фискальный код»|false
ReadOnly|I|-|игнорируется (поле только для просмотра рисуется как обычное белое)|{G06}: поля «Пробег общий» — фон как у формы, рамка осталась, кнопка калькулятора серая|false
=Enabled
MultiLine|I|-|игнорируется (многострочное поле рисуется одной строкой)|-|false
PasswordMode|I|-|игнорируется (звёздочки не показываются)|-|false
MarkNegatives|I|-|игнорируется (отрицательные числа красным)|-|false
HorizontalAlign|I|-|игнорируется; числа всегда справа (.num), остальное слева|-|Auto
VerticalAlign|I|-|игнорируется|-|Top
VerticalScrollBar|I|-|игнорируется|-|Auto
HorizontalScrollBar|I|-|игнорируется|-|Auto
ChoiceListMode|I|-|игнорируется (выбор только из списка без ввода с клавиатуры)|-|false
Format|I|-|игнорируется (форматная строка; образец значения фиксированный)|-|-
Mask|I|-|игнорируется|-|-
ValueType|P|inputField|образец значения по типу: `0`, `0,00`, ` . . : :`, ` . .` (только в режиме «Конфигуратор»); длина и ссылочные типы не учитываются|{G02}: «.  .» в поле даты|-
=Font
=TextColor
=BackColor
=FieldBackColor
=FieldTextColor
=ButtonBackColor
=ButtonTextColor
=BorderColor
=BorderStyle
=TransparentBack
Picture|I|-|игнорируется|-|нет
ChoiceButtonPicture|I|-|игнорируется (картинка на кнопке выбора)|-|нет
=Tooltip
=Visible
"""

ROWS['LabelDecoration'] = """
Caption|D|label|текст в .cap; в «Конфигураторе» пустая подпись показывается как <<Имя>>|{G02}: подписи «Наименование:», «Код:»|-
HorizontalAlign|D|label|Center/Right → justify-content + text-align; Left/Auto — слева|-|Auto
VerticalAlign|I|-|игнорируется (текст всегда по центру высоты)|-|Top
Hyperlink|D|label|цвет #0054E3 + подчёркивание + .link|-|false
TextColor|P|label|inline color:, только атрибут value; kind=style без value не разрешается|{G06}: цвет подписей|-
BackColor|P|label|inline background:, только value|-|-
TransparentBack|I|-|в «Предприятии» подпись прозрачная, в «Конфигураторе» фон формы; значение тега не читается|-|false
Font|P|label|читается только bold (.cap.b); italic/underline/strikeout/face/size игнорируются|{G06}: «Номер:», «Организация:» жирные|шрифт формы
Picture|P|label|одна фиксированная иконка 14px (.ic) вместо настоящей картинки|-|нет
PictureSize|I|-|игнорируется|-|-
LabelPictureLocation|I|-|игнорируется (картинка всегда слева)|-|Left
BorderStyle|I|-|игнорируется|-|-
=BorderColor
Format|I|-|игнорируется|-|-
MarkNegatives|I|-|игнорируется|-|false
ScrollingText|I|-|игнорируется (бегущая строка)|-|DontUse
=Enabled
=Tooltip
=Visible
"""

ROWS['Button'] = """
Caption|D|ctl|текст в .cap, по центру; если пусто — пустая кнопка (имя не подставляется)|{G02}: кнопка «Изменить...»|-
=Visible
=Enabled
Picture|I|-|игнорируется|-|нет
PictureLocation|I|-|игнорируется|-|Left
PictureSize|I|-|игнорируется|-|-
Font|I|-|игнорируется (даже bold)|-|шрифт формы
ButtonBackColor|I|-|игнорируется (фон кнопки всегда var(--cmdbar-bg))|-|-
ButtonTextColor|I|-|игнорируется|-|-
BorderColor|I|-|игнорируется|-|-
HorizontalAlign|I|-|игнорируется (текст всегда по центру)|-|Center
VerticalAlign|I|-|игнорируется|-|Center
MultiLine|I|-|игнорируется (перенос текста)|-|false
DefaultButton|I|-|игнорируется (кнопка по умолчанию без выделения)|-|false
MenuMode|I|-|игнорируется (стрелка меню на кнопке)|-|false
Shortcut|I|-|игнорируется|-|нет
=TransparentBack
=Tooltip
"""

ROWS['CheckBox'] = """
Caption|D|ctl|13×13 `.box` + `.cap`; флажок никогда не отмечен|{G04}: «Сумма включает НДС»; {G05}: четыре флажка|-
Font|P|label|только bold; italic/underline/strikeout игнорируются|{G05}: «Жирный», «Наклонный», «Подчеркнутый», «Зачеркнутый» — подпись в соответствующем начертании|шрифт формы
TitleLocation|I|-|игнорируется (подпись всегда справа)|-|Right
ThreeState|I|-|игнорируется (третье состояние)|-|false
=Enabled
=TextColor
=BackColor
=TransparentBack
=BorderStyle
=BorderColor
=FieldBackColor
=FieldTextColor
HorizontalAlign|I|-|игнорируется|-|Auto
VerticalAlign|I|-|игнорируется|-|Top
=Tooltip
=Visible
"""

ROWS['RadioButton'] = """
Caption|D|ctl|круг 13px `.rad` + `.cap`; всегда не выбран|{G04}: два переключателя, выбранный — с точкой|-
SelectionValue|I|-|игнорируется (какой из переключателей выбран)|{G04}: у выбранного круг с точкой|-
Font|P|label|только bold|-|шрифт формы
TitleLocation|I|-|игнорируется (подпись всегда справа)|-|Right
=Enabled
=TextColor
=BackColor
=TransparentBack
=BorderStyle
=BorderColor
=FieldBackColor
=FieldTextColor
HorizontalAlign|I|-|игнорируется|-|Auto
VerticalAlign|I|-|игнорируется|-|Top
=Tooltip
=Visible
"""

ROWS['Table'] = """
Footer|D|tableBox|подвал `.tf` над горизонтальной полосой, ячейки `.fd` по ширинам колонок с FooterText|-|false
HeaderHeight|D|tableBox|высота шапки 20+(N-1)*13 px (`--head-h`, `.mh`: перенос по словам, по центру)|-|1
FooterHeight|D|tableBox|высота подвала 20+(N-1)*13 px|-|1
HeaderFont|D|tableBox|классы/стиль шрифта на все `.colh`|-|шрифт таблицы
FooterFont|D|tableBox|классы/стиль шрифта на все `.fd`|-|шрифт таблицы
HeaderBackColor|D|tableBox|сплошной фон на `.th` и `.colh` (без градиента)|{G01}: шапка — вертикальный светлый градиент #FAFAF0→#F2EBCC, разделители колонок #BFB999|-
HeaderTextColor|D|tableBox|цвет текста `.colh`|-|-
FooterBackColor|D|tableBox|сплошной фон на `.tf` и `.fd`|-|-
FooterTextColor|D|tableBox|цвет текста `.fd`|-|-
Font|D|tableBox|классы `.b/.f-i/.f-u/.f-s` и font-family/size на корне (ячейки и шапка без HeaderFont)|-|шрифт формы (Tahoma 8)
BackColor|D|tableBox|фон области без строк (корень)|-|-
TextColor|D|tableBox|цвет текста `.cell` (FieldTextColor важнее)|-|-
FieldBackColor|D|tableBox|фон строк `.tr` (кроме выделенной)|-|-
FieldTextColor|D|tableBox|цвет текста `.cell`|-|-
BorderColor|D|tableBox|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
HorizontalLines|D|tableBox|`false` → `.no-hl`: линии между строками прозрачные|{G01}: тонкие линии строк #CBD2D6|true
VerticalLines|D|tableBox|`false` → без `.vl`: вертикальных линий в строках нет (по умолчанию есть)|-|true
UseAlternationRowColor|D|tableBox|designer: вторая строка-образец `.alt`|-|false
AlternationRowBackColor|D|tableBox|переменная `--alt-bg` на корне|-|-
SelectionBackColor|D|tableBox|фон выделенной строки-образца `.tr.rsel` (designer; основной список формы или таблица с Selection*)|{G01}: текущая строка — синяя заливка с белым текстом|системный синий
SelectionTextColor|D|tableBox|цвет текста выделенной строки-образца|-|белый
HorizontalScrollBar|D|tableBox|`DontUse`/`false` — полосы нет; иначе `.sb` рисуется всегда|{G01}: горизонтальная полоса внизу|Auto
VerticalScrollBar|D|tableBox|`UseAlways`/`true` → `.vsb` со стрелками; иначе не рисуется|{G01}: вертикальная полоса справа|Auto
InitialTreeView|D|tableBox|в первой колонке (или с ShowHierarchy) строки-образца: узел «+» и папка|{G01}: дерево групп слева: узел «+» и значок папки|-
SelectionMode|I|-|не отображается: влияет только на поведение|-|Single
RowSelectionMode|D|tableBox|`Cell`: синяя только первая редактируемая ячейка (`.cell.csel`), фон строки белый|-|Row
FixedLeft|D|tableBox|первые N колонок `.fixed` (тень-разделитель)|-|0
FixedRight|D|tableBox|последние N колонок `.fixed`|-|0
ReadOnly|D|tableBox|designer: все ячейки-образцы пустые|-|false
Enabled|D|ctl|класс `.dis`: серые шапка и текст ячеек|-|true
TransparentBack|D|ctl|класс `.tr-bg` (прозрачный фон)|-|false
Tooltip|D|titleOf|текст подсказки выбранного языка в title= корня (без тега — имя элемента); на холсте подсказки нет|-|нет
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['TableColumn'] = """
Header|D|tableBox|текст `.colh` (ellipsis); пустой заголовок — пустая ячейка шапки, имя только в title=|{G06}: «Автошина», «Нач. периода» — подписи слева; {G01}: значок сортировки «▲» у активной колонки|-
Width|D|parse|flex:0 0 Npx и width на `.colh`/`.cell`/`.fd`; 0 или нет тега → 75|{G01}|75
Visible|D|tableBox|скрытая колонка не рисуется; в режиме «показывать скрытые» — с opacity .35|-|true
ReadOnly|D|tableBox|designer: ячейка-образец пустая (`.cell.ro`), иначе значок «ab» + <Поле ввода>|-|false
Editor|D|tableBox|CheckBox → флажок по центру; ChoiceField → «ab» + <Поле выбора>; InputField/нет → <Поле ввода>; прочие — пусто|-|InputField
ValueType|D|tableBox|Number → образец `0` / `0,00` по fractionDigits (`.num`)|-|-
HeaderFont|D|tableBox|классы/стиль шрифта на `.colh` колонки|-|шрифт таблицы
HeaderTextColor|D|tableBox|цвет текста `.colh` колонки|-|-
HeaderBackColor|D|tableBox|фон `.colh` колонки|-|-
HeaderHorizontalAlign|D|tableBox|`.ha-Center` / `.ha-Right` на `.colh`|-|Auto
HorizontalAlignInColumn|D|tableBox|`.num` / `.ctr` на `.cell` (число — вправо по умолчанию)|-|Auto
HeaderPicture|D|tableBox|значок 16×16 в `.colh` перед текстом (без подписи — по центру)|-|нет
RowsPictures|D|tableBox|значок `icon-item` по центру ячейки-образца (и у колонки «Картинка»)|{G01}: значки папок и элементов в первой колонке|нет
FooterText|D|tableBox|текст `.fd` колонки при Footer=true|-|-
FooterFont|D|tableBox|классы/стиль шрифта на `.fd` колонки|-|-
FooterBackColor|D|tableBox|фон `.fd` колонки|-|-
FooterTextColor|D|tableBox|цвет текста `.fd` колонки|-|-
FieldBackColor|D|tableBox|фон `.cell` колонки|-|-
FieldTextColor|D|tableBox|цвет текста `.cell` колонки|-|-
TextFont|D|tableBox|классы/стиль шрифта на `.cell` колонки|-|шрифт таблицы
Hyperlink|D|tableBox|текст образца в `.lnk` (синий, подчёркнутый)|-|false
MarkNegatives|I|-|не отображается: в превью нет данных|-|false
CheckBoxThreeState|D|tableBox|флажок-образец `.mix`|-|false
ShowHierarchy|D|tableBox|перед текстом образца узел «+» и папка|{G01}: узел «+» в колонке дерева|false
Enabled|D|tableBox|серый текст `.colh`/`.cell`/`.fd` колонки|-|true
Tooltip|P|tableBox|только title= заголовка колонки|-|нет
"""

ROWS['Panel'] = """
ShowTabs|P|tabsHtml|положение закладок определяется по геометрии первой страницы (Top≥Left → сверху, Left>Top → слева); значения Top/Bottom/Left/Right/Multiline/Scrolling игнорируются|{G02}: закладки сверху, справа стрелки прокрутки при нехватке места; {G06}: вложенная панель|Top
=BackColor
=TextColor
=BorderColor
=BorderStyle
=Font
=TransparentBack
Picture|I|-|игнорируется (картинка панели)|-|нет
PanelPictureLocation|I|-|игнорируется|-|-
PictureSize|I|-|игнорируется|-|-
ScrollPageMode|I|-|игнорируется (прокручиваемые страницы)|-|false
SeparateByPage|I|-|игнорируется|-|false
=Enabled
=Tooltip
=Visible
"""

ROWS['PanelPage'] = """
Title|D|tabsHtml|текст .ptab (18px, 19px у активной); при ≥7 страниц ужимается до max-width:92px|{G02}: активная закладка светлая с оранжевой полоской сверху (#FADFA3), неактивные — #EBE6CC с рамкой #CCC6AD|-
=Visible
=Enabled
TitlePicture|I|-|игнорируется (значок на закладке)|-|нет
"""

ROWS['UsualGroup'] = """
Caption|D|groupBox|`.gt`: всегда жирный #9F6500 на фоне формы, поверх линии|{G06}: «Расход топлива:» — жирный коричневый заголовок над линией|-
BorderStyle|D|groupBox|None/Frame/SingleLine → `.bs-Frame` (рамка, скругление 3px) / `.bs-SingleLine` (линия снизу); без тега: h>20 → Frame, иначе SingleLine|{G06}: SingleLine — линия под заголовком|-
Font|P|groupBox|игнорируется: заголовок всегда жирный (в 1С заголовок группы жирный в 99% форм, docs/form-design-standards.md §10)|{G06}|жирный
TextColor|I|-|игнорируется (цвет заголовка всегда #9F6500)|{G06}|#9F6500
BackColor|I|-|игнорируется|-|-
=BorderColor
=TransparentBack
=Enabled
=Tooltip
=Visible
"""

ROWS['CommandBar'] = """
AutoFill|D|autoItems|добавляет стандартные кнопки (Действия, Добавить, Копировать, Изменить, Удалить…) с угаданными значками|{G01}: полоса «Действия ▾ + цветные значки + Перейти ▾ + справка»|false
HorizontalAlign|D|commandBar|Right → кнопки вправо (.right)|{G02}: нижняя панель «OK Записать Закрыть» прижата вправо|Left
Orientation|I|-|игнорируется (вертикальная панель)|-|Horizontal
=BackColor
=ButtonBackColor
=ButtonTextColor
=Font
=BorderColor
=BorderStyle
=TransparentBack
Secondary|I|-|игнорируется|-|false
=Enabled
=Tooltip
=Visible
"""

ROWS['CommandBarButton'] = """
@kind|D|cbtn|Action — кнопка; Submenu — кнопка со стрелкой ▼ (подменю только во всплывающем окне инспектора); Separator — линия .csep|{G06}: «Действия ▾», разделители между группами|Action
Caption|D|cbtn|подпись .cap (если нет — имя кнопки)|{G06}: «Параметры», «Печать ▾», «Заполнить»|-
@representation|D|cbtn|Picture / Text / PictureText / Auto → иконка и/или подпись|{G06}: значки без подписи и значок+подпись («Заполнить»)|Auto
@checked|D|cbtn|`.on` — нажатая кнопка|{G01}: нажатая кнопка в панели (подсвечена)|false
@action|P|btnIcon|значок подбирается по таблице STD_ICON или по ключевым словам имени; настоящая картинка кнопки не рисуется|{G01}: настоящие цветные значки (зелёный плюс, карандаш, красный крест)|-
Picture|P|btnIcon|не используется, значок угадывается|{G01}|нет
DefaultButton|P|cbtn|жирная подпись только для Standard:141 или имени, оканчивающегося на ОК|{G06}: «OK» жирным в нижней панели|false
=Enabled
Tooltip|P|cbtn|только title= при наведении|-|нет
Shortcut|I|-|игнорируется|-|нет
ButtonOrder|I|-|игнорируется|-|-
"""

ROWS['PictureDecoration'] = """
Picture|D|pictureBox|без подписи и >24px: `.pic-box` (пустая рамка) или `.pic-box.pic-has` с заглушкой 16×16; маленькая/с подписью — значок в `label`|{G02}: пустые рамки (картинки не заданы) — тонкая рамка #B3AC86 без заглушки|нет
PictureSize|P|pictureBox|AutoSize/RealSize/Tile → заглушка в левом верхнем углу (`.ps-tl`); остальное — по центру|-|-
Zoomable|I|-|не отображается: поведение|-|false
Hyperlink|D|label|подпись синяя подчёркнутая (`.link`)|-|false
NonselectedPictureText|D|pictureBox|текст `.nptxt` по центру пустой рамки, если картинки нет|-|-
UseContextMenu|I|-|не отображается: поведение|-|true
BackColor|D|pictureBox|`background` на корне|-|-
BorderColor|D|pictureBox|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
TransparentBack|D|ctl|класс `.tr-bg` (прозрачный фон)|-|false
TextColor|D|label|цвет подписи `.cap`|-|-
Font|D|label|шрифт подписи `.cap`|-|шрифт формы (Tahoma 8)
Enabled|D|ctl|класс `.dis`: заглушка бледная, подпись серая|-|true
Tooltip|D|titleOf|текст подсказки выбранного языка в title= корня (без тега — имя элемента); на холсте подсказки нет|-|нет
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['SpreadsheetDocumentField'] = """
ShowGrid|D|sdocBox|`false` — белое поле; иначе сетка 64×17 (`.grid`)|-|true
ShowHeaders|D|sdocBox|`true` → заголовки колонок `.shd-h` и строк `.shd-v`|-|true
ShowGroups|D|sdocBox|`false` — без полосы группировок `.shd-g` (9 px)|-|true
FixedBackground|I|-|не отображается: поведение|-|false
HorizontalScrollBar|D|sdocBox|`DontUse`/`false` — без полосы `.sdoc-h`; поле ниже 40 px — без полос|-|Auto
VerticalScrollBar|D|sdocBox|`DontUse`/`false` — без полосы `.sdoc-v`|-|Auto
ReadOnly|I|-|не отображается: вид не меняется|-|false
BorderColor|D|sdocBox|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['CalendarField'] = """
FieldBackColor|D|calendarBox|`background` корня|-|-
FieldTextColor|D|calendarBox|цвет чисел `.cal-gr`|-|-
ButtonBackColor|D|calendarBox|фон шапки `.cal-hd`|-|-
SelectionBackColor|D|calendarBox|`--sel-bg` на корне (выбранный день)|-|системный синий
SelectionTextColor|D|calendarBox|`--sel-text` на корне|-|белый
HolidayColor|D|calendarBox|`--holiday` на корне (выходные)|-|красный
ShowCurrentDate|D|calendarBox|`false` или поле меньше 140×110 — без подвала (`.nocur`)|-|true
CalendarNavigation|D|calendarBox|`false` — стрелки скрыты (`.nonav`)|-|true
SelectionMode|D|calendarBox|Single — один день; Multiple — два несмежных; Interval — три подряд (средний `.rng`)|-|Single
Font|P|calendarBox|жирность/курсив/гарнитура на корне; размер не доходит до дней недели и подвала|-|шрифт формы (Tahoma 8)
BorderColor|D|calendarBox|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
TransparentBack|D|ctl|класс `.tr-bg` (прозрачный фон)|-|false
Enabled|D|ctl|класс `.dis`: серые числа и шапка, выделение #C9C4AB|-|true
Tooltip|D|titleOf|текст подсказки выбранного языка в title= корня (без тега — имя элемента); на холсте подсказки нет|-|нет
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['ChoiceField'] = """
ChoiceButton|I|-|игнорируется: всегда одна кнопка ▾|-|true
ClearButton|I|-|игнорируется|-|false
OpenButton|I|-|игнорируется|-|false
SpinButton|I|-|игнорируется|-|false
ChoiceList|S|inspectCol|только в inspector; список значений не показывается|-|-
ChoiceButtonPicture|I|-|игнорируется|-|нет
=Picture
ChoiceListWidth|I|-|игнорируется (ширина выпадающего списка)|-|-
ChoiceListHeight|I|-|игнорируется (высота выпадающего списка)|-|-
=ReadOnly
=Font
=FieldBackColor
=FieldTextColor
=ButtonBackColor
=ButtonTextColor
=BorderColor
=BorderStyle
=TransparentBack
=Enabled
=Tooltip
=Visible
"""

ROWS['ProgressBar'] = """
ShowPercent|D|progressBox|`<span class="prg-pct">38%</span>` по центру|-|false
ShowStyle|D|progressBox|`Broken`/`BrokenTilt` → `.ps-*` блоки|-|Smooth
Orientation|D|progressBox|`Vertical` → `.vert`: заполнение снизу вверх|-|Horizontal
MinValue|I|-|не отображается: доля образца 38 %|-|0
MaxValue|I|-|не отображается: доля образца 38 %|-|100
Step|I|-|не отображается: поведение|-|1
BackColor|D|progressBox|`background` корня|-|-
BorderColor|D|progressBox|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
Font|D|progressBox|шрифт `.prg-pct`|-|шрифт формы (Tahoma 8)
Enabled|D|ctl|класс `.dis`: заполнение серое, opacity .5|-|true
Tooltip|D|titleOf|текст подсказки выбранного языка в title= корня (без тега — имя элемента); на холсте подсказки нет|-|нет
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['TrackBar'] = """
Orientation|D|trackBox|`Vertical` → `.vert`|-|Horizontal
MarkingAppearance|D|trackBox|DontShow / TopLeft / Both / BottomRight (по умолчанию под линией)|-|BottomRight
MarkingStep|D|trackBox|число делений = (Max−Min)/шаг+1, не больше 21|-|1
Step|I|-|не отображается: поведение|-|1
LargeStep|I|-|не отображается: поведение|-|5
MinValue|D|trackBox|участвует в числе делений|-|0
MaxValue|D|trackBox|число делений (по умолчанию 10 → 11)|-|10
BackColor|D|trackBox|`background` корня|-|-
TextColor|D|trackBox|цвет делений|-|-
ButtonBackColor|D|trackBox|фон ползунка|-|-
BorderColor|D|trackBox|`border-color` линии `.trk-line`|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
TransparentBack|D|ctl|класс `.tr-bg` (прозрачный фон)|-|false
Enabled|D|ctl|класс `.dis`: ползунок и деления opacity .5|-|true
Tooltip|D|titleOf|текст подсказки выбранного языка в title= корня (без тега — имя элемента); на холсте подсказки нет|-|нет
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['ListBox'] = """
ShowCheckBox|D|listBox|designer: флажок `.box` в строках-образцах|-|false
ShowPicture|D|listBox|designer: значок `icon-item` в строках-образцах|-|false
FieldBackColor|D|listBox|`background` корня|-|-
FieldTextColor|D|listBox|цвет текста строк-образцов|-|-
ReadOnly|I|-|не отображается: вид не меняется|-|false
BorderColor|D|listBox|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
Enabled|D|ctl|класс `.dis`: серые строки, выделение #C9C4AB|-|true
Tooltip|D|titleOf|текст подсказки выбранного языка в title= корня (без тега — имя элемента); на холсте подсказки нет|-|нет
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['Splitter'] = """
Orientation|D|splitterBox|Vertical/Horizontal/Auto (по пропорциям); ручка из точек по центру, не рисуется при длине < 20 px|{G01}: вертикальный разделитель 6px между деревом и списком (едва заметный)|Vertical
BackColor|D|splitterBox|`--spl-bg`, класс `.colored`|-|-
BorderColor|I|-|игнорируется: у разделителя нет рамки|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
Enabled|D|ctl|класс `.dis`: ручка opacity .4|-|true
Tooltip|D|titleOf|текст подсказки выбранного языка в title= корня (без тега — имя элемента); на холсте подсказки нет|-|нет
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['TextDocumentField'] = """
Font|D|textDocBox|шрифт текста-образца `.txt-ph`|-|шрифт формы (Tahoma 8)
BackColor|D|textDocBox|`background` корня|-|-
TextColor|D|textDocBox|цвет текста-образца|-|-
ReadOnly|I|-|не отображается: вид не меняется|-|false
BorderColor|D|textDocBox|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
Enabled|D|ctl|класс `.dis`: фон формы, серый текст-образец|-|true
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['HTMLDocumentField'] = """
BorderColor|D|ctl|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

ROWS['GraphicalSchemaField'] = """
ReadOnly|I|-|не отображается: вид не меняется|-|false
BackColor|D|ctl|`background-color` корня|-|-
BorderColor|D|ctl|`border-color` на корне|-|-
BorderStyle|D|ctl|класс `.bd-<тип>`: None — без рамки, SingleLine/Frame/Single3D/Double/Rounded/Overline/Underline/Embossed/Indented|-|-
TransparentBack|D|ctl|класс `.tr-bg` (прозрачный фон)|-|false
Enabled|D|ctl|класс `.dis`: сетка бледнее|-|true
Visible|D|vis|скрытый элемент не рисуется; в режиме «показывать скрытые» — полупрозрачный с пунктирной рамкой (.hid)|-|true
"""

# Shared structures: (id, xml, ru, values, default, status, now, ref)
SHARED = {
    'Font': [
        ('face', 'Font@face', 'Имя', 'название шрифта (Tahoma, Arial, Verdana, Courier New…)', 'шрифт формы', 'D', 'font-family на носителе текста (fontSpec)', '{G05}'),
        ('size', 'Font@size', 'Размер', 'число, пункты', '8', 'D', 'font-size в pt на носителе текста', '-'),
        ('bold', 'Font@bold', 'Жирный', 'true / false', 'false', 'D', 'класс `.b` (false — font-weight:normal); у всех классов со шрифтом', '{G06}'),
        ('italic', 'Font@italic', 'Наклонный', 'true / false', 'false', 'D', 'класс `.f-i`', '{G05}'),
        ('underline', 'Font@underline', 'Подчеркивание', 'true / false', 'false', 'D', 'класс `.f-u`', '{G05}'),
        ('strikeout', 'Font@strikeout', 'Зачеркивание', 'true / false', 'false', 'D', 'класс `.f-s` (вместе с `.f-u` — оба)', '{G05}'),
        ('scale', 'Font@scale', 'Масштаб', 'число, %', '100', 'D', 'font-size: calc(Npt * scale / 100)', '-'),
        ('source', 'Font@source', 'Вид', 'bin — шрифт задан в форме; style — из стиля 1С; без атрибута — по умолчанию', '-', 'I', 'игнорируется', '-'),
    ],
    'Color': [
        ('kind', '*Color@kind', 'Вид', 'rgb — прямой цвет; style — элемент стиля 1С (ссылка ref, часто с запасным value)', '-', 'D', 'rgb и style с value → цвет; style без value и auto → цвет по умолчанию роли (colorCss)', '-'),
        ('value', '*Color@value', 'Красный/Зеленый/Синий (RGB)', '#RRGGBB', '-', 'D', 'inline-цвет (color/background/border-color) по роли свойства; у kind=style без value цвет не задаётся', '{G06}'),
        ('ref', '*Color@ref', 'Элемент стиля', 'GUID элемента стиля 1С (например, «цвет фона формы»)', '-', 'I', 'игнорируется: имена стилевых цветов в превью не сопоставлены с hex', '-'),
    ],
    'Border': [
        ('type', 'BorderStyle', 'Вид / Рамка', 'None — нет; SingleLine — линия; Frame — рамка', 'зависит от класса', 'D', 'класс `.bd-<тип>` (у UsualGroup — `.bs-*`); у всех классов с рамкой (borderType)', '{G06}'),
        ('code', 'BorderStyle@code', 'ТипРамки', 'число: 0 = None, 4 = SingleLine, 6 = Frame', '-', 'D', 'используется, если нет type: 0 → None, 4 → SingleLine, 6 → Frame', '-'),
    ],
    'Picture': [
        ('kind', 'Picture@kind', 'Вид', '1 — картинка из библиотеки/ресурсов (refGuid); true — «у элемента есть картинка»; нет тега — нет картинки', 'нет', 'D', 'заглушка `icon-picture` 16×16 (подписи, кнопки, поля, закладки, заголовок формы, колонки); у PictureDecoration — `.pic-has`', '{G02}'),
        ('refGuid', 'Picture@refGuid', 'Ссылка', 'GUID картинки в конфигурации; пиксели в Form.xml не лежат', '-', 'I', 'недоступно: изображение подставить нельзя, нужна заглушка', '-'),
    ],
    'Shortcut': [
        ('key', 'Shortcut@key', 'Клавиша', 'код клавиши', 'нет', 'I', 'игнорируется (горячая клавиша на кнопке не показывается)', '-'),
        ('mods', 'Shortcut@mods', 'Alt/Ctrl/Shift', 'число-маска модификаторов', 'нет', 'I', 'игнорируется', '-'),
    ],
}
