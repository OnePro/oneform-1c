Status: done

# A2 design report: adding, removing and reordering CommandBar buttons

The corpus forms used are under `target/corpus/{ut2,<Бух.>}`. Every survey number below comes from parsing all of their FormStream.bin files with `target/std-codes/brace.py`. I ran the scripts as throwaway Python files in /tmp and deleted them afterwards; no repo files were changed.

## 1. Stream layout

**Panel record.** For a control of type `e69bf21d-…` (oof.rs:48), `info[1] = X`:
- `X[1]` is the record version: 8 means 12 fields, 9 means 14 fields.
- `X[2]` Orientation, `X[3]` AutoFill (0 or 1), `X[4]` HorizontalAlign (2 = Right), `X[5]` Secondary (oof_props.rs:1056-1086, form_patch.rs:6106-6109).
- `X[7]` is the button table, T.
- `X[8..9]` is the root group key. It is always `(b78f2e80-…,4)`: 2847 of 2847 panels in «Бух.», 2881 of 2881 in ut2.
- `X[10..11]` is either `(9d0a2e40 null,0)` or a unique guid with 1. That guid appears nowhere else in the stream. Its meaning is unknown, so it must be preserved as is.
- `meta[2]` is ActionSource: 0 = the form, 4294967295 = none, any other value is a control id (oof.rs:900-906).

**Table T:** `{5, T1, T2, 1, nAct, Action×nAct, nGroups, Group×nGroups}`
- T1 is the panel's table guid. Every button copies it into `b[6]` (18359 of 18359 buttons).
- T2 is the **last allocated id**. Button ids and user sub-menu group codes are both drawn from it. It is never smaller than the largest button id (equal in 947 panels, larger in 1240).
- Actions and buttons pair 1:1. No action is unreferenced, and only 2 ut2 panels share an action. Each action guid appears exactly twice in the whole stream: in its action record and in the button's reference.

**Action record:** `{6|7, actGuid, 1, kindGuid, body, mask, overrides…}`. Version 7 adds one trailing atom.

| kind (oof_props.rs:18-23) | body | used for |
|---|---|---|
| `e1692cc2` Handler | `{3,"Proc",{1,"Proc",text,tooltip,description,picture{3/4,…},shortcut{0,0,0}}}` | action button; the proc name sits at body[1] and body[2][1], always equal |
| `fbe38877` Standard | `{6, srcId, typeGuid, CODE, {1,0/99,groupGuid,groupCode,flag},0,1}` | standard command |
| `abde0c9a` Submenu | `{1, groupGuid, code}` | **separators** use the null group `{1,9d0a2e40…,0}` (6237 cases); user sub-menus use `{1,T1,code}` |
| `c93a51ed` StdSubmenu | `{4, srcGroupGuid, code, {1,99,…},0,0}` | platform sub-menu such as «Действия» (b78f2e80,55) |
| `fb3652fd` OpenMovements | `{1,…}` | «(Список движений)»; 250 in «Бух.» |

The `mask` plus override items are a per-button override block for texts, picture and shortcut. The ordering is only partly decoded. With mask 7, `[6]`, `[7]` and `[8]` hold loc, loc and picture. With mask 14 they hold loc, loc and shortcut. The reader takes the tooltip from `[7]` (oof_props.rs:849-865). Mask 0 means no overrides: 1416 ut2 handlers use it, and so does the donor. That makes mask 0 the safe choice for new records.

**Group:** `{5, keyGuid, keyCode, flag, k, (actGuidRef, Button)×k, Tail}`

**Button:** `{8, "Name", b2, b3, caption, b5, T1, id, 1e2|99, kind 0/1/2, repr, 1, checked, b13, b14, b15}`, 16 fields. ut2 also has version 7 with 13 fields, where `b13..b15` are absent (12208 buttons). Versions never mix inside one panel. The combinations seen are (X8, act6, btn7), (X8, act6, btn8), (X9, act6/7, btn8).

**Tail:** `{pos, nRefs, (T1,id)×nRefs, {nSub, (T1,submenuBtnId,flag)×nSub}}`
- `nSub` lists **all sub-menu buttons of the group, in button order**. This held in every populated case (1098 in «Бух.»). The only exception is 25 ut2 groups whose list is empty.
- `refs` is a subset of the StdSubmenu buttons.
- `pos` is a button index: -1 in 1952 groups, inside the list in 1542, equal to k in 839, larger than k in 13. In root groups it correlates with AutoFill, so it is probably the place where auto-filled standard buttons are inserted. That is a hypothesis. Either way it is an index, so inserting, removing or reordering buttons must shift it.

