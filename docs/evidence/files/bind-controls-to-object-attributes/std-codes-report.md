# Standard-attribute codes `{-N}` of ordinary-form links — runtime ground truth (config «Бух.»)

Source: live 1C 8.3.27.2342 (ordinary application), `Эл.Данные` of every control of the form obtained with `<Менеджер>[Имя].ПолучитьФорму(Форма)`; joined with corpus stream links `root[2][3]` rows `{controlId,{2,{attr},{-N}}}` (K=2, single step) by control id → control name (Form.xml `name`/`id`).

- Forms with negative K=2 steps in the corpus: 250; sections dumped in live 1C: 250 (the 40 selected forms of `forms_sel.lst` are a subset); join rows: 524; confirmed (control found, runtime Данные non-empty): 519.

## Conflicts (same N → different runtime names within a kind)

None.

Runtime `Данные` containing a dot (path through a nested attribute): 0. Rows whose link attribute is not the form's main attribute: 0 (their runtime Данные is still a plain standard name).

## Table: kind, code → runtime name

Columns: code, runtime name, confirming controls, forms, example form.

### Catalogs (`Справочники`) — main-attribute names seen: СправочникОбъект ×200, СправочникОбъектВалюты ×2, ПланВидовРасчетаОбъект ×2

| code | runtime name | controls | forms | example form |
|---|---|---|---|---|
| {-5} | Владелец | 12 | 12 | Справочники.ДоговораФизическихЛиц.ФормаЭлемента |
| {-4} | Родитель | 25 | 25 | Справочники.ВидыОбъектов.ФормаЭлемента |
| {-3} | Наименование | 89 | 89 | Справочники.АвтомобилиОрганизации.ФормаЭлемента |
| {-2} | Код | 78 | 78 | Справочники.АвтомобилиОрганизации.ФормаЭлемента |

### Documents (`Документы`) — main-attribute names seen: ДокументОбъект ×237

| code | runtime name | controls | forms | example form |
|---|---|---|---|---|
| {-3} | Дата | 118 | 118 | Документы.АвансовыйОтчет.ФормаДокумента |
| {-2} | Номер | 119 | 119 | Документы.АвансовыйОтчет.ФормаДокумента |

### ChartsOfAccounts (`ПланыСчетов`) — main-attribute names seen: ПланСчетовОбъект ×12

| code | runtime name | controls | forms | example form |
|---|---|---|---|---|
| {-12} | ВидыСубконто | 2 | 2 | ПланыСчетов.Основной.ФормаСчета |
| {-11} | Забалансовый | 2 | 2 | ПланыСчетов.Основной.ФормаСчета |
| {-10} | Вид | 2 | 2 | ПланыСчетов.Основной.ФормаСчета |
| {-8} | Наименование | 2 | 2 | ПланыСчетов.Основной.ФормаСчета |
| {-7} | Код | 2 | 2 | ПланыСчетов.Основной.ФормаСчета |
| {-6} | Родитель | 2 | 2 | ПланыСчетов.Основной.ФормаСчета |

### ChartsOfCalculationTypes (`ПланыВидовРасчета`) — main-attribute names seen: ПланВидовРасчетаОбъект ×8

| code | runtime name | controls | forms | example form |
|---|---|---|---|---|
| {-3} | Наименование | 4 | 4 | ПланыВидовРасчета.НалогиИОтчисления.ФормаВидаРасчета |
| {-2} | Код | 4 | 4 | ПланыВидовРасчета.НалогиИОтчисления.ФормаВидаРасчета |

### ChartsOfCharacteristicTypes (`ПланыВидовХарактеристик`) — main-attribute names seen: ПланВидовХарактеристикОбъект ×7

