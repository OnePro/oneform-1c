//! Read side of the extended control properties that `pack` can patch (`ext_slot`, the `InputField` /
//! `Table` slots and the control meta flags): `extract` writes them to `Form.xml` sparsely, i.e. only when the
//! stream's value differs from the donor prototype control of the same type, and `pack` verifies them after
//! patching (an explicit default value is normalized away by [`drop_defaults`] before the comparison).

use std::collections::HashSet;
use std::sync::OnceLock;

use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Flag,
    Int,
    Str,
    Color,
    Font,
    Shortcut,
    Picture,
    Loc,
}

#[derive(Debug, Clone)]
enum ExtVal {
    Flag(bool),
    Int(i64),
    Str(String),
    Color(ColorSpec),
    Font(FontSpec),
    Shortcut(ShortcutSpec),
    Picture(PictureSpec),
    Loc(LocItems),
}

/// A `ControlValues` field that holds one extended property.
trait ExtField: Sized {
    const KIND: Kind;
    fn put(&mut self, x: ExtVal);
    fn val(&self) -> Option<ExtVal>;
    fn drop_if_same(&mut self, default: &Self);
    fn keep_if_absent(&mut self, old: &Self);
}

macro_rules! opt_field {
    ($t:ty, $kind:ident, $var:ident) => {
        impl ExtField for Option<$t> {
            const KIND: Kind = Kind::$kind;
            fn put(&mut self, x: ExtVal) {
                if let ExtVal::$var(v) = x {
                    *self = Some(v);
                }
            }
            fn val(&self) -> Option<ExtVal> {
                self.clone().map(ExtVal::$var)
            }
            fn drop_if_same(&mut self, default: &Self) {
                if self == default {
                    *self = None;
                }
            }
            fn keep_if_absent(&mut self, old: &Self) {
                if self.is_none() {
                    self.clone_from(old);
                }
            }
        }
    };
}

opt_field!(bool, Flag, Flag);
opt_field!(i64, Int, Int);
opt_field!(String, Str, Str);
opt_field!(ColorSpec, Color, Color);
opt_field!(FontSpec, Font, Font);
opt_field!(ShortcutSpec, Shortcut, Shortcut);
opt_field!(PictureSpec, Picture, Picture);

impl ExtField for LocItems {
    const KIND: Kind = Kind::Loc;
    fn put(&mut self, x: ExtVal) {
        if let ExtVal::Loc(v) = x {
            *self = v;
        }
    }
    fn val(&self) -> Option<ExtVal> {
        (!self.is_empty()).then(|| ExtVal::Loc(self.clone()))
    }
    fn drop_if_same(&mut self, default: &Self) {
        if self == default {
            self.clear();
        }
    }
    fn keep_if_absent(&mut self, old: &Self) {
        if self.is_empty() {
            self.clone_from(old);
        }
    }
}

/// The emitted properties in their `Form.xml` order, with the `ControlValues` field that holds each.
macro_rules! ext_fields {
    ($($field:ident, $prop:ident, $ty:ty;)*) => {
        const EXT_ORDER: &[Property] = &[$(Property::$prop),*];

        fn kind_of(p: Property) -> Option<Kind> {
            match p {
                $(Property::$prop => Some(<$ty as ExtField>::KIND),)*
                _ => None,
            }
        }

        fn put(v: &mut ControlValues, p: Property, x: ExtVal) {
            match p {
                $(Property::$prop => v.$field.put(x),)*
                _ => {}
            }
        }

        fn val(v: &ControlValues, p: Property) -> Option<ExtVal> {
            match p {
                $(Property::$prop => v.$field.val(),)*
                _ => None,
            }
        }

        /// Clear every emitted property whose value equals the donor default (`None` / empty = "as the donor").
        pub(super) fn drop_defaults(v: &mut ControlValues, default: &ControlValues) {
            $(v.$field.drop_if_same(&default.$field);)*
        }

        /// A property that `new` does not mention keeps its `old` value: `Form.xml` files extracted before these
        /// properties were written do not list them, which is no request to remove them.
        pub(super) fn keep_absent(new: &mut ControlValues, old: &ControlValues) {
            $(new.$field.keep_if_absent(&old.$field);)*
        }
    };
}