Groups appear in any order: the root is the last group in 2260 «Бух.» panels and not last in 587. User sub-menu group keys are `(T1, code ≤ T2)` and never collide with a button id.

**Nothing outside the panel depends on the buttons.** Panel geometry is set by the user, and the platform lays the buttons out. One caveat: T1 is copied verbatim when a panel is copied, so a few forms have two panels with the same T1. Each panel is still self-contained.

**Samples:**
1. Three handler buttons: «Бух.» `Catalogs/ВнешниеОбработки/…/ФормаЭлемента` (Form.xml:500, panel `КоманднаяПанельФайлаИсточника`). T2 = 3, nAct = 3, one group `(b78f2e80,4)` with k = 3, tail `{-1,0,{0}}`. Button ids 1, 2, 3. The button order (Заменить, Сохранить, Удалить) differs from the action order. Actions use mask 7 with a picture override `{3,1,{0,guid},…}`.
2. Separators plus the standard «ОсновныеДействияФормы»: `Catalogs/БазыТаможенныхВыплат/…/ФормаЭлемента` (Form.xml:137). T2 = 5, five actions:
   - 141 «OK», runtime text «Записать и закрыть»
   - 140 «Записать»
   - 142 «Закрыть»
   - two separators (abde0c9a with the null group)
   
   The buttons are OK, Разделитель, Сохранить, Разделитель1, Закрыть with ids 1 to 5. Standard body: `{6,0,000…,140,{1,0,357c6a54…,2147483647,0},0,1}`.
3. User sub-menu: the donor `tests/fixtures/forms/DonorFormV2`, panel `КоманднаяПанель2`. T2 = 5. The root group `(b78f2e80,4)` holds НоваяКнопка (id 1) and Подменю (id 2, action `{1,T1,3}`). The sub-group `(T1,3)` holds Действие (4) and Действие1 (5). The root tail is `{-1,0,{1,T1,2,0}}`.
4. Standard sub-menu: `Catalogs/ГруппыПочтовойРассылки/…/ФормаГруппы` (Form.xml:127). Button Подменю has id 111, `b8 = 99`, `b13 = 1`, action c93a51ed pointing to `(b78f2e80,55)`, and root tail `{0,1,T1,111,{1,T1,111,0}}`. Group `(b78f2e80,55)` has flag 1 and holds a separator (60) and a handler (61). At runtime, AutoFill merges standard items into it (target/a1-live/out/doc-after.txt).

**Standard codes and their meaning,** from the most common captions or tooltips in both corpora. Usage counts are given for the most frequent codes:

| codes | meaning |
|---|---|
| 140 / 141 / 142 | Записать (558) / OK, runtime «Записать и закрыть» (572) / Закрыть (1587) |
| 24 / 29 | Провести (245) / Отмена проведения (134) |
| 38 | Пометка удаления |
| 40 | Ввести на основании |
| 53 | Отбор и сортировка |
| 80 / 82 / 83 / 84 | Добавить / Скопировать / Удалить / Изменить |
| 85 / 86 | Вверх / Вниз |
| 88 / 89 | Сортировка по возрастанию / по убыванию |
| 90 | Обновить |
| 91 / 93 | Отбор по значению / Отключить отбор |
| 94 | Закончить редактирование |
| 95 | Выбрать |
| 97 | Настройка списка |
| 99 | Вывести список |
| 6 / 10 | Новая группа |
| 8 | Переместить в группу |
| 9 | Иерархический просмотр |
| 11 | Просмотр по владельцу |
| 20 | Интервал дат |
| 21 | Поиск по номеру |
| 106 / 107 | Представление / Подробно |
| 120 / 121 | Включить все / Выключить все |
| 143 / 144 | Сохранить значения / Восстановить значения |
| 145 / 146 / 147 | Скопировать / Перечитать / Найти в списке |
| 300 | Справка (795) |
| 320 / 321 | Назад / Вперёд |

Codes 1, 2, 23, 27, 31, 42, 44, 65, 71 and 72 have empty captions (platform default text), so their meaning is unknown.

The standard body is not a function of the code alone. `srcId` takes values like 4294967295, 0, 2 or 22. `typeGuid` is zero or, for example, `55c7732c` for 24 and 29. The origin group is NONE, (FORM,4) or (FORM,55). One example: code 142 has 4 common variants.

