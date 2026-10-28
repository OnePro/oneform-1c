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
Caption|P|draw|текст .form-title (22px, жёлтый градиент) только для выбранного языка, иначе имя формы; настоящей рамки окна 1С нет|{G01}: рамка окна и заголовок с иконкой, кнопки «развернуть/закрыть»|-
ShowTabs|P|tabsHtml|закладки корня через tabsHtml; проверяется только code≠0, member/type игнорируются|-|None
CaptionPicture|I|-|игнорируется (иконка в заголовке окна не рисуется)|{G01}: иконка формы слева в заголовке окна|нет
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
Footer|D|tableBox|строка подвала .tf (20px) снизу, пустые ячейки|-|false
HeaderHeight|I|-|игнорируется (шапка всегда 20px)|-|1
FooterHeight|I|-|игнорируется (подвал всегда 20px)|-|1
HeaderFont|I|-|игнорируется (шапка обычным шрифтом)|-|шрифт таблицы
FooterFont|I|-|игнорируется|-|шрифт таблицы
HeaderBackColor|I|-|игнорируется (градиент --head-from→--head-to)|{G01}: шапка — вертикальный светлый градиент #FAFAF0→#F2EBCC, разделители колонок #BFB999|-
HeaderTextColor|I|-|игнорируется|-|-
FooterBackColor|I|-|игнорируется|-|-
FooterTextColor|I|-|игнорируется|-|-
=Font
=BackColor
=TextColor
=FieldBackColor
=FieldTextColor
=BorderColor
=BorderStyle
HorizontalLines|I|-|игнорируется (линии между строками всегда есть)|{G01}: тонкие линии строк #CBD2D6|true
VerticalLines|I|-|игнорируется (вертикальных линий в строках нет)|-|true
UseAlternationRowColor|I|-|игнорируется (чередование цвета строк)|-|false
AlternationRowBackColor|I|-|игнорируется|-|-
SelectionBackColor|I|-|игнорируется (выбранная строка не показывается)|{G01}: текущая строка — синяя заливка с белым текстом|системный синий
SelectionTextColor|I|-|игнорируется|-|белый
HorizontalScrollBar|I|-|игнорируется (полоса 16px рисуется всегда)|{G01}: горизонтальная полоса внизу|Auto
VerticalScrollBar|I|-|игнорируется (вертикальная полоса рисуется всегда)|{G01}: вертикальная полоса справа|Auto
InitialTreeView|I|-|игнорируется (дерево рисуется как плоская таблица)|{G01}: дерево групп слева: узел «+» и значок папки|-
SelectionMode|I|-|игнорируется|-|Single
RowSelectionMode|I|-|игнорируется (выделение строки или ячейки)|-|Row
FixedLeft|I|-|игнорируется (закреплённые колонки слева)|-|0
FixedRight|I|-|игнорируется|-|0
=ReadOnly
=Enabled
=TransparentBack
=Tooltip
=Visible
"""

ROWS['TableColumn'] = """
Header|D|tableBox|текст `.colh`; пустой заголовок заменяется именем колонки; обрезается по ширине|{G06}: «Автошина», «Нач. периода» — подписи слева; {G01}: значок сортировки «▲» у активной колонки|-
Width|D|parse|flex-basis и width в px; 0 или нет тега → 75|{G01}|75
Visible|D|tableBox|скрытая колонка не рисуется|-|true
ReadOnly|D|tableBox|в «Конфигураторе»: редактируемая колонка = значок «ab» + <Поле ввода>, ReadOnly = пустая ячейка (.cell.ro)|-|false
Editor|S|inspectCol|только в inspector; флажки в колонках не рисуются|-|InputField
ValueType|S|inspectCol|только в inspector; образец значения по типу не рисуется|-|-
HeaderFont|I|-|игнорируется|-|шрифт таблицы
HeaderTextColor|I|-|игнорируется|-|-
HeaderBackColor|I|-|игнорируется|-|-
HeaderHorizontalAlign|I|-|игнорируется (подпись всегда слева)|-|Auto
HorizontalAlignInColumn|I|-|игнорируется|-|Auto
HeaderPicture|I|-|игнорируется|-|нет
RowsPictures|I|-|игнорируется (значки в строках: папка/элемент)|{G01}: значки папок и элементов в первой колонке|нет
FooterText|S|inspectCol|читается, но в подвале не рисуется|-|-
FooterFont|I|-|игнорируется|-|-
FooterBackColor|I|-|игнорируется|-|-
FooterTextColor|I|-|игнорируется|-|-
=FieldBackColor
=FieldTextColor
TextFont|I|-|игнорируется (шрифт текста ячеек)|-|шрифт таблицы
Hyperlink|I|-|игнорируется (ячейки-ссылки)|-|false
MarkNegatives|I|-|игнорируется|-|false
CheckBoxThreeState|I|-|игнорируется|-|false
ShowHierarchy|I|-|игнорируется (отступ и «+» в колонке дерева)|{G01}: узел «+» в колонке дерева|false
=Enabled
Tooltip|P|tableBox|только title= при наведении на заголовок|-|нет
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
Picture|P|label|без подписи и размер >24px → `.pic-box` с «пейзажем»-заглушкой (максимум 96×110), иначе значок 14px; настоящая картинка не рисуется|{G02}: пустые рамки (картинки не заданы) — тонкая рамка #B3AC86 без заглушки|нет
PictureSize|I|-|игнорируется (растянуть, пропорционально, реальный размер, плитка)|-|-
Zoomable|I|-|игнорируется|-|false
Hyperlink|D|label|подчёркивание + синий цвет|-|false
NonselectedPictureText|I|-|игнорируется (текст, если картинка не выбрана)|-|-
UseContextMenu|I|-|игнорируется|-|true
=BackColor
=BorderColor
=BorderStyle
=TransparentBack
=TextColor
=Font
=Enabled
=Tooltip
=Visible
"""

ROWS['SpreadsheetDocumentField'] = """
ShowGrid|I|-|игнорируется (сетка ячеек не рисуется)|-|true
ShowHeaders|I|-|игнорируется (заголовки строк/колонок)|-|true
ShowGroups|I|-|игнорируется (группировки по краям)|-|true
FixedBackground|I|-|игнорируется|-|false
HorizontalScrollBar|I|-|игнорируется (полосы рисуются всегда: .sdoc-h/.sdoc-v)|-|Auto
VerticalScrollBar|I|-|игнорируется|-|Auto
=ReadOnly
=BorderColor
=BorderStyle
=Visible
"""

ROWS['CalendarField'] = """
=FieldBackColor
=FieldTextColor
=ButtonBackColor
SelectionBackColor|I|-|игнорируется (выделение всегда #316AC5)|-|системный синий
SelectionTextColor|I|-|игнорируется|-|белый
HolidayColor|I|-|игнорируется (выходные всегда #c00)|-|красный
ShowCurrentDate|I|-|игнорируется (подпись «Сегодня» внизу всегда есть)|-|true
CalendarNavigation|I|-|игнорируется (стрелки смены месяца)|-|true
SelectionMode|I|-|игнорируется (одна дата / несколько / интервал)|-|Single
=Font
=BorderColor
=BorderStyle
=TransparentBack
=Enabled
=Tooltip
=Visible
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
ShowPercent|I|-|игнорируется (процент текстом)|-|false
ShowStyle|I|-|игнорируется (Smooth/Broken/BrokenTilt — сплошной/блоки/наклонные блоки)|-|Smooth
Orientation|I|-|игнорируется (всегда горизонтально)|-|Horizontal
MinValue|I|-|игнорируется (заполнение всегда 38%)|-|0
MaxValue|I|-|игнорируется|-|100
Step|I|-|игнорируется|-|1
=BackColor
=BorderColor
=BorderStyle
=Font
=Enabled
=Tooltip
=Visible
"""

ROWS['TrackBar'] = """
Orientation|I|-|игнорируется (всегда горизонтально)|-|Horizontal
MarkingAppearance|I|-|игнорируется (деления: нет / сверху-слева / снизу-справа / с обеих сторон)|-|BottomRight
MarkingStep|I|-|игнорируется (шаг делений; рисуется 9 делений)|-|1
Step|I|-|игнорируется|-|1
LargeStep|I|-|игнорируется|-|5
MinValue|I|-|игнорируется|-|0
MaxValue|I|-|игнорируется|-|10
=BackColor
=TextColor
=ButtonBackColor
=BorderColor
=BorderStyle
=TransparentBack
=Enabled
=Tooltip
=Visible
"""

ROWS['ListBox'] = """
ShowCheckBox|I|-|игнорируется (флажки в строках)|-|false
ShowPicture|I|-|игнорируется (значки в строках)|-|false
=FieldBackColor
=FieldTextColor
=ReadOnly
=BorderColor
=BorderStyle
=Enabled
=Tooltip
=Visible
"""

ROWS['Splitter'] = """
Orientation|I|-|игнорируется (разделитель рисуется пустым)|{G01}: вертикальный разделитель 6px между деревом и списком (едва заметный)|Vertical
=BackColor
=BorderColor
=BorderStyle
=Enabled
=Tooltip
=Visible
"""

ROWS['TextDocumentField'] = """
=Font
=BackColor
=TextColor
=ReadOnly
=BorderColor
=BorderStyle
=Enabled
=Visible
"""

ROWS['HTMLDocumentField'] = """
=BorderColor
=BorderStyle
=Visible
"""

ROWS['GraphicalSchemaField'] = """
=ReadOnly
=BackColor
=BorderColor
=BorderStyle
=TransparentBack
=Enabled
=Visible
"""

# Shared structures: (id, xml, ru, values, default, status, now, ref)
SHARED = {
    'Font': [
        ('face', 'Font@face', 'Имя', 'название шрифта (Tahoma, Arial, Verdana, Courier New…)', 'шрифт формы', 'I', 'в inspector строка Font; на холсте всегда Tahoma', '{G05}'),
        ('size', 'Font@size', 'Размер', 'число, пункты', '8', 'I', 'игнорируется', '-'),
        ('bold', 'Font@bold', 'Жирный', 'true / false', 'false', 'P', 'читается только у Label, CheckBox, RadioButton', '{G06}'),
        ('italic', 'Font@italic', 'Наклонный', 'true / false', 'false', 'I', 'игнорируется', '{G05}'),
        ('underline', 'Font@underline', 'Подчеркивание', 'true / false', 'false', 'I', 'игнорируется', '{G05}'),
        ('strikeout', 'Font@strikeout', 'Зачеркивание', 'true / false', 'false', 'I', 'игнорируется', '{G05}'),
        ('scale', 'Font@scale', 'Масштаб', 'число, %', '100', 'I', 'игнорируется', '-'),
        ('source', 'Font@source', 'Вид', 'bin — шрифт задан в форме; style — из стиля 1С; без атрибута — по умолчанию', '-', 'I', 'игнорируется', '-'),
    ],
    'Color': [
        ('kind', '*Color@kind', 'Вид', 'rgb — прямой цвет; style — элемент стиля 1С (ссылка ref, часто с запасным value)', '-', 'P', 'читается только value у TextColor/BackColor подписей', '-'),
        ('value', '*Color@value', 'Красный/Зеленый/Синий (RGB)', '#RRGGBB', '-', 'P', 'читается только у TextColor/BackColor подписей; у kind=style без value цвет не определяется', '{G06}'),
        ('ref', '*Color@ref', 'Элемент стиля', 'GUID элемента стиля 1С (например, «цвет фона формы»)', '-', 'I', 'игнорируется: имена стилевых цветов в превью не сопоставлены с hex', '-'),
    ],
    'Border': [
        ('type', 'BorderStyle', 'Вид / Рамка', 'None — нет; SingleLine — линия; Frame — рамка', 'зависит от класса', 'P', 'читается только у UsualGroup', '{G06}'),
        ('code', 'BorderStyle@code', 'ТипРамки', 'число: 0 = None, 4 = SingleLine, 6 = Frame', '-', 'I', 'игнорируется', '-'),
    ],
    'Picture': [
        ('kind', 'Picture@kind', 'Вид', '1 — картинка из библиотеки/ресурсов (refGuid); true — «у элемента есть картинка»; нет тега — нет картинки', 'нет', 'P', 'рисуется только как фиксированная иконка на подписях', '{G02}'),
        ('refGuid', 'Picture@refGuid', 'Ссылка', 'GUID картинки в конфигурации; пиксели в Form.xml не лежат', '-', 'I', 'недоступно: изображение подставить нельзя, нужна заглушка', '-'),
    ],
    'Shortcut': [
        ('key', 'Shortcut@key', 'Клавиша', 'код клавиши', 'нет', 'I', 'игнорируется (горячая клавиша на кнопке не показывается)', '-'),
        ('mods', 'Shortcut@mods', 'Alt/Ctrl/Shift', 'число-маска модификаторов', 'нет', 'I', 'игнорируется', '-'),
    ],
}