| code | runtime name | controls | forms | example form |
|---|---|---|---|---|
| {-11} | ТипЗначения | 1 | 1 | ПланыВидовХарактеристик.ТипыШтрихкодов.ФормаЭлемента |
| {-9} | Наименование | 3 | 3 | ПланыВидовХарактеристик.ВидыКатегорийОсвобождений.ФормаЭлемента |
| {-8} | Код | 3 | 3 | ПланыВидовХарактеристик.ВидыКатегорийОсвобождений.ФормаЭлемента |

### ExchangePlans (`ПланыОбмена`) — main-attribute names seen: ПланОбменаОбъект ×28

| code | runtime name | controls | forms | example form |
|---|---|---|---|---|
| {-10} | НомерПринятого | 7 | 7 | ПланыОбмена.ОбменБухгалтерияРозница.ФормаУзла |
| {-9} | НомерОтправленного | 7 | 7 | ПланыОбмена.ОбменБухгалтерияРозница.ФормаУзла |
| {-3} | Наименование | 7 | 7 | ПланыОбмена.ОбменБухгалтерияРозница.ФормаУзла |
| {-2} | Код | 7 | 7 | ПланыОбмена.ОбменБухгалтерияРозница.ФормаУзла |

### InformationRegisters (`РегистрыСведений`) — main-attribute names seen: РегистрСведенийМенеджерЗаписи ×23

| code | runtime name | controls | forms | example form |
|---|---|---|---|---|
| {-2} | Период | 23 | 23 | РегистрыСведений.ЗапралатныеПроекты.ФормаЗаписи |

## Catalogs: codes vs catalog structure (no dependence found)

| Hierarchical | HierarchyType | Owners | has Code | has Description | codes seen |
|---|---|---|---|---|---|
| false | HierarchyFoldersAndItems | 0 | False | True | -3=Наименование |
| false | HierarchyFoldersAndItems | 0 | True | True | -3=Наименование, -2=Код |
| false | HierarchyFoldersAndItems | 1 | True | True | -5=Владелец, -3=Наименование, -2=Код |
| false | HierarchyFoldersAndItems | 2 | True | True | -5=Владелец, -3=Наименование, -2=Код |
| true | HierarchyFoldersAndItems | 0 | False | True | -4=Родитель, -3=Наименование |
| true | HierarchyFoldersAndItems | 0 | True | True | -4=Родитель, -3=Наименование, -2=Код |
| true | HierarchyFoldersAndItems | 1 | True | True | -5=Владелец, -4=Родитель, -3=Наименование, -2=Код |
| true | HierarchyFoldersAndItems | 2 | False | True | -4=Родитель, -3=Наименование |
| true | HierarchyOfItems | 0 | True | False | -4=Родитель |
| true | HierarchyOfItems | 0 | True | True | -4=Родитель, -3=Наименование, -2=Код |
| true | HierarchyOfItems | 1 | True | True | -4=Родитель, -3=Наименование, -2=Код |

For catalogs the code is fixed per kind (-2 Код, -3 Наименование, -4 Родитель, -5 Владелец); an absent standard attribute simply has no link.

## Root names seen

Main attribute (root) name per kind from the form streams, with runtime check `ТипЗнч(Ф.<имя>)` (roots.out, one form per distinct name):

| kind | main attribute names (confirmed controls) | runtime ТипЗнч |
|---|---|---|
| Catalogs | СправочникОбъект ×200, СправочникОбъектВалюты ×2, ПланВидовРасчетаОбъект ×2 | СправочникОбъект → Справочник объект; СправочникОбъектВалюты → Справочник объект; ПланВидовРасчетаОбъект → План видов расчета объект/Справочник объект |
| Documents | ДокументОбъект ×237 | ДокументОбъект → Документ объект |
| ChartsOfAccounts | ПланСчетовОбъект ×12 | ПланСчетовОбъект → План счетов объект |
| ChartsOfCalculationTypes | ПланВидовРасчетаОбъект ×8 | ПланВидовРасчетаОбъект → План видов расчета объект/Справочник объект |
| ChartsOfCharacteristicTypes | ПланВидовХарактеристикОбъект ×7 | ПланВидовХарактеристикОбъект → План видов характеристик объект |
| ExchangePlans | ПланОбменаОбъект ×28 | ПланОбменаОбъект → План обмена объект |
| InformationRegisters | РегистрСведенийМенеджерЗаписи ×23 | РегистрСведенийМенеджерЗаписи → Регистр сведений менеджер записи |
| list forms | СправочникСписок, ДокументСписок | СправочникСписок → Справочник список; ДокументСписок → Документ список |

