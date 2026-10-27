Status: done

## 1. Panel stream layout

A control record is `{guid, id, info, geom, meta, children}` (`oof.rs:564` `control_parts`). The form root is also a Panel, `{guid, info, children}`, found at `root[1][2]` (`form_patch.rs:4516`). A Panel is the only control type that holds children. In both corpora, every control with children is a Panel, except one SpreadsheetDocumentField in ut2.

**Info `{1, P, events}`.** `P` is the panel parameters:
- `P[0]`: appearance node. `P[1]`: record version (18/20/21/25/26 in the corpora; the donor is 26).
- `P[2..]`: six counted incoming-anchor lists `{0,src,edge}`. They hold the panel's own children whose edges attach to target 0, the container (`anchors.rs:1-6, 47-52`). Donor Панель1 has lists `0 | 1,{0,34,1} | 0 | 2,{0,34,3},{0,35,3} | 0 | 0`, which point to the donor's own children.
- `base` is the index after the six lists (`oof.rs:487-491`).
  - `P[base]`: picture node `{3|4|8,…}` (Picture at [2], PictureSize at [6], PanelPictureLocation at [7]; `form_patch.rs:6195-6202`).
  - `base+1`: ShowTabs code.
  - `base+2`: 1 (664/705 panels in ut2, 731/736 in «Бух.»).
  - `base+3`: page table `{1, N, rec×N}`.
  - `base+4..6`: SeparateByPage / AutoTabOrder / ScrollPageMode (`form_patch.rs:6204-6211`). The value is `(1,1,0)` in 646/705 (ut2) and 691/736 («Бух.»).
  - `base+7`: R, the number of boundary rows, followed by R rows.
  - Then a tail.
- `info[2]` is the event list. The donor has one event, Панель1ПриСменеСтраницы.

**Page record.** Layout: `{ver, title{1,n,{lang,text}…}, appearance{…,picture at [2],…}, -1, visible, enabled, "name", 1, [ver5: {3,4,{0}},{3,4,{0}},{7,3,0,1,100}]}`. The reader uses fields [1], [4], [5] and [6] (`oof.rs:507-549`).
- Record versions: 3 = 8 items, 4 = 10 items, 5 = 11 items (donor). Counts in ut2: 1507/162/231; in «Бух.»: 711/940/0.
- `rec[3]` is always -1. `rec[7]` is 1 in every page except one.
- **Pages have no id.** A page's identity is its index in the page table plus its name (`rec[6]`). Pages do not take ids from the max-id slot `root[1][1][1]` (`donor.rs:255ff`). Form.xml writes `<Page name=…>` without an id.

**Boundary rows.** Each row is `{2, coord, axis(1=x,0=y), 1, edge, pageIdx, 0, margin, flag}`. There are 4 rows per page (edges 1-4: left, top, right, bottom). R = 4×N in almost every panel; 134 rows in total are extra guide rows with edge 0. Old-version rows have 8 items (124 rows).
- The decoder computes the page client area as `cw = e3.coord + e3.margin` and `ch = e4.coord + e4.margin`. Left and top come from the dx/dy heuristic (`oof.rs:511-526`).
- Pages without rows fall back to `(1,19,W-2,H-20)` with tabs, or `(0,0,W,H)` without (`oof.rs:527-537`). 9 panels in the corpora have no rows.

**Tail.** `{t0, N per-page control slots, 5, 64, …version extras}`. The donor v26 tail is `0,4294967295,4294967295,5,64,0,{3,4,{0}},0,0,57,0,0`.
- The per-page slots are `4294967295` (empty), or a child on that same page (all 10 observed non-empty cases). For the root panel, slot [1] is the default control (`form_patch.rs:6015-6020`).
- `t0` is 0 or 1 (45 panels have 1). Its meaning is unknown.