ext_fields! {
    modifies_data, ModifiesData, Option<bool>;
    skip_on_input, SkipOnInput, Option<bool>;
    first_in_group, FirstInGroup, Option<bool>;
    choose_type, ChooseType, Option<bool>;
    text_edit, TextEdit, Option<bool>;
    password_mode, PasswordMode, Option<bool>;
    extended_edit, ExtendedEdit, Option<bool>;
    multi_line, MultiLine, Option<bool>;
    mask, Mask, Option<String>;
    auto_cell_height, AutoCellHeight, Option<bool>;
    drop_list_button, DropListButton, Option<bool>;
    spin_button, SpinButton, Option<bool>;
    auto_choice_incomplete, AutoChoiceIncomplete, Option<bool>;
    vertical_align, VerticalAlign, Option<String>;
    format, Format, LocItems;
    choice_list_width, ChoiceListWidth, Option<i64>;
    choice_list_height, ChoiceListHeight, Option<i64>;
    min_value, MinValue, Option<i64>;
    max_value, MaxValue, Option<i64>;
    field_back_color, FieldBackColor, Option<ColorSpec>;
    field_text_color, FieldTextColor, Option<ColorSpec>;
    button_back_color, ButtonBackColor, Option<ColorSpec>;
    button_text_color, ButtonTextColor, Option<ColorSpec>;
    shortcut, Shortcut, Option<ShortcutSpec>;
    picture, Picture, Option<PictureSpec>;
    choice_button_picture, ChoiceButtonPicture, Option<PictureSpec>;
    selection_value, SelectionValue, Option<String>;
    menu_mode, MenuMode, Option<bool>;
    picture_location, PictureLocation, Option<String>;
    picture_size, PictureSize, Option<String>;
    scrolling_text, ScrollingText, Option<String>;
    label_picture_location, LabelPictureLocation, Option<String>;
    panel_picture_location, PanelPictureLocation, Option<String>;
    separate_by_page, SeparateByPage, Option<bool>;
    auto_tab_order, AutoTabOrder, Option<bool>;
    scroll_page_mode, ScrollPageMode, Option<bool>;
    orientation, Orientation, Option<String>;
    secondary, Secondary, Option<bool>;
    zoomable, Zoomable, Option<bool>;
    use_context_menu, UseContextMenu, Option<bool>;
    nonselected_picture_text, NonselectedPictureText, LocItems;
    enable_start_drag, EnableStartDrag, Option<bool>;
    enable_drag, EnableDrag, Option<bool>;
    show_picture, ShowPicture, Option<bool>;
    show_check_box, ShowCheckBox, Option<bool>;
    step, Step, Option<i64>;
    large_step, LargeStep, Option<i64>;
    marking_step, MarkingStep, Option<i64>;
    marking_appearance, MarkingAppearance, Option<String>;
    show_style, ShowStyle, Option<String>;
    show_percent, ShowPercent, Option<bool>;
    selection_back_color, SelectionBackColor, Option<ColorSpec>;
    selection_text_color, SelectionTextColor, Option<ColorSpec>;
    holiday_color, HolidayColor, Option<ColorSpec>;
    show_current_date, ShowCurrentDate, Option<bool>;
    calendar_navigation, CalendarNavigation, Option<bool>;
    begin_of_display_period, BeginOfDisplayPeriod, Option<i64>;
    end_of_display_period, EndOfDisplayPeriod, Option<i64>;
    table_read_only, ReadOnly, Option<bool>;
    header, Header, Option<bool>;
    horizontal_lines, HorizontalLines, Option<bool>;
    vertical_lines, VerticalLines, Option<bool>;
    auto_insert_new_row, AutoInsertNewRow, Option<bool>;
    change_row_set, ChangeRowSet, Option<bool>;
    change_position_of_columns, ChangePositionOfColumns, Option<bool>;
    change_setting_of_columns, ChangeSettingOfColumns, Option<bool>;
    initial_list_view, InitialListView, Option<String>;
    output, Output, Option<String>;
    choice_mode, ChoiceMode, Option<bool>;
    use_alternation_row_color, UseAlternationRowColor, Option<bool>;
    header_back_color, HeaderBackColor, Option<ColorSpec>;
    header_text_color, HeaderTextColor, Option<ColorSpec>;
    footer_back_color, FooterBackColor, Option<ColorSpec>;
    footer_text_color, FooterTextColor, Option<ColorSpec>;
    alternation_row_back_color, AlternationRowBackColor, Option<ColorSpec>;
    fixed_left, FixedLeft, Option<i64>;
    fixed_right, FixedRight, Option<i64>;
    row_input_mode, RowInputMode, Option<String>;
    row_selection_mode, RowSelectionMode, Option<String>;
    header_height, HeaderHeight, Option<i64>;
    footer_height, FooterHeight, Option<i64>;
    header_font, HeaderFont, Option<FontSpec>;
    footer_font, FooterFont, Option<FontSpec>;
    show_selection, ShowSelection, Option<String>;
    auto_context_menu, AutoContextMenu, Option<bool>;
    collapse, Collapse, Option<String>;
    embedded_into_cell, EmbeddedIntoCell, Option<bool>;
    print, Print, Option<bool>;
    protection, Protection, Option<bool>;
    show_grid, ShowGrid, Option<bool>;
    show_headers, ShowHeaders, Option<bool>;
    show_groups, ShowGroups, Option<bool>;
    black_and_white_view, BlackAndWhiteView, Option<bool>;
    totals_right, TotalsRight, Option<bool>;
    totals_below, TotalsBelow, Option<bool>;
    print_scale, PrintScale, Option<i64>;
    fit_to_page, FitToPage, Option<bool>;
    black_and_white, BlackAndWhite, Option<bool>;
    collate, Collate, Option<bool>;
    copies, Copies, Option<i64>;
    per_page, PerPage, Option<i64>;
    page_orientation, PageOrientation, Option<String>;
    bottom_margin, BottomMargin, Option<i64>;
    top_margin, TopMargin, Option<i64>;
    left_margin, LeftMargin, Option<i64>;
    right_margin, RightMargin, Option<i64>;
    header_size, HeaderSize, Option<i64>;
    footer_size, FooterSize, Option<i64>;
    duplex_printing, DuplexPrinting, Option<i64>;
    print_accuracy, PrintAccuracy, Option<i64>;
    step_direction, StepDirection, Option<i64>;
    save_view_properties, SaveViewProperties, Option<bool>;
    fixed_background, FixedBackground, Option<bool>;
}

