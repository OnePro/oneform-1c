#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Generate docs/design/preview-designer/render-spec.skeleton.md: the mandatory skeleton of render-spec.md
(one section per element class with a pre-filled ID column for every catalog row).

  python scripts/gen_render_spec_skeleton.py [--out docs/design/preview-designer/render-spec.skeleton.md]"""
import argparse, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import designer_catalog_data as D  # noqa: E402
import gen_designer_catalog as G  # noqa: E402

REPO = os.path.dirname(HERE)

HEAD = """# render-spec.md — как собрать каждую часть формы 1С в HTML/CSS

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
%s

"""

SECTION = """## §%d. %s — `%s`

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
%s

### Состояния

> normal / hover / focus / disabled / read-only / selected / pressed / checked — только те, что бывают у этого элемента в 1С.
> Для каждого: CSS-селектор и отличия от normal; ссылка на образец в `style-sheet.html` (`data-xml`/`data-state`).

| состояние | селектор | отличия | образец на листе |
|---|---|---|---|
| normal | … | … | … |

### Особые случаи

> Пустая подпись, очень узкий/низкий элемент, длинный текст, скрытый элемент, комбинации свойств, поведение при `Visible=false`.

"""

TAIL = """## §%d. Что не охвачено и что мы предлагаем изменить

> Свободный текст: свойства, которых нет в каталоге, но которые видны в 1С; предложения по упрощению.
"""


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--out', default=os.path.join(REPO, 'docs/design/preview-designer/render-spec.skeleton.md'))
    a = ap.parse_args()
    shared = []
    for sname, items in D.SHARED.items():
        for it in items:
            shared.append('| `%s.%s` | … | … |' % (sname, it[0]))
    out = [HEAD % '\n'.join(shared)]
    n = 2
    ids = 0
    for cid, owner, xmlname, ru, desc in D.CLASSES:
        lines = ['| `%s.%s` | … | … |' % (cid, r['tag'].lstrip('@')) for r in G.parse_rows(cid)]
        ids += len(lines)
        out.append(SECTION % (n, ru, cid, '\n'.join(lines)))
        n += 1
    out.append(TAIL % n)
    with open(a.out, 'w', encoding='utf-8', newline='\n') as f:
        f.write(''.join(out))
    print('wrote %s: %d class sections, %d class IDs + %d shared IDs' % (a.out, len(D.CLASSES), ids, len(shared)))


if __name__ == '__main__':
    main()