**Children of pages.** They live in the panel's single flat `children` list, not in per-page lists. A child's page is the index stored in its geometry tail: `geom[12..]` six lists, then `page, z, tab, flagA, flagB` (`oof.rs:445-463`). Child coordinates are relative to the page client area: 18450/18501 children fit inside it.
- **z-order** is per page, 0..k-1: 3490/3490 page groups.
- **Tab order** is per page, 1..k: 3349/3490 groups. Across the pages of one panel, tab values are not unique (839/1436 panels). So the guide's rule "TabOrder = next free in the form" (`docs/agent-guide.md:44`) is wrong for children on a page.
- If a child's page index is out of range, the decoder silently files it on page 0 (`oof.rs:1607-1608`).

**What depends on N (the number of pages):** the page table count and records, R = 4N rows, and the N tail slots. The page index of each child depends on page order.

**Donor panel.** `prototype()` takes the first Panel in the donor, Панель1 id 13 (`donor.rs:23-33`). It brings:
- 2 pages: Страница1 with **visible=0** and Страница2;
- ShowTabs=2;
- children Table 34 «ТабличноеПоле2» and CommandBar 35, with the donor's ids;
- anchor lists that point to 34 and 35;
- 8 rows sized for 278×154;
- the ПриСменеСтраницы event.

`clone_prototype_raw` changes only the id, the name and UsualGroup appearance (`donor.rs:213-247`). For a Panel, `set_clone_props` sets only ShowTabs (`form_patch.rs:6571-6584`). The donor children are therefore written out as-is, which causes error (b).

**Annotated samples**

1. **2 pages × 3 controls, ShowTabs=0.** ut2 `DataProcessors/ЭкспортИмпортКонтактныхДанныхОбъектов/…/ФормаВыполненияВыгрузки`, panel 3, 480×400.
   - Lists: 6/2/4/6/0/0 entries, all of them its children.
   - `P[base]` = `{3,1,…}`, ShowTabs 0, `+2` = 1.
   - Pages: v3 records «ЗаполнениеТаблицыПолей» and «НастройкаВыгрузкиЗначений».
   - Flags `1,1,0`.
   - 8 rows: `{2,0,1,1,1,p,..}`, `{2,0,0,1,2,p}`, `{2,480,1,1,3,p,0,0,0}`, `{2,400,0,1,4,p,0,0,0}` for p = 0 and 1.
   - Tail `{0,4294967295,4294967295,5,64}`.
   - Children as (page, z, tab): Table 4 (0,0,3), CmdBar 5 (0,1,2), Group 13 (0,2,1); Table 9 (1,0,3), CmdBar 10 (1,1,2), Group 14 (1,2,1).
2. **One page, ShowTabs=1.** ut2 `Catalogs/ТаможенныеВыплаты/ФормаЭлемента`, panel 30, 524×189.
   - Rows `6/6/516+6/163+6`, so the client area is 522×169 = (W-2, H-20).
   - Tail `{0,4294967295,5,64,0}`.
   - Children: 21 (0,0,1), 22 (0,1,2).
3. **One page, ShowTabs=0.** ut2 `Catalogs/ВнешниеОбработки/ФормаЭлемента`, panel 60, 602×247.
   - Rows `0/6/602+0/247+0`, so the client area equals the panel size.
   - Tail `{0,4294967295,5,64,0}`.
4. **Typical tabbed element form.** ut2 `Catalogs/Организации/ФормаЭлемента`, panel 58. It has 5 pages and ShowTabs=7, and its decoded client area is (1,19,547,205), i.e. tabs on top.
5. **Form.xml grammar** for a tabbed panel. «Бух.» `Catalogs/ГруппыТМЦдляИморта/ФормаЭлемента/Form.xml:145-429`:
   - `<Panel name id><Left..Height><ZOrder><TabOrder><Anchors/><ShowTabs type="ShowTabs" member="Top" code="1"/>`
   - `<ChildItems><Page name><Title/><Left>1</Left><Top>19</Top><Width>482</Width><Height>291</Height><ChildItems>controls…</ChildItems></Page>…`