// ------------------------------------------------------------------ reading the stream

fn nav<'a>(mut cur: NodeRef<'a>, path: &[usize]) -> Option<NodeRef<'a>> {
    for &i in path {
        cur = cur.child(i)?;
    }
    Some(cur)
}

fn bare_int(n: NodeRef, i: usize) -> Option<i64> {
    let (a, _) = n.atom(i)?;
    if a.kind != AtomKind::Bare {
        return None;
    }
    strict_int(&a.value)
}

fn flag_at(n: NodeRef, i: usize) -> Option<bool> {
    match bare_int(n, i)? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn code_name(table: CodeTable, n: i64) -> Option<String> {
    table.iter().find(|(_, c)| *c == n).map(|(name, _)| (*name).to_string())
}

/// `{"U"}` / `{"B",n}` / `{"N",n}` / `{"S","s"}` -> the `<SelectionValue>` text.
fn selection_value_text(n: &Node) -> Option<String> {
    let tag = n.atom(0)?.value.as_str();
    let s = match (tag, n.len()) {
        ("U", 1) => String::new(),
        ("B", 2) => match n.atom(1)?.value.as_str() {
            "1" => "true".to_string(),
            "0" => "false".to_string(),
            _ => return None,
        },
        ("N", 2) => strict_int(&n.atom(1)?.value)?.to_string(),
        ("S", 2) => n.atom(1)?.value.clone(),
        _ => return None,
    };
    let again = crate::brace::parse_metadata(selection_value_node(&s).as_bytes()).ok()?;
    (&again == n).then_some(s)
}

fn read_slot(info: NodeRef, slot: &ExtSlot, prop: Property) -> Option<ExtVal> {
    let parent = nav(info, &slot.path)?;
    let item = parent.node.children.get(slot.idx)?;
    match kind_of(prop)? {
        Kind::Flag => {
            let n = bare_int(parent, slot.idx)?;
            if prop == Property::AutoContextMenu {
                return match n {
                    0 => Some(ExtVal::Flag(true)),
                    2 => Some(ExtVal::Flag(false)),
                    _ => None,
                };
            }
            flag_at(parent, slot.idx).map(ExtVal::Flag)
        }
        Kind::Int => {
            if matches!(prop, Property::MinValue | Property::MaxValue) && slot.path == [2, 0] {
                let n = item.as_node().filter(|n| n.len() == 2 && n.atom(0).is_some_and(|a| a.value == "N"))?;
                return strict_int(&n.atom(1)?.value).map(ExtVal::Int);
            }
            bare_int(parent, slot.idx).map(ExtVal::Int)
        }
        Kind::Str => match prop {
            Property::SelectionValue => selection_value_text(item.as_node()?).map(ExtVal::Str),
            Property::Output => Some(ExtVal::Str(output_name(bare_int(parent, slot.idx)?).to_string())),
            _ => code_name(slot.table, bare_int(parent, slot.idx)?).map(ExtVal::Str),
        },
        Kind::Color => read_color_spec(item).map(ExtVal::Color),
        Kind::Font => read_font_spec(item).map(ExtVal::Font),
        Kind::Shortcut => read_shortcut_spec(item).map(ExtVal::Shortcut),
        Kind::Picture => read_picture_spec(item).map(ExtVal::Picture),
        Kind::Loc => {
            let items = strict_localized(parent.child(slot.idx)?)?;
            Some(ExtVal::Loc(items.into_iter().map(|i| (i.lang, i.text)).collect()))
        }
    }
}

fn output_name(code: i64) -> &'static str {
    match code {
        1 => "Enable",
        2 => "Disable",
        _ => "Auto",
    }
}

