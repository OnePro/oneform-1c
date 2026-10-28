# История `pack` и `history restore` — живая проверка

**Дата:** 28.10.2026 (09:50–10:00). **База:** тестовая файловая «Бух.» (`ONEFORM_BUH_DB`), пользователь `admin`, платформа 8.3.27.2342; один экземпляр 1С, перед каждым запуском `taskkill /F /IM 1cv8.exe`. **Объект:** `Catalogs/Контрагенты` (4 формы: `ФормаСписка`, `ФормаЭлемента`, `ФормаСпискаКопия`, `ФормаСпискаКопия2`). **Бинарник:** `target/release/oneform.exe` собран из рабочего дерева с историей.

Исходное состояние: частичная выгрузка объекта и его 4 форм из базы (`db-dump-xml`, `-Mode Partial`) побайтно совпала с копией выгрузки `target/a1-live/cfgx` (`Form.bin` и `Forms/*.xml` всех форм, `Контрагенты.xml`).

## Ход

| Шаг | Команда / действие | Результат |
|---|---|---|
| 1 | копия `cfgx/Catalogs/Контрагенты` (без `src/`) в `target/night/hl/cfg`, `oneform extract Catalogs/Контрагенты` | `src/Forms/ФормаЭлемента/Form.xml` |
| 2 | в `Form.xml` формы `ФормаЭлемента` заголовок формы `ru`: «Контрагенты» → «Контрагенты (история)» | — |
| 3 | `oneform pack Catalogs/Контрагенты --dry-run` ([pack-dry.txt](files/pack-history/pack-dry.txt)) | `WOULD PACK`, `edit …: form caption`, истории нет |
| 4 | `oneform pack Catalogs/Контрагенты` ([pack.txt](files/pack-history/pack.txt)) | `PACKED`, строка `history 20261028-095256-ФормаЭлемента` |
| 5 | `oneform history Catalogs/Контрагенты` ([history-list.txt](files/pack-history/history-list.txt)) | один снимок, `pack` |
| 6 | частичная загрузка (`load.sh`, список из ВСЕХ 4 форм и файла объекта, UTF-8 с BOM: [list-partial-load.txt](files/pack-history/list-partial-load.txt); `src/` и `.oneform/` в список не входят), `UpdateDBCfg` | «Обновление конфигурации успешно завершено» |
| 7 | открыть форму и снять дамп свойств (`live.py Справочники.Контрагенты ФормаЭлемента first`), [dump-packed.txt.gz](files/pack-history/dump-packed.txt.gz) | `READY`, 6 441 строка, `ФОРМА.Заголовок=Строка\|Контрагенты (история)` |
| 8 | `oneform history restore Catalogs/Контрагенты <id> --dry-run` ([restore-dry.txt](files/pack-history/restore-dry.txt)), затем без `--dry-run` ([restore.txt](files/pack-history/restore.txt)) | `RESTORED`, новый снимок `20261028-095438-ФормаЭлемента` (`reason: restore`, состояние до отката), `src/ refreshed`; `Form.bin` побайтно равен исходному из `cfgx`, заголовок в `src` снова «Контрагенты» |
| 9 | та же частичная загрузка, затем дамп ([dump-restored.txt.gz](files/pack-history/dump-restored.txt.gz)) | `READY`; `diff` дампов «после правки» и «после отката»: отличаются только 2 строки `ФОРМА.Заголовок` (`pre:` и `post:`) |
| 10 | частичная выгрузка тех же объектов из базы и `diff -r` с выгрузкой до эксперимента ([base-proof.txt](files/pack-history/base-proof.txt)) | пусто: **база возвращена в исходное состояние** (13 файлов, `Form.bin` и XML всех форм) |

Из дампов удалена строка `@object|…` (название организации из базы), остальное без изменений.

## Дерево снимка

[snapshot-tree.txt](files/pack-history/snapshot-tree.txt) (размеры в байтах, пути относительно `Catalogs/Контрагенты/.oneform/history`):

```
20261028-095256-ФормаЭлемента/            # pack
    before/Form.bin  before/Form.xml  before/FormModule.bsl
    after/Form.xml   after/FormModule.bsl
    manifest.json    # files/pack-history/manifest-pack.json
20261028-095438-ФормаЭлемента/            # restore: состояние до отката
    before/Form.bin  before/Form.xml  before/FormModule.bsl
    manifest.json    # files/pack-history/manifest-restore.json
```

Каталог `.oneform` объекта здесь одновременно хранит витрину (`index.html`, `forms/`): перестроение витрины историю не стирает (проверено и тестом `history_survives_the_viewer_rebuild_when_the_object_is_the_root`).

## Не подтверждено

* Дамп свойств открытой формы не включает проверку, что форма «сохраняется в Конфигураторе»; Конфигуратор после загрузки не открывался.
* Дамп «до правки» запускатором не снимался (база до начала была в исходном состоянии, но в 1С она не открывалась): равенство «исходному» доказано побайтно по `Form.bin`/XML выгрузки (шаги 8 и 10) и дампом после отката; различие «правка» ↔ «откат» ограничено заголовком формы.
* Правка заголовка — единственная проверенная в 1С правка; сам пакет `pack` на других правках проверен тестами (`tests/history.rs`), не в 1С.
* Формы `.epf` в 1С не проверялись (история и откат `.epf` — только тесты).