## 2. Survey (ut2: 1525 streams, 1521 forms; «Бух.»: 1481)

| | ut2 | «Бух.» |
|---|---|---|
| Panels | 705 | 736 |
| Forms with 0 / 1 / 2 / 3 / 4+ panels | 1088 / 318 / 71 / 20 / 24 | 1074 / 260 / 52 / 79 / 16 |
| Panels with 1 / 2 / 3 / 4 / 5 / 6+ pages | 210 / 204 / 154 / 43 / 44 / 50 (max 20) | 370 / 157 / 95 / 38 / 23 / 53 (max 20) |
| Children per page (most common) | 2 (598), 1 (244), 4 (185), 3 (169); empty pages 35 | 2 (516), 4 (296), 1 (269); empty pages 26 |
| Panel placement | direct root child 522; inside another Panel 183 (depths 1/2/3: 149/31/3) | root 606; nested 130 (110/19/1) |
| ShowTabs codes | 0: 431, 1: 255, other 19 (2,3,4,5,7,8,11,13) | 0: 409, 1: 243, 7: 64, 13: 10, 5: 8, 2: 1, 3: 1 |
| Root panel pages | 1 page in 1511 forms; 10 forms with 2-15 pages; root ShowTabs=1 in 2 forms | always 1 page, ShowTabs 0 |

- **Meaning of ShowTabs codes:** Form.xml maps only 0 = None and 1 = Top; every other code is written as "Unknown" (`oof.rs:1716-1720`). Codes ≥2 include tabs on the side: the page offsets have dx>dy, e.g. (22,2) and (225,2).
- **Page area inside the panel:**
  - ShowTabs=0: client area = W×H in 1494/1584 pages.
  - ShowTabs=1: client area = (W-2)×(H-20) in 1463/1571 pages.
  - Margin guides (left, top, right margin, bottom margin): `(6,6,6,6)` in 1534/3551 pages, `(0,0,0,0)` in 768.
  - Rows on all four edges for every page: 1432/1441 panels.
- **Panel anchors.** Edge order is top, bottom, left, right; `0` = attached to the container, `-` = not attached. Most common: `(-,0,-,0)` (stretches with the container) in 265 (ut2) / 292 («Бух.»). All four edges to the container: 114 / 155. Anchors to another control are also common.
- **Panel meta:** `meta[2]` is 4294967295 in all 1441 panels.

## 3. Design

**Grammar.** Keep what extract already writes:
- `<Panel>` with `<ShowTabs code>`, `<ChildItems><Page name><Title/>[<Visible/><Enabled/>]<ChildItems>…`.
- Page `Left`/`Top`/`Width`/`Height` are optional on an added panel. If present, they must equal the derived values; otherwise pack fails with `unsupported-panel` (same rule as today).
- For added panels, allow only ShowTabs 0 or 1. Default to 1. Today a missing `<ShowTabs>` silently means code 2, which is a donor leftover (`form_patch.rs:3056-3058`).

**Why adding fails today**
- **(a)** A child's `c.parent` is the *XML* id of the new panel. The additions loop looks the parent up in the index of the *existing* stream (`form_patch.rs:10143-10147`). `resolve_page` does the same (`:5583-5624`). The children list is also taken from the index (`:10166-10172`). On top of that, `container_holder` falls back to the root panel 0 for an unknown parent (`anchors.rs:34-44`), so the children's anchor entries would go into the wrong lists.
- **(b)** The donor children 34/35 are cloned along with the panel.