/// Properties that have no `ext_slot`: the control meta flags, the `InputField` slots and the `Table` slots.
fn read_direct(entry: &ControlEntry, info: NodeRef, prop: Property) -> Option<ExtVal> {
    use Property as P;
    let flag = |b: Option<bool>| b.map(ExtVal::Flag);
    match prop {
        P::ModifiesData | P::SkipOnInput | P::FirstInGroup => {
            let idx = match prop {
                P::ModifiesData => 3,
                P::SkipOnInput => 4,
                _ => 5,
            };
            return flag(flag_at(entry.meta()?, idx));
        }
        _ => {}
    }
    match entry.type_name {
        "InputField" => {
            if prop == P::ChooseType {
                return flag(flag_at(info, 6));
            }
            let q = nav(info, &[2, 0])?;
            match prop {
                P::AutoCellHeight => flag(flag_at(q, 4)),
                P::DropListButton => flag(flag_at(q, 6)),
                P::SpinButton => flag(flag_at(q, 9)),
                P::AutoChoiceIncomplete => flag(flag_at(q, 36)),
                P::PasswordMode => flag(flag_at(q, 5)),
                P::ExtendedEdit => flag(flag_at(q, 24)),
                P::MultiLine => flag(flag_at(q, 26)),
                P::TextEdit => flag(flag_at(q, 12).map(|inverted| !inverted)),
                P::Mask => {
                    let (a, _) = q.atom(21)?;
                    (a.kind == QUOTED).then(|| ExtVal::Str(a.value.clone()))
                }
                P::VerticalAlign => code_name(V_ALIGN, bare_int(q, 18)?).map(ExtVal::Str),
                _ => None,
            }
        }
        "Table" => {
            let t = nav(info, &[2, 1])?;
            match prop {
                P::ChangeRowSet => bare_int(t, 22).map(|n| ExtVal::Flag(n != 0)),
                P::InitialListView => {
                    let name = match bare_int(t, 20)? {
                        0 | 2 => "Begin",
                        1 => "End",
                        _ => "Auto",
                    };
                    Some(ExtVal::Str(name.to_string()))
                }
                P::Output => Some(ExtVal::Str(output_name(bare_int(t, 24)?).to_string())),
                p if is_table_flag(p) => Some(ExtVal::Flag(table_flag_read(bare_int(t, 1)?, p))),
                _ => None,
            }
        }
        _ => None,
    }
}

fn read_one(entry: &ControlEntry, info: NodeRef, prop: Property) -> Option<ExtVal> {
    if !tag_supported_for(entry.type_name, prop.xml_name()) {
        return None;
    }
    match ext_slot(entry.type_name, prop, info.node) {
        Some(slot) => read_slot(info, &slot, prop),
        None => read_direct(entry, info, prop),
    }
}

/// `ControlValues` holding only the extended properties of `entry` (the others keep their empty defaults); every
/// value is the one `parse_form_xml` yields for the element that [`enrich_ext_properties`] writes for it.
fn read_ext_values(entry: &ControlEntry) -> ControlValues {
    let mut v = ControlValues::empty();
    let Some(info) = entry.info() else { return v };
    for &prop in EXT_ORDER {
        if let Some(x) = read_one(entry, info, prop).and_then(|x| canonical(prop, x)) {
            put(&mut v, prop, x);
        }
    }
    v
}