Runtime `Данные` of an item control is the path relative to the link's root (`Код`, `Номер`, …) — the root name is not part of it. A catalog form may carry a main attribute with another kind's name (`ПланВидовРасчетаОбъект` on a catalog form is a catalog object at runtime); the name is only a label.

## Versus the current global std_element (src/oof_props.rs)

Global table: -2 Номер, -3 Дата, -4 ПометкаУдаления, -5 Проведен, -6 Родитель, -7 Владелец, -9 Наименование. Runtime: Documents match for -2/-3 only; Catalogs -2 Код (not Номер), -3 Наименование (not Дата), -4 Родитель, -5 Владелец; ChartsOfAccounts -6 Родитель, -7 Код, -8 Наименование, -10 Вид, -11 Забалансовый, -12 ВидыСубконто; ChartsOfCalculationTypes -2 Код, -3 Наименование; ChartsOfCharacteristicTypes -8 Код, -9 Наименование, -11 ТипЗначения; ExchangePlans -2 Код, -3 Наименование, -9 НомерОтправленного, -10 НомерПринятого; InformationRegisters (record manager) -2 Период. The code is per object kind; within Catalogs it does not depend on hierarchy/owners/code/description flags.

## Unconfirmed / not testable

- Link rows without runtime confirmation: 5
  - Справочники.КонтактныеЛицаКонтрагентов.ФормаВводаНового cid=6 {-5}: control-not-in-runtime
  - ПланыОбмена.ПоОрганизации.ФормаУзла cid=18 {-2}: control-not-in-runtime
  - ПланыОбмена.ПоОрганизации.ФормаУзла cid=20 {-3}: control-not-in-runtime
  - ПланыОбмена.ПоОрганизации.ФормаУзла cid=26 {-9}: control-not-in-runtime
  - ПланыОбмена.ПоОрганизации.ФормаУзла cid=28 {-10}: control-not-in-runtime
- Forms not dumped: one exchange-plan node form hangs the client when created; one catalog form fails module init (`ФормаВводаНового`: «Ошибка инициализации модуля»).
- Codes that no corpus form links with a single K=2 step (not confirmable with this corpus): Catalogs other than -2..-5; Documents other than -2,-3 (so Проведен/ПометкаУдаления etc. unconfirmed); ChartsOfAccounts other than -6,-7,-8,-10,-11,-12 (e.g. -9, -13..); ChartsOfCalculationTypes other than -2,-3; ChartsOfCharacteristicTypes other than -8,-9,-11 (e.g. -10); ExchangePlans other than -2,-3,-9,-10; InformationRegisters other than -2. Kinds with no negative K=2 step in the corpus: AccumulationRegisters, AccountingRegisters, CalculationRegisters, DocumentJournals (longer paths only); Tasks/BusinessProcesses do not exist in the config.
- Negative steps inside longer paths (K≥3, ignored in the table): Catalogs K=4 {-2}×2, Catalogs K=4 {-60001}×1, DataProcessors K=3 {-3}×2, DocumentJournals K=4 {-2}×4, DocumentJournals K=4 {-3}×4, DocumentJournals K=4 {-60001}×8, Documents K=3 {-8}×5, Documents K=4 {-2}×1, Documents K=4 {-60001}×1

## Selected forms (40, greedy cover of (kind,code), ≥2 forms per pair when available)