## 2. Survey

| | «Бух.» | ut2 |
|---|---|---|
| forms / CommandBar controls | 1481 / 2847 | 1525 / 2881 |
| empty panels (nAct = 0) | 660 | 633 |
| buttons | 18359 | 19325 |
| Action / Separator / Submenu (Form.xml) | 10751 / 6271 / 1434 | 11618 / 6241 / 1563 |
| actions: Handler / Standard / abde0c9a / StdSub / Movements | 5477 / 4945 / 6992 / 695 / 250 | 5841 / 5716 / 6564 / 1202 / 0 |
| depth 1 / 2 / 3 | 1936 / 741 / 170 | 1876 / 894 / 111 |
| groups per panel: 1 / 2 / 3+ | 1926 / 606 / 315 | 1871 / 756 / 254 |
| max actions per panel | 85 | 85 |
| AutoFill true / false | 1477 / 1370 | 1371 / 1430 |
| button names not unique within a panel | 364 panels | 376 panels |

Maximum nesting is 3 levels (two nested sub-menus).

**Donor.** It has 4 CommandBars. Two of them, `КоманднаяПанель1` and `КоманднаяПанель3`, are empty. `ОсновныеДействияФормы` holds a handler, a separator and Standard:142. `КоманднаяПанель2` holds handlers plus a user sub-menu. All use X9, act7, btn8. The donor's prototype mechanism (`src/donor.rs:23,213`) clones whole controls only and has no per-button templates.

## 3. Design

**Existing limitations to fix first:**
- **Wrong root group.** `cb_table` takes the **last** group as the root (form_patch.rs:8160), but the reader matches the `X[8..9]` key (oof_props.rs:897-911). In 587 «Бух.» panels the root is not last, e.g. sample 4. There, editing a root button patches a sub-menu group instead. The read-back check probably catches this and the pack fails rather than writing a bad stream, but that is untested.
- **Sub-menu buttons are never patched.** `patch_cb_buttons` (8221-8350) walks only one level.
- **`checked` is required.** `parse_cb_buttons` needs the `checked` attribute (3727); without it the whole `<Buttons>` element falls into the unsupported "rest".

**XML grammar.** Keep the grammar that extract already emits (`<Button kind=…>`) so extract and pack stay symmetric:
- `id` is the identity. A missing `id` means a new button; an `id` absent from the list means removal; list order is the button order; nesting inside `<Buttons>` defines the sub-menu.
- `kind` is Action, Separator or Submenu, default Action.
- `checked` becomes optional, default false.
- `actionKind` is derived when omitted: `action="Standard:N"` gives Standard, any other non-empty value gives Handler.
- A new Submenu without an action becomes a user sub-menu.
- A new StdSubmenu or OpenMovements button is rejected (copying an existing one is allowed).
- Children stay as they are: Caption, Tooltip, Description, Picture, Shortcut, DefaultButton, ButtonOrder.
- A `<Separator/>` shorthand is optional and not recommended.

**Write strategy.** Rebuild the whole `X[7]` text, following the same pattern the form-attribute rewrite uses (form_patch.rs:9857+):
- Kept action and button records are copied verbatim from their source spans, with in-place edits applied.
- New records are built from **per-version templates**:
  - Take the panel's own versions from its siblings.
  - For an empty panel: X9 uses the donor formats (act7/btn8). For X8, (act6, btn8) is the proposal; this needs a live check.
  - Handler action: a fresh guid, the proc name in both slots, text, tooltip and description in body meta, mask 0.
  - Separator: the null-group abde0c9a action with button `{…,"Name",0,1,{1,0},0,T1,id,1e2,2,0,1,0,0,0,0}`.
  - User sub-menu: action `{1,T1,code}` plus a new group `{5,T1,code,0,k,…,{-1,0,{0}}}`.
- The bookkeeping that must be maintained:
  - every group's `k` and the table's `nAct` and `nGroups`;
  - T2 grows by one for each new id or group code;
  - `b[6] = T1`;
  - each tail's sub list is regenerated from the group's sub-menu buttons in order, keeping existing flags and using 0 for new ones;
  - each tail's refs drop removed buttons;
  - each tail's `pos` stays anchored to the same surviving button (removed button → next survivor; `pos == k` → the new k; -1 stays -1).
- Removing a sub-menu deletes its group recursively along with all actions under it.
- Moving a button between levels keeps its action record.
- The root group is located by `X[8..9]`.