**Stage A: add a Panel with pages (~350-450 lines of code + ~15 tests)**
1. Process additions as a tree. The parser already lists them parent-first (`form_patch.rs:2878-2905`). Keep a map from XML id to the allocated id and the clone. Serialize bottom-up and insert into the parent clone's children list, not into a stream list.
2. Build a clean panel, `build_panel_clone(pages, show_tabs, W, H)`:
   - six lists set to `0`;
   - page table `{1,N,…}` built from the donor's version-5 page record (Страница2), with title, name, visible and enabled set and the picture reset;
   - 4N rows with margins 6 (or 0) and `cw/ch` as in §2, so that `panel_pages` gives (1,19,W-2,H-20) or (0,0,W,H);
   - tail `{0, N×4294967295, 5,64,0,{3,4,{0}},0,0,57,0,0}`;
   - children `{0}`;
   - events from the request, or `{0}`.
3. Children: the page index is the position of the page name in the requested Panel's pages. Write z and tab per page. Anchors with target 0 go to `Holder` = the new panel's own lists, applied inside the clone. v1 refuses anchors to other new controls, as the code does today (`anchors.rs:274-275`). It should also refuse anchors to existing controls outside the panel; today they pass, because only existence is checked (`:276`).
4. Verification:
   - extend `verify_structural` (`form_patch.rs:10308`) to check each child's parent and page;
   - push `DerivedPage` entries for the new panel so `with_derived_pages` (`:2518`) fills in, or *inserts*, page geometry. Today it only overwrites tags that are present.
   - Edge case: a 1-page, ShowTabs=0 panel with no children decodes without any `<Page>` (`oof.rs:1783`). Normalize the expected XML for that case.
   - Fix the agent guide (`docs/agent-guide.md:61` and the TabOrder rule at `:44`).
   - Requirements to test: «Основное»/«Дополнительно» with 3 fields each; one page; ShowTabs 0; a nested panel; a re-extract that matches the request; donor ids 34/35 absent; a later resize works through `panel_page_pieces`.

**Stage B: pages of an existing panel (~300-400 lines of code + ~12 tests)**

Today:
- a different page count fails ("page count mismatch", `form_patch.rs:9003-9005`);
- a renamed page makes its children fail with `moved-control` (`:4960-4962`).

Proposed:
- Match pages by name. If the count is unchanged and only names differ, pair pages by position (rename). A reorder is a permutation of the same names.
- **Add a page:** clone that panel's own page[0] record (same version) and append 4 rows. Copy the margins from page 0's rows so the new page gets the same client area. Append a tail slot.
- **Remove a page:** refuse unless no surviving control is on it, i.e. its controls are deleted in the same request. Moving controls to another page stays unsupported.
- **On remove or reorder,** renumber `row[5]` in the boundary rows, the tail slots and the geometry page index of *every* child. Without this, the decoder silently moves children to page 0. If `t0` is ≥ the new page count, reset it to 0.
- Root panel pages are out of scope.

**Stage C: live checks (needs 1C)**
- The launcher (`target/a1-live/launcher_module.bsl:59,82-85`) dumps geometry, ТекущаяСтраница, anchors, and per page Имя/Заголовок/Видимость/Доступность. **It needs extending** to dump: ОтображениеЗакладок, РаспределятьПоСтраницам, АвтоПорядокОбхода, РежимПрокручиваемыхСтраниц, the page count and order, and the effect of switching ТекущаяСтраница.
- Which page a child is on cannot be read from the dump. Check it with a screenshot (`run-a1shot.cmd`) or a Designer round-trip (`target/designer-acceptance`).
- Confirm in Designer: that the panel opens; tab order on new pages (AutoTabOrder=1); and that the empty tail slots and `t0`=0 are accepted.

**Stage D: docs and compatibility matrix (small).**

Open questions:
- What ShowTabs codes ≥2 mean (only 0 and 1 are mapped).
- The meaning of tail `t0`, page `rec[3]`/`rec[7]`, and the extra guide rows with edge 0.
- Whether 1C accepts version-5 donor page records in an *existing* panel with version-3 records. The design avoids this by cloning the panel's own page record.
- Nothing above was checked in live 1C. The corpus figures come from my own scan using `target/std-codes/brace.py`.