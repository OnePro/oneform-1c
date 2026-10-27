# add-form-attributes-and-value-tables — таблица значений с колонками

**Дата:** 27.10.2026 (07:37). **База:** внешняя обработка `ext3.epf`, открыта в тестовой базе `target/db-real` (так задано в `scripts/1c-e2e/epf_dump.py`). **Изменение OpenSpec:** `add-form-attributes-and-value-tables`, п. 1.3.

## Команды

```
oneform extract <каталог обработки>; форма ФормаВТ: <Attributes> с реквизитом ValueTable «Таблица», <Table> «ТаблицаДанных» с колонками Наименование (String 50), Сумма (Number 12,2), Дата (Date); oneform pack
python scripts/1c-e2e/epf_dump.py ext3.epf a1-ext3 ФормаВТ
```

## Результат

[Дамп](files/add-form-attributes-and-value-tables/processor-value-table-after.txt): форма открывается; `ТаблицаДанных` (8,40; 330×150) с `Данные=Таблица`; три колонки с `Данные` = `Наименование`, `Сумма`, `Дата`, `ТекстШапки` = имя, `Ширина` 15, `Редактор` «Поле ввода» у каждой.

## Не подтверждено артефактами

* **Типы колонок** (String, Number, Date): дамп колонок не содержит `ТипЗначения`; тип виден лишь в [`Form.xml`, выгруженном повторным `extract` из собранной обработки](files/add-form-attributes-and-value-tables/processor-value-table-reextracted-Form.xml) (String 50, Number 12,2, Date) — это доказывает, что `pack` записал типы, а не то, что 1С их применила.
* Реквизиты формы простых типов (String, Number, Date, Boolean, Ref) и сквозная привязка поля к ним со значением в `ПриОткрытии` — **нет артефакта** (долг V6); поля `Сумма` и `Флажок` в [`add-new-form-command`](add-new-form-command.md) привязаны к реквизитам объекта, не формы.
* Ссылочные и составные колонки, колонки существующей таблицы значений — не делаются (B10).