**Verification:**
- Re-decode the patched panel with `decode_props` / `read_cb_buttons` and compare it with the requested list, with the ids assigned to new buttons filled in.
- Also check invariants: one action per button and every reference resolves; `T2` is at least every id and code; sub lists match the sub-menu buttons; `pos` is within -1..k; versions are uniform within the panel; every non-root group is reachable from the root.
- Make sure `buttonsSig` is recomputed when carriers are on (oof_props.rs:1012).

**BSL linkage.** The handler is linked by name only: action body[1] and body[2][1]. There is no form event-table entry, and the runtime `Строка(Кн.Действие)` returns the procedure name (a1-live/out/doc-after.txt).
- For a new or renamed handler, look for `Процедура <Proc>(` in FormModule.bsl and **warn** if it is missing.
- Optionally offer a stub `Процедура X(Кнопка) КонецПроцедуры`; the corpus signature is `(Кнопка)`.
- Do not reject: the module may live elsewhere. Today nothing in the code validates handlers.

## 4. Staged plan

| stage | scope | size | testable requirements |
|---|---|---|---|
| S0 | Locate the root by `X[8..9]`; patch sub-menu buttons recursively; make `checked` optional | S, about 100 LOC | Edit a caption in sample 4 (root not last) and in a nested button; read-back equals the request |
| S1 | Flat (root group): add, remove, reorder Handler buttons and separators; T2, counts, tail `pos` and sub list maintained; BSL warning | M, about 500 LOC plus tests | The donor ОсновныеДействияФормы plus «Выполнить», «Заполнить», a separator, then reordered; the empty donor panel plus 2 buttons; removing a separator; round trip with extract shows ids T2+1…; invariants hold; live opening shows the order |
| S2 | User sub-menus: create, remove, nest up to 3 levels, move buttons between levels | M, about 300 LOC | A «Действия» sub-menu with 2 handlers inside a sub-menu; deleting it removes its group and actions |
| S3 | New standard commands: `Standard:N` with a per-code body template mined from the corpus (most frequent variant per code and per kind of action source), plus a code catalogue in docs | M | Add 24 «Провести», 141, 142 to a document panel; live run shows the runtime `Действие` text «Провести» and the button works |
| S4 | Properties of new buttons: Picture, Shortcut, Description, representation, checked, DefaultButton | S | Values readable in the stream and in live 1C |
| S5 (optional) | Copying StdSubmenu buttons; exposing `pos` as a `<StandardButtons/>` marker; override-block (mask) semantics | L, unknown | After live research |

**Risks and unknowns:**
- Meaning of `pos` (AutoFill insert point?). Whether the platform accepts a new X8 empty panel with btn8.
- Meaning of `b2`, `b3` (values 0-15), `b5`, `b8` (99 vs 1e2), `b11`, `b13` on sub-menus, the tail flag, and `X[10..11]`.
- Standard body fields `srcId` (2, 22, …), `typeGuid` and origin group.
- Order of the override block, and whether the existing tooltip reading from `[7]` is right.
- What happens in 1C when a handler procedure is missing (error at open or at click?).
- Duplicate button names are allowed (364 panels have them), so names are not unique keys.

**Live checks («Бух.»):**
1. Open, save and reopen in the Конфигуратор after S1, S2 and S3.
2. Compare runtime button order, texts and `Действие` for each change.
3. AutoFill panels show merged standard buttons at runtime, so compare only the panel's own buttons.
4. Change `pos` on an AutoFill panel and observe where the standard buttons land.
5. A missing handler.

**Launcher:** `target/a1-live/launcher_module.bsl:37-53,88` already dumps nested buttons with Текст, Подсказка, Видимость, Доступность, Пометка, ТипКнопки, Отображение, Положение and Действие. `std-codes/launcher_module.bsl` has no button dump. The a1-live launcher should be extended with:
- the button's position index;
- Картинка (empty or not);
- СочетаниеКлавиш, Пояснение, КнопкаПоУмолчанию;
- `ТипЗнч(Кн.Действие)`;
- the panel's АвтоЗаполнение and ИсточникДействий.

Running it over more forms would also yield runtime names for the unknown standard codes.

Open questions:
- I did not run anything in live 1C. The semantics listed under risks are unverified.
- The root-group mismatch in `cb_table` comes from reading the code; I did not run a pack to confirm it.
- I did not check how version-7 buttons (13 fields) interact with the DefaultButton/ButtonOrder slots in the existing patcher.