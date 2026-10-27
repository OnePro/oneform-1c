Status: done

Findings:

## 1. Survey

**Scope and method.** I read 3,006 streams: ut, which equals ut2 and ut3 (the three give identical counts), plus «Бух.», plus tpd. They hold 15,650 link rows in ut+«Бух.» and 100 in tpd. I did not change the repo. The supervisor allowed scripts under `target/`, and they are in `target/advisor-survey/`: `meta.py` builds uuid→name from both config exports, and `classify.py`, `detail.py`, `cols.py`, `join.py` do the analysis. `join.py` cross-checks against the runtime dumps in `target/std-codes/runtime.json`.

**Row structure.** In every row, `{aid}` has one element, there is one row per control, steps are never nested, and K = 1 + number of steps. Each step has one of three shapes:
- `{0,uuid}`: 9,865 rows
- `{-N}`, including `{-60001}`
- `{N}` with N ≥ 0: 2,809 rows together with `{-N}`

At the step-kind level there are 56 distinct shapes.

**K distribution:**

| K | ut | «Бух.» | tpd |
|---|---|---|---|
| 1 | 2443 | 2468 | 59 |
| 2 | 4636 | 4881 | 41 |
| 3 | 494 | 78 | – |
| 4 | 487 | 100 | – |
| 5 | 44 | 19 | – |

**What the root (`aid`) can be:**
- The main attribute.
- Another form attribute. Its type can be a list (Catalog/Document/Journal/register List), ValueTable/Tree/List, ConstantsSet, another object (DataProcessor/Report/Catalog) or a builder. Builder type uuids: `0dda99d9` = ПостроительОтчета and `cab0d12b` = КомпоновщикНастроек (from attribute names).
- `-2`, a pseudo-root that is not a form attribute. It appears in 382 rows: `{4|5,{-2},{<id of a Table control in the same form>},{0},…}`. In 381 of them step 1 is the id of an existing `Table`; in 1 the control is missing.

**Step meanings, named from the config exports and runtime dumps:**
- `{0,U}` can be any of these: Attribute, TabularSection, Resource, Dimension, AddressingAttribute, AccountingFlag, journal Column, Constant, or the uuid of an AccountingRegister object.
  - K=3 `UU`: 387 rows, and in all of them the second uuid belongs to a different object than the first. So it means `Attr.RefAttr`. Example: `{3,{2},{0,32c6f63b…},{0,17fd118e…}}` = ОбработкаОбъект.Организация.НаименованиеПолное, where the second uuid is an attribute of Catalog Организации.
- `{-N}` is a standard attribute of the kind reached at that point. Two cases are not in `STD_ATTRS` (`oof_props.rs:631`):
  - Document `{-8}` = **Движения**. Confirmed at runtime 5 times: `{3,{1},{-8},{0,940dccd3…}}` → `Движения.ПланСчетовОсновной2014`.
  - `{-60001}` = **Отбор**. Confirmed at runtime once: `{4,{3},{-60001},{-2},{-2}}` → `НастройкаСчетовОтчета.Отбор.Период.Значение`. The last step is a property of the filter item: `{-2}` = Значение (confirmed), `{-3}` = Использование (all 18 such controls are CheckBoxes), `{-6}` = ВидСравнения (all 5 controls are named `ВидСравнения…`). The current XML shows these as `СписокСобытий.Номер`, `.Дата` and `.Родитель`, which is wrong.
- `{N}` and `{0}` are property ordinals of a platform type (report builder, settings composer, settings, the current row data of a table). The numbering depends on that type:
  - For КомпоновщикНастроек.Настройки: `{0}` ПараметрыДанных, `{1}` Отбор, `{2}` Выбор, `{3}` Порядок, `{4}` УсловноеОформление. I inferred these from the Table names in ДанныеОбОтработанномВремени and ФормаНастройкиСтруктурыОтчета.
  - For the current row of a structure table, from Table names: `{2}` ПараметрыДанных … `{9}` ПользовательскиеПоля.
  - A trailing `{0}` most likely means `ДоступныеПоля*`.
  - `b9754f01-29e9-11d6-a3c7-0050bae0a776` (252 rows) is not in either config. Its usage points to the platform attribute ОтчетОбъект.КомпоновщикНастроек.
  - None of these can be named offline: 529 rows (ut 349, «Бух.» 180) have such a step and need a type table checked in 1C.

**Links not recoverable from today's default XML (no `--carriers`):**

| | ut | «Бух.» |
|---|---|---|
| Lossy rows | 1,238 of 8,104 | 368 of 7,546 |
| K=3/5 not rendered | 538 | 97 |
| K=4 shown with at most one name | 487 | 100 |
| K=2 collapsed to bare root | 213 | 171 |

The 384 collapsed K=2 rows break down as:
- 212 are a Resource/Dimension of the form's own register. The cause is that the lookup only takes `Attribute|TabularSection` (`formbin.rs:238`).
- 64 are Constants.
- 34 are a TS or attribute of another object under a non-main root.
- 47 are builder `{0}`/`{N}` steps.
- 16 are standard codes under an unknown kind.
- 8 are AddressingAttribute or AccountingFlag.

Example of a K=3 row with no XML today: Table `ТаблицаПроводок` with `{3,{1},{-8},{0,…}}` looks unbound. If an agent "binds" it, `pack` overwrites the link (`check_object_bindings`, `form_patch.rs:5122`).

**Columns** (`X={8,P,DATA,CHECK,PIC}`):
- DATA can be uuid, `{-1}`, `{-N}`, `{N}` (6,124 LIST columns, never rendered) or `{q,U}` with q = 1..3. The `{q,U}` form appears in about 150 accounting-register columns, for example `{2,e0c502c7…}` where the uuid is Resource Количество.
- CHECK is set while DATA = `{-1}` in 1,048 columns, and the decoder then shows CHECK as `DataPath` (`oof.rs:373-376`).
- In 67 columns both DATA and CHECK are set, so CHECK is invisible.
- PIC is always `{-1}` in the corpora.
- For VT columns, 252 have only `w[3]`.

**Writer side today:**
- The parser accepts `DataPath` only when it has exactly one attribute and the text is either the root name or `Main.X` (`form_patch.rs:3478-3486`). Anything else goes to `rest`.
- `resolve_link_step` refuses paths of 3 or more segments (`form_patch.rs:5080`).
- The baseline is re-decoded at pack time (`pack.rs:73`).
- `raw`/`sig` exist only for K=4 and only with `--carriers` (`oof.rs:1607-1611`).

## 2. Candidate designs

- **(A) Text plus `raw="{…}"` only when lossy.** Lossless, but the attribute appears or disappears depending on metadata and oneform version, so diffs churn. Brace syntax is easy for an agent to break, and it repeats K and aid.
- **(B) `<Step>` children, dotted text derived.** About 4× the tokens. An agent can no longer edit by name, and child elements in `DataPath` break today's parser and `diff_elements`. Rejected.
- **(C) `pathHash`.** It tells whether the text was edited, but it cannot restore anything. A copied control carries a meaningless hash, and the hash is opaque and version-bound. Rejected; the consistency rule in (D) gives the same guard.
- **(D, recommended) Always-present tokenised `steps` plus readable text with bracketed unknowns.** It is lossless for all 56 shapes and does not depend on metadata. It replaces `raw`/`sig` and also covers columns.
  - Cost: about 45 characters (roughly 20–25 tokens) per uuid step. That is about 0.48 MB on an estimated 40 MB of default XML (about 1.2%). For tpd it is about 1.8 KB on 322 KB.

## 3. Recommendation (D)

**Grammar.** A control has `<DataPath attributeId="A" [steps="T( T)*"]>Path</DataPath>`.
- Tokens:
  - `uuid` ⇔ `{0,uuid}`
  - `-N` ⇔ `{-N}`
  - `N` ⇔ `{N}`
  - `q:uuid` ⇔ `{q,uuid}`, columns only
