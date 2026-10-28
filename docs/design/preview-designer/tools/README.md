# tools/ — собрать превью формы на своих CSS

Нужны только **Python 3.8+** (стандартная библиотека) и браузер Edge/Chrome. Установка ничего не требует. Все команды выполняются из папки `tools/`; проверены на Windows 10 + Python 3.12 + Edge.

## Что здесь

| Путь | Что это |
|---|---|
| `rebuild_preview.py` | Собирает один HTML-файл превью из `Form.xml` с вашими CSS/JS |
| `build_samples.py` | Собирает все образцы из `samples/` (и PNG-снимки) одной командой |
| `check_spec.py` | Самопроверка возвращаемой папки (`python check_spec.py --dir ../response`) |
| `assets/` | Текущие `form_theme.css`, `form_render.js` (код превью), `hljs_1c_xml.js` (подсветка XML) — копии из репозитория |
| `skeleton/preview_template.rs.txt` | Снимок шаблона страницы (читается скриптом; не редактировать) |
| `samples/` | 5 образцов `Form.xml`: `01-kontr-list` (список: таблица, дерево, командная панель), `02-org-item` (карточка: закладки, поля, картинки), `03-nalogi-radio` (переключатели, флажки, группа), `04-ofor-font` (шрифты флажков), `05-putlist` (документ: вложенные закладки, таблицы) |
| `out/` | Результат сборки (создаётся) |

## Команды

```
# 1. Собрать один образец (результат: out/<имя>.html)
python rebuild_preview.py samples/02-org-item

# 2. Собрать на СВОЁМ CSS: файл целиком вместо стандартного…
python rebuild_preview.py samples/02-org-item -o out/my.html --theme my_theme_full.css
#    …или ваш theme.css поверх стандартного (его правила идут последними и перекрывают)
python rebuild_preview.py samples/02-org-item -o out/my.html --theme-extra ../response/theme.css

# 3. Все образцы + снимки PNG (нужен Edge или Chrome)
python build_samples.py --overlay ../response/theme.css --screenshot

# 4. Проверка своей работы
python check_spec.py --dir ../response
```

Открыть `out/*.html` в браузере. Параметры в адресной строке: `?chrome=0` — только холст формы; `?mode=runtime` — вид «Предприятие» (без образцов значений); `?theme=xp` — запасная палитра; `?page=<имя>` — начальная закладка. Вкладка «Form.xml» показывает исходный XML, боковая панель — свойства выбранного элемента.

Изменения **CSS** подхватываются сразу. Изменения поведения (`assets/form_render.js`) — отдельная задача для нас: опишите в `render-spec.md` / `questions.md`.

## Проверено

На образце `samples/02-org-item` и всех пяти образцах: `python rebuild_preview.py …`, `python build_samples.py --screenshot` и `--overlay` отработали, HTML (200–360 КБ) открывается офлайн, PNG строятся. `rebuild_preview.py` воспроизводит штатный `FormPreview.html` байт-в-байт (проверка `--check` работает, когда рядом лежит штатный `FormPreview.html`).