/// Donor values of every control type (first donor control of the type): the reference of the sparse emission.
pub(super) fn defaults(type_name: &str) -> Option<&'static ControlValues> {
    static DEFAULTS: OnceLock<HashMap<&'static str, ControlValues>> = OnceLock::new();
    DEFAULTS
        .get_or_init(|| {
            let mut out = HashMap::new();
            if let Ok(index) = ControlIndex::build(crate::donor::donor()) {
                for entry in &index.entries {
                    out.entry(entry.type_name).or_insert_with(|| read_ext_values(entry));
                }
            }
            out
        })
        .get(type_name)
}

// ------------------------------------------------------------------ XML elements

fn xml_safe(s: &str) -> bool {
    s.chars().all(|c| matches!(c, '\t' | '\n' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'))
}

fn elem_of(tag: &str, x: &ExtVal) -> Option<XmlElement> {
    let mut e = XmlElement::new(tag, 0);
    let mut attr = |k: &str, v: String| {
        e.attrs.insert(k.to_string(), v);
    };
    let mut text = String::new();
    let mut children = Vec::new();
    match x {
        ExtVal::Flag(b) => text = b.to_string(),
        ExtVal::Int(n) => text = n.to_string(),
        ExtVal::Str(s) => {
            if !xml_safe(s) {
                return None;
            }
            text.clone_from(s);
        }
        ExtVal::Color(ColorSpec::Rgb(_, hex)) => {
            attr("kind", "rgb".into());
            attr("value", hex.clone());
        }
        ExtVal::Color(ColorSpec::Style(r)) => {
            attr("kind", "style".into());
            attr("ref", r.clone());
        }
        ExtVal::Font(f) => {
            for (k, on) in [("bold", f.bold), ("italic", f.italic), ("underline", f.underline), ("strikeout", f.strikeout)] {
                if on {
                    attr(k, "true".into());
                }
            }
            if f.style_source {
                attr("source", "style".into());
            }
            if let Some(face) = &f.face {
                if !xml_safe(face) {
                    return None;
                }
                attr("face", face.clone());
            }
            if let Some(size) = f.size {
                attr("size", size.to_string());
            }
            if let Some(scale) = f.scale {
                attr("scale", scale.to_string());
            }
        }
        ExtVal::Shortcut(s) => {
            attr("key", s.key.to_string());
            attr("mods", s.mods.to_string());
        }
        ExtVal::Picture(p) => {
            attr("kind", p.kind.to_string());
            if let Some(g) = &p.ref_guid {
                attr("refGuid", g.clone());
            }
            if let Some(id) = p.std_id {
                attr("stdId", id.to_string());
            }
        }
        ExtVal::Loc(items) => {
            let mut seen = HashSet::new();
            for (lang, t) in items {
                if lang.is_empty() || !xml_safe(lang) || !xml_safe(t) || t.contains('\r') || !seen.insert(lang) {
                    return None;
                }
                let mut it = XmlElement::new("Item", 0);
                it.attrs.insert("language".into(), lang.clone());
                it.text.clone_from(t);
                children.push(it);
            }
        }
    }
    e.text = text;
    e.children = children;
    Some(e)
}

/// The value as `parse_form_xml` reads it back from the element written for it; `None` when it cannot be
/// expressed (then the property is not written and does not take part in any comparison).
fn canonical(prop: Property, x: ExtVal) -> Option<ExtVal> {
    let x = match x {
        ExtVal::Font(f) if f.scale == Some(100) => ExtVal::Font(FontSpec { scale: None, ..f }),
        other => other,
    };
    if matches!(&x, ExtVal::Str(s) if s.contains('\r')) {
        return None;
    }
    let e = elem_of(prop.xml_name(), &x)?;
    Some(match &x {
        ExtVal::Color(_) => ExtVal::Color(parse_color_elem(&e)?),
        ExtVal::Font(_) => ExtVal::Font(parse_font_elem(&e)?),
        ExtVal::Shortcut(_) => ExtVal::Shortcut(parse_shortcut_elem(&e)?),
        ExtVal::Picture(_) => ExtVal::Picture(parse_picture_elem(&e)?),
        ExtVal::Loc(items) if items.is_empty() => return None,
        _ => x,
    })
}

fn write_elem(e: &XmlElement, indent: &str, out: &mut String) {
    out.push_str(indent);
    out.push('<');
    out.push_str(&e.tag);
    for (k, v) in &e.attrs {
        out.push(' ');
        out.push_str(k);
        out.push('=');
        out.push_str(&crate::oof::quoteattr(v));
    }
    if e.children.is_empty() {
        if e.text.is_empty() {
            out.push_str("/>\n");
        } else {
            out.push('>');
            out.push_str(&crate::oof::escape(&e.text));
            out.push_str(&format!("</{}>\n", e.tag));
        }
        return;
    }
    out.push_str(">\n");
    let inner = format!("{indent}  ");
    for c in &e.children {
        write_elem(c, &inner, out);
    }
    out.push_str(&format!("{indent}</{}>\n", e.tag));
}

/// The sparse extended-property elements of one control, in their fixed order.
fn control_block(entry: &ControlEntry, skip: &HashSet<&str>, indent: &str) -> String {
    let mut have = read_ext_values(entry);
    if let Some(d) = defaults(entry.type_name) {
        drop_defaults(&mut have, d);
    }
    let mut out = String::new();
    for &prop in EXT_ORDER {
        if skip.contains(prop.xml_name()) {
            continue;
        }
        if let Some(e) = val(&have, prop).and_then(|x| elem_of(prop.xml_name(), &x)) {
            write_elem(&e, indent, &mut out);
        }
    }
    out
}

/// Direct children of a control element in front of which the extended properties are written.
const ANCHORS: [&str; 5] = ["Columns", "Events", "InfoRecord", "PanelRecord", "ChildItems"];

/// Insert the extended properties of every control of `stream` into the decoder's `xml`. Controls of the XML are
/// matched to the stream by their `id` attribute; carriers and the elements the decoder already wrote stay as
/// they are.
pub fn enrich_ext_properties(stream: &[u8], xml: &[u8]) -> Result<Vec<u8>> {
    let source = parse_spanned(stream)?;
    let index = ControlIndex::build(&source)?;
    let text = std::str::from_utf8(xml).map_err(|e| layout_err(format!("Form.xml is not UTF-8: {e}")))?;
    let doc = roxmltree::Document::parse(text).map_err(|e| layout_err(format!("Form.xml is not well-formed: {e}")))?;
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for n in doc.descendants().filter(|n| n.is_element() && is_control_tag(n.tag_name().name())) {
        // `<Button>` items of a command bar's `<Buttons>` are not controls (their ids are the bar's own numbering)
        if n.ancestors().skip(1).any(|a| a.tag_name().name() == "Buttons") {
            continue;
        }
        let Some(entry) = n.attribute("id").and_then(|v| v.parse::<i64>().ok()).and_then(|id| index.get(id)) else { continue };
        if entry.type_name != n.tag_name().name() {
            continue;
        }
        let range = n.range();
        let line_start = text[..range.start].rfind('\n').map_or(0, |p| p + 1);
        let own_indent = &text[line_start..range.start];
        if !own_indent.bytes().all(|b| b == b' ') {
            continue;
        }
        let child_indent = format!("{own_indent}  ");
        // `<Picture>true</Picture>` (no attributes) is the decoder's marker, not the picture property.
        let skip: HashSet<&str> = n
            .children()
            .filter(|c| c.is_element())
            .filter(|c| !(c.tag_name().name() == "Picture" && c.attributes().len() == 0))
            .map(|c| c.tag_name().name())
            .collect();
        let block = control_block(entry, &skip, &child_indent);
        if block.is_empty() {
            continue;
        }
        let anchor = n.children().find(|c| c.is_element() && ANCHORS.contains(&c.tag_name().name()));
        if let Some(a) = anchor {
            let at = text[..a.range().start].rfind('\n').map_or(0, |p| p + 1);
            edits.push((at, at, block));
        } else if n.children().any(|c| c.is_element()) {
            let close = text[..range.end].rfind("</").unwrap_or(range.end);
            let at = text[..close].rfind('\n').map_or(0, |p| p + 1);
            edits.push((at, at, block));
        } else if text[..range.end].ends_with("/>") {
            let tail = format!(">\n{block}{own_indent}</{}>", n.tag_name().name());
            edits.push((range.end - 2, range.end, tail));
        }
    }
    edits.sort_by_key(|e| e.0);
    let mut out = String::with_capacity(text.len() + edits.iter().map(|e| e.2.len()).sum::<usize>());
    let mut last = 0;
    for (start, end, ins) in edits {
        out.push_str(&text[last..start]);
        out.push_str(&ins);
        last = end;
    }
    out.push_str(&text[last..]);
    Ok(out.into_bytes())
}