- `steps` is required for K ≥ 2 and absent for K = 1.
- `Path` is `Root(.Seg)*`, where `Seg` is a name or `[token]`. The number of segments equals the number of tokens.
- Root `-2` is shown as `ЭлементыФормы`.
- Columns have `<DataPath>` for DATA (omitted when it is `{-1}`), `<CheckBoxDataPath>` for CHECK and `<PictureDataPath>` for PIC. Each carries one token, and the text is the table path plus one segment. The tag names are a suggestion; follow the project's naming.

**Examples (real rows):**
1. Form attribute, `{1,{28}}`: `<DataPath attributeId="28">ТаблицаЦен</DataPath>`
2. Standard attribute, `{2,{1},{-3}}`: `<DataPath attributeId="1" steps="-3">ДокументОбъект.Дата</DataPath>`
3. Tabular-section table: `<DataPath attributeId="1" steps="f151e036-e16a-469e-9a19-5c21d661f163">СправочникОбъект.Выплаты</DataPath>`
4. Column with check-box data, `{8,P,{-1},{0,54bae1ee…},{-1}}`: `<Column …><CheckBoxDataPath attributeId="…" steps="54bae1ee-7af1-4413-b0aa-bb4711aef4b1">Root.ЗаписьДополнительногоЛиста</CheckBoxDataPath>` and no `<DataPath>`.
5. K=3: `<DataPath attributeId="2" steps="32c6f63b-a441-4745-a7a0-4b173052935c 17fd118e-1906-418c-8ae8-3b5f8518eb01">ОбработкаОбъект.Организация.НаименованиеПолное</DataPath>`; and `steps="-8 940dccd3-6f58-4e24-9269-8c36f5090baa"` → `ДокументОбъект.Движения.ПланСчетовОсновной2014`.
6. K=4 with `-60001`: `<DataPath attributeId="51" steps="-60001 a3a08ce6-323a-4849-88bf-9e5bae6c9fe9 -6">СписокСобытий.Отбор.Контрагент.ВидСравнения</DataPath>`. Until the 1C check, the last segment is written `[-6]`.
7. Positive steps: `<DataPath attributeId="1" steps="b9754f01-29e9-11d6-a3c7-0050bae0a776 0 1">ОтчетОбъект.[b9754f01-29e9-11d6-a3c7-0050bae0a776].[0].[1]</DataPath>`. After the 1C check this becomes `ОтчетОбъект.КомпоновщикНастроек.Настройки.Отбор`.
8. Root `-2`: `<DataPath attributeId="-2" steps="275 0 377d6b79-e17d-46b4-ae3b-0a086cca5595">ЭлементыФормы.РасшифровкаПлатежа.[0].Сделка</DataPath>`. `[0]` is expected to be ТекущиеДанные.
9. Unresolvable uuid: `ОбработкаОбъект.[116ed66f-3305-4f06-88b0-6c2538664b23]`, with the same token in `steps`.

**Extract rules:**
- Emit every row. Do not guess: a step is named only if a table entry exists, otherwise it is written as `[token]`.
- Name standard codes by the kind reached at that point: the form owner for the main attribute, the pattern TypeId for other roots, the reference type after a uuid step.
- Extend the uuid lookup to Dimension, Resource, AddressingAttribute, AccountingFlag, ExtDimensionAccountingFlag, journal Column, Constant and register objects (`formbin.rs:238`). The lookup also needs other objects for K=3 rows.
- Add Document `-8` Движения and `-60001` Отбор to the tables.
- Remove the CHECK→DataPath fallback (`oof.rs:374`).
- Drop `raw`/`sig` on `DataPath`.

**Pack rules** (B = baseline re-decoded at pack time, E = edited):
- No `steps` in E: legacy behaviour. Same text means bytes are kept; otherwise the text is resolved or refused.
- `E.steps == B.steps`:
  - Same text: keep bytes.
  - Different text: resolve the text; if it cannot be resolved, refuse with a hint to re-extract. This covers renames.
