# add-remove-elements-small-forms — добавление элемента в малые формы обработки

**Дата:** 26.10.2026 (05:04–05:06). **База:** тестовая обработка `tp-original.epf`, три малые формы, отдельная тестовая база `target/db-real` (`scripts/1c-e2e/epf_dump.py`). Относится к строке реестра «Добавление и удаление элементов» (`agent-readiness.md`).

## Команды

```
scripts/1c-e2e/e2e_small.sh <Форма>     # extract; в Form.xml добавляется <LabelDecoration name="АгентНовая">; pack; дамп в 1С; сравнение с дампом исходной формы
```

## Результат

Дампы «до» (исходная форма) и «после»: во всех трёх формах добавлена ровно одна надпись `АгентНовая` (32 новых строки свойств, `Лево=10`, `Верх=2`, `Ширина=150`), **0 изменённых и 0 пропавших** значений у прежних элементов.

| Форма | До | После |
|---|---|---|
| выбор поиска (`ФормаВыбораПоиска`) | [до](files/add-remove-elements-small-forms/select-search-before.txt.gz) (4 элемента, 227 свойств) | [после](files/add-remove-elements-small-forms/select-search-after-label-added.txt.gz) (5, 259) |
| лицензионное соглашение | [до](files/add-remove-elements-small-forms/license-agreement-before.txt.gz) (3, 76) | [после](files/add-remove-elements-small-forms/license-agreement-after-label-added.txt.gz) (4, 108) |
| поиск документов | [до](files/add-remove-elements-small-forms/document-search-before.txt.gz) (6, 382) | [после](files/add-remove-elements-small-forms/document-search-after-label-added.txt.gz) (7, 414) |

## Не подтверждено артефактами

* Все 26 типов элементов: в живой 1С проверялась надпись; остальные типы — только тесты (`tests/form_patch.rs`, `pack_existing_form_xml.rs`).
* **Удаление** элементов и растягивание окна после добавления: артефактов нет (V9). Наборы правок с удалением среди `e2e-*.txt` есть (метка `rm`: 8 изменённых значений, 32 пропавшие строки), но состав правки не записан — см. [сплошная проверка](property-sweep.md).