- ПланыСчетов.Основной|ФормаСчета
- ПланыСчетов.Основной2014|ФормаСчета
- Справочники.СпецификацииНоменклатуры|ФормаЭлемента
- ПланыОбмена.ОбменБухгалтерияРозница|ФормаУзла
- ПланыОбмена.ОбменЗарплата3Бухгалтерия3|ФормаУзла
- Справочники.ВидыОбъектов|ФормаЭлемента
- ПланыВидовХарактеристик.ТипыШтрихкодов|ФормаЭлемента
- ПланыВидовРасчета.НалогиИОтчисления|ФормаВидаРасчета
- ПланыВидовРасчета.Начисления|ФормаВидаРасчета
- ПланыВидовХарактеристик.ВидыКатегорийОсвобождений|ФормаЭлемента
- Документы.АвансовыйОтчет|ФормаДокумента
- Документы.АктПереработкиПереданныхТМЦ|ФормаДокумента
- Справочники.ДоговораФизическихЛиц|ФормаЭлемента
- РегистрыСведений.ЗапралатныеПроекты|ФормаЗаписи
- РегистрыСведений.КатегорииЗастрахованногоЛица|ФормаЗаписи
- Справочники.АвтомобилиОрганизации|ФормаЭлемента
- Справочники.АккумуляторныеБатареиИАвтошины|ФормаЭлемента
- Справочники.БазыТаможенныхВыплат|ФормаЭлемента
- Справочники.Банки|ФормаЭлемента
- Справочники.Валюты|ФормаЭлемента
- Справочники.ВариантыПересчета|ФормаЭлемента
- Справочники.ВариантыРасчетаНалогов|ФормаЭлемента
- Справочники.ВидыДоходов|ФормаЭлемента
- Справочники.ВидыНалогов|ФормаВидаРасчета
- Справочники.ВидыНоменклатуры|ФормаЭлемента
- Справочники.ВидыПособий|ФормаЭлемента
- Справочники.ВложенияЭлектронныхПисем|ФормаЭлемента
- Справочники.ВнешниеОбработки|ФормаГруппы
- Справочники.ГрафикиРаботы|ФормаЭлемента
- Справочники.ГруппыПисемЭлектроннойПочты|ФормаЭлемента
- Справочники.ГруппыПользователей|ФормаЭлемента
- Справочники.ГруппыПочтовойРассылки|ФормаГруппы
- Справочники.Договоры|ФормаЭлемента
- Справочники.ДолгосрочныеАктивы|ФормаЭлемента
- Справочники.ДолжностиОрганизаций|ФормаЭлемента
- Справочники.ЕдиницыИзмеренияТМЦ|ФормаЭлемента
- Справочники.ЗарплатныеПроекты|ФормаЭлемента
- Справочники.ЗемельныеУчастки|ФормаЭлемента
- Справочники.Кассы|ФормаЭлемента
- Справочники.КатегорииНоменклатуры|ФормаЭлемента

## Rerun

```
python target/std-codes/scan_corpus.py      # links from corpus streams -> scan.json
python target/std-codes/select_forms.py     # forms_sel.lst / forms_all.lst / meta.json
python target/std-codes/build_launcher.py   # launcher_module.bsl -> std-codes.epf (module swapped inside dump-cfg.epf)
python target/std-codes/run_batch.py target/std-codes/forms_sel.lst target/std-codes/sel.out   # live 1C via schtasks /it
python target/std-codes/join.py && python target/std-codes/report.py
```
Launcher parameter: `/C"batch:<list>|<out>|-"` (list lines `Вид.Имя|Форма`) or `/C"cfg:Вид.Имя|<out>|<Форма>"`. Output: `## Вид.Имя|Форма`, then `Имя|Данные` / `Имя.Колонка|Данные`. run_batch kills a hung client after 90 s without output growth; a hanging form must then be removed from the list and the rest rerun.