- `E.steps != B.steps`, or an added or copied control:
  - Tokens must be well-formed, the `aid` must exist, and for `-2` the control id must be an existing Table.
  - Known uuids are checked when metadata is available.
  - Write the row only if the text equals render(steps) or resolve(text) == steps. Otherwise refuse, naming the line, the text and the rendering.
  - If metadata is missing, uuid segments are not checked (steps are authoritative) and a note is reported.
- Refuse deleting a Table that a `-2` link points to.
- Column steps can be edited only on tables bound to a tabular section, as today.

**Verification:** decode the patched stream and require (aid, steps) to be equal and the text to equal render(steps).

**Code changes:**
- `ControlValues.data_path` becomes `{aid, text, steps: Option<Vec<Step>>}`.
- `LinkStep` (`form_patch.rs:5044`) becomes `Vec<Step>`.
- `data_path_matches` (`form_patch.rs:9318`) compares steps.
- The JSON projection (`oof.rs:1949`) gains `steps`.
- Bump `ordinaryFormVersion` to 2.2 (`oof.rs:1793`). `pack` refuses 2.1 files with a re-extract hint; without this, a 2.1 XML that has no `DataPath` on a K=3 control would read as "link removed".
- Regenerate `xml-reference.md`; update `agent-guide.md:46-48,104`.

**Stages** (S < 200, M 200–600 lines):
- **S1 (M), do first.** Lossless read side: `steps` on every K ≥ 2 link, column check/picture paths, lookup extension, version bump. Pack keeps bytes when unchanged and refuses edits to K ≥ 3 links or to `steps`. This alone meets "never damage, always visible".
- **S2 (M).** `steps` become authoritative for edits and copies of any shape; text/steps consistency check; verification on steps.
- **S3 (S–M), after 1C checks.** Naming tables for filter items, ЭлементыФормы, the composer and builder ordinals, and the `b9754f01` uuid. Text-based resolution of K=3 `Ref.Field`.
- **S4 (M).** Editing column CHECK; standard codes per list kind for list columns.

**Testable requirements:**
1. Every corpus row (15,650) appears as exactly one `<DataPath>`, and (aid, steps) re-encodes to the same bytes.
2. Extract then pack with no edits gives byte-identical streams.
3. Editing any other property of a control with a K = 3, 4 or 5 link leaves the row bytes unchanged.
4. Text and steps that disagree are refused with a message.
5. Copying a K=4 control copies its row.
6. Without metadata: an unchanged form packs, a copy with `steps` packs, and a text-only new link is refused.
7. A Form.xml of version 2.1 is refused.
8. No guessed names (unit test for each table entry).
9. A column with DATA `{-1}` and CHECK set has no `<DataPath>`.
10. `xml_reference_is_current` passes after regeneration.

**Checks needed in live 1C** (runtime `Данные` dumps on the «Бух.» test base; 73 «Бух.» forms hold the complex links and only 6 links are confirmed so far):
- Filter-item codes `-3` and `-6`.
- That `aid -2` / `{id}` / `{0}` means ЭлементыФормы / ТекущиеДанные.
- What `b9754f01` is, and the ordinal tables for each type.
- Whether 1C loads newly written K ≥ 3 rows and shows them correctly in the designer.
- Column ДанныеФлажка/ДанныеКартинки, and the meaning of `{q,U}`.

Open questions:
- Unverified: the meanings of the positive/`{0}` ordinals, of `-3`/`-6` in filters, of root `-2` with `{0}`, of `b9754f01`, of the column `q` codes and of PIC. The new tag names `CheckBoxDataPath`/`PictureDataPath` are a suggestion.
- My map did not resolve the 41 tpd uuids because the processor's metadata is not in the exports; the extractor itself does resolve them.
- The corpus Form.xml files come from an older extract, made with `--carriers`. I did not re-run the current extract.
- The 40 MB figure for default-mode XML is an estimate.
- Not counted: list columns whose standard codes are named by the form owner's kind (`oof_props.rs:1240`) and so may be misnamed.
- Decision for you: on a rename, the rule is to refuse with a re-extract hint rather than keep the bytes silently.