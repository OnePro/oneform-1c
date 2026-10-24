//! `oneform explore`: slot-level mutation of ordinary-form streams for the property-exploration
//! conveyor (OpenSpec change `form-property-exploration-corpus`).
//!
//! An *experiment* (`ExperimentSpec`, a JSON document) names a control and one leaf slot of its
//! `info` or `geom` node using the same index-path addressing as `oneform props` (`info[2][0][3]`).
//! [`mutate_stream`] replaces exactly that leaf atom in the original stream via
//! [`crate::brace::apply_replacements`]; every other byte is preserved. Nodes and base64 blobs are
//! not mutable experiment targets.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::brace::{parse_spanned, AtomKind, Item, Replacement, Span, SpannedMetadata};
use crate::errors::{ExtractionError, Result, CONFIG_INVALID, IO_ERROR, METADATA_LAYOUT_UNSUPPORTED, PAYLOAD_MISSING};
use crate::form_patch::{ControlEntry, ControlIndex, NodeRef};

const USAGE: &str = "usage: oneform explore mutate --spec <exp.json> --stream <FormStream.bin> --out <dir>\n";

// ---------------------------------------------------------------- experiment spec

/// New value for a leaf slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotValue {
    /// Written as-is (must contain no brace-list delimiters or whitespace).
    Bare(String),
    /// Written with surrounding quotes; inner quotes are escaped as `""`.
    Quoted(String),
}

impl SlotValue {
    fn render(&self) -> Result<Vec<u8>> {
        match self {
            SlotValue::Bare(v) => {
                if v.is_empty() || v.bytes().any(|b| matches!(b, b'{' | b'}' | b',' | b'"' | b' ' | b'\t' | b'\r' | b'\n')) {
                    return Err(ExtractionError::new(
                        CONFIG_INVALID,
                        format!("bare value {v:?} is empty or contains a brace-list delimiter or whitespace"),
                    )
                    .with("value", v.as_str()));
                }
                Ok(v.clone().into_bytes())
            }
            SlotValue::Quoted(v) => Ok(format!("\"{}\"", v.replace('"', "\"\"")).into_bytes()),
        }
    }
}

/// Address of one leaf slot: `info[...]` or `geom[...]` (same indexing as `oneform props`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotPath {
    pub section: SlotSection,
    pub indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotSection {
    Info,
    Geom,
    Meta,
}

impl SlotSection {
    pub fn as_str(&self) -> &'static str {
        match self {
            SlotSection::Info => "info",
            SlotSection::Geom => "geom",
            SlotSection::Meta => "meta",
        }
    }
}

impl SlotPath {
    pub fn parse(text: &str) -> Result<SlotPath> {
        let bad = || {
            ExtractionError::new(
                CONFIG_INVALID,
                format!("slot {text:?} must look like 'info[2][0][3]' or 'geom[5]'"),
            )
            .with("slot", text)
        };
        let (section, rest) = if let Some(r) = text.strip_prefix("info") {
            (SlotSection::Info, r)
        } else if let Some(r) = text.strip_prefix("geom") {
            (SlotSection::Geom, r)
        } else if let Some(r) = text.strip_prefix("meta") {
            (SlotSection::Meta, r)
        } else {
            return Err(bad());
        };
        let mut indices = Vec::new();
        let mut r = rest;
        while !r.is_empty() {
            let Some(inner) = r.strip_prefix('[').and_then(|s| s.split_once(']')) else {
                return Err(bad());
            };
            let i: usize = inner.0.parse().map_err(|_| bad())?;
            indices.push(i);
            r = inner.1;
        }
        if indices.is_empty() {
            return Err(bad());
        }
        Ok(SlotPath { section, indices })
    }
}

/// One experiment: which control, which slot, which new value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentSpec {
    pub id: String,
    pub control: String,
    pub slot: SlotPath,
    pub new_value: SlotValue,
    pub hypothesis: String,
}

impl ExperimentSpec {
    pub fn parse_json(bytes: &[u8]) -> Result<ExperimentSpec> {
        let v: Value = serde_json::from_slice(bytes).map_err(|e| {
            ExtractionError::new(CONFIG_INVALID, format!("experiment spec is not valid JSON: {e}"))
        })?;
        let field = |name: &str| -> Result<&Value> {
            v.get(name).filter(|x| !x.is_null()).ok_or_else(|| {
                ExtractionError::new(CONFIG_INVALID, format!("experiment spec is missing required field '{name}'"))
                    .with("field", name)
            })
        };
        let string = |name: &str| -> Result<String> {
            field(name)?
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    ExtractionError::new(CONFIG_INVALID, format!("experiment spec field '{name}' must be a non-empty string"))
                        .with("field", name)
                })
        };
        let id = string("id")?;
        let control = string("control")?;
        let slot = SlotPath::parse(&string("slot")?)?;
        let new_value = field("newValue")?;
        let kind = new_value
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| ExtractionError::new(CONFIG_INVALID, "experiment spec field 'newValue.kind' must be a string").with("field", "newValue.kind"))?;
        let value = new_value
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| ExtractionError::new(CONFIG_INVALID, "experiment spec field 'newValue.value' must be a string").with("field", "newValue.value"))?
            .to_string();
        let new_value = match kind {
            "bare" => SlotValue::Bare(value),
            "quoted" => SlotValue::Quoted(value),
            other => {
                return Err(ExtractionError::new(
                    CONFIG_INVALID,
                    format!("experiment spec field 'newValue.kind' must be 'bare' or 'quoted', got {other:?}"),
                )
                .with("field", "newValue.kind")
                .with("kind", other))
            }
        };
        let hypothesis = v.get("hypothesis").and_then(Value::as_str).unwrap_or("").to_string();
        Ok(ExperimentSpec { id, control, slot, new_value, hypothesis })
    }
}

// ---------------------------------------------------------------- mutation

/// The control of `index` named `name`; errors name the ambiguity or the miss.
fn find_control<'a>(index: &'a ControlIndex, name: &str) -> Result<&'a ControlEntry<'a>> {
    let found: Vec<&ControlEntry> = index.entries.iter().filter(|e| e.name == name).collect();
    match found.as_slice() {
        [] => Err(ExtractionError::new(PAYLOAD_MISSING, format!("control '{name}' not found in the form stream"))
            .with("control", name)),
        [one] => Ok(one),
        many => Err(ExtractionError::new(
            METADATA_LAYOUT_UNSUPPORTED,
            format!("control name '{name}' is ambiguous: {} controls share it; experiments need a unique control name", many.len()),
        )
        .with("control", name)),
    }
}

/// Span and current text of the leaf atom addressed by `spec.slot` inside `entry`.
fn leaf_span(entry: &ControlEntry, slot: &SlotPath) -> Result<(Span, String, AtomKind)> {
    let what = format!("{} '{}'", slot.section.as_str(), entry.name);
    let mut cur: NodeRef = match slot.section {
        SlotSection::Info => entry.info(),
        SlotSection::Geom => entry.geom(),
        SlotSection::Meta => entry.meta(),
    }
    .ok_or_else(|| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, format!("control '{}' has no {} node", entry.name, slot.section.as_str())))?;
    let last = slot.indices.len() - 1;
    for (depth, &i) in slot.indices.iter().enumerate() {
        if depth == last {
            let Some((atom, span)) = cur.atom(i) else {
                let is_node = matches!(cur.node.children.get(i), Some(Item::Node(_)));
                let why = if is_node { "is a node, not a leaf atom" } else { "does not exist" };
                return Err(ExtractionError::new(
                    METADATA_LAYOUT_UNSUPPORTED,
                    format!("{what}[{i}] {why}; experiments can mutate only leaf atoms"),
                )
                .with("control", entry.name.as_str())
                .with("slot", format!("{}[{}]", slot.section.as_str(), slot.indices.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("]["))));
            };
            if atom.kind == AtomKind::Base64 {
                return Err(ExtractionError::new(
                    METADATA_LAYOUT_UNSUPPORTED,
                    format!("{what}[{i}] is a base64 blob; blobs are not mutable experiment targets"),
                )
                .with("control", entry.name.as_str()));
            }
            return Ok((span, atom.value.clone(), atom.kind));
        }
        cur = cur.child(i).ok_or_else(|| {
            ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, format!("{what}[{i}] does not exist or is not a node"))
                .with("control", entry.name.as_str())
        })?;
    }
    unreachable!("slot indices are never empty")
}

/// Apply `spec` to the original stream: exactly one leaf atom is replaced, every other byte preserved.
/// Returns `(patched bytes, old value)`.
pub fn mutate_stream(source: &SpannedMetadata, spec: &ExperimentSpec) -> Result<(Vec<u8>, String)> {
    let index = ControlIndex::build(source)?;
    let entry = find_control(&index, &spec.control)?;
    let (span, old, _kind) = leaf_span(entry, &spec.slot)?;
    let bytes = spec.new_value.render().map_err(|e| e.with("control", entry.name.as_str()))?;
    let out = source.apply(&[Replacement { range: span, bytes }])?;
    Ok((out, old))
}

// ---------------------------------------------------------------- CLI

/// `oneform explore mutate --spec <exp.json> --stream <FormStream.bin> --out <dir>`; returns
/// `(exit code, stdout, stderr)`; `args` excludes `explore` itself.
pub fn run(args: &[String]) -> (i32, String, String) {
    match execute(args) {
        Ok(out) => (0, out, String::new()),
        Err(e) => (1, String::new(), format!("oneform explore: error: {}\n", e.message)),
    }
}

fn execute(args: &[String]) -> Result<String> {
    match args.first().map(String::as_str) {
        Some("mutate") => execute_mutate(&args[1..]),
        Some("run") => execute_run(&args[1..]),
        Some("candidates") => execute_candidates(&args[1..]),
        Some("probe") => execute_probe(&args[1..]),
        _ => Err(ExtractionError::new(
            CONFIG_INVALID,
            "usage: oneform explore <mutate|run|candidates> ...",
        )),
    }
}

fn execute_probe(args: &[String]) -> Result<String> {
    let (mut epf, mut stream, mut base, mut platform, mut out) = (None, None, None, None, None);
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        let Some(value) = args.get(i + 1) else {
            return Err(ExtractionError::new(CONFIG_INVALID, format!("argument {a}: expected one argument")));
        };
        match a.as_str() {
            "--epf" => epf = Some(value.clone()),
            "--stream" => stream = Some(value.clone()),
            "--base" => base = Some(value.clone()),
            "--platform" => platform = Some(value.clone()),
            "--out" => out = Some(value.clone()),
            other => return Err(ExtractionError::new(CONFIG_INVALID, format!("unrecognized argument {other}"))),
        }
        i += 2;
    }
    let (Some(epf), Some(stream), Some(base), Some(platform), Some(out)) = (epf, stream, base, platform, out) else {
        return Err(ExtractionError::new(
            CONFIG_INVALID,
            "usage: oneform explore probe --epf <epf> --stream <FormStream.bin> --base <base> --platform <1cv8> --out <json>",
        ));
    };
    let epf_bytes = fs::read(&epf).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {epf}: {e}")))?;
    let stream_bytes = fs::read(&stream).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {stream}: {e}")))?;
    let rg = RuntimeGate {
        platform,
        base: Path::new(&base).to_path_buf(),
    };
    let rt = rg.run(&epf_bytes, &stream_bytes)?;
    fs::write(&out, serde_json::to_string_pretty(&rt.snapshot).unwrap())
        .map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot write {out}: {e}")))?;
    Ok(format!("probed {} properties in {}ms\n", rt.snapshot.len(), rt.duration_ms))
}

fn execute_mutate(rest: &[String]) -> Result<String> {
    let mut spec: Option<String> = None;
    let mut stream: Option<String> = None;
    let mut out: Option<String> = None;
    let mut i = 0;
    while i < rest.len() {
        let a = &rest[i];
        let Some(value) = rest.get(i + 1) else {
            return Err(ExtractionError::new(CONFIG_INVALID, format!("argument {a}: expected one argument\n{USAGE}")));
        };
        match a.as_str() {
            "--spec" => spec = Some(value.clone()),
            "--stream" => stream = Some(value.clone()),
            "--out" => out = Some(value.clone()),
            other => return Err(ExtractionError::new(CONFIG_INVALID, format!("unrecognized argument {other}\n{USAGE}"))),
        }
        i += 2;
    }
    let (Some(spec), Some(stream), Some(out)) = (spec, stream, out) else {
        return Err(ExtractionError::new(CONFIG_INVALID, format!("--spec, --stream and --out are required\n{USAGE}")));
    };
    let read = |path: &str| {
        fs::read(path).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {path}: {e}")).with("path", path))
    };
    let spec = ExperimentSpec::parse_json(&read(&spec)?).map_err(|e| e.with("spec", spec.as_str()))?;
    let source = parse_spanned(&read(&stream)?).map_err(|e| e.with("path", stream.as_str()))?;
    let (patched, old) = mutate_stream(&source, &spec)?;
    let out_dir = Path::new(&out);
    fs::create_dir_all(out_dir).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot create {out}: {e}")))?;
    let target = out_dir.join("mutated.bin");
    fs::write(&target, &patched).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot write {}: {e}", target.display())))?;
    Ok(format!(
        "{}: {} '{}' slot {}: {} -> {}\n",
        spec.id,
        spec.slot.section.as_str(),
        spec.control,
        spec.slot.indices.iter().map(|i| format!("[{i}]")).collect::<String>(),
        old,
        match &spec.new_value {
            SlotValue::Bare(v) => v.clone(),
            SlotValue::Quoted(v) => format!("\"{v}\""),
        }
    ))
}

// ---------------------------------------------------------------- EPF repack (design decision 3)

use crate::container::{deflate, inflate, parse_container, serialize_container, EntryOut, Limits};
use crate::props::diff_props;

/// The `form` stream of the single form inside an EPF. Errors when zero or several sub-containers
/// carry a `form` entry (exploration works one form at a time).
pub fn epf_form_stream(epf: &[u8]) -> Result<Vec<u8>> {
    let limits = Limits::default();
    let outer = parse_container(epf, &limits, 0, None)?;
    let mut found: Option<Vec<u8>> = None;
    for e in &outer.entries {
        let Ok(inner_raw) = inflate(&e.raw, &limits, None) else { continue };
        let Ok(sub) = parse_container(&inner_raw, &limits, 1, None) else { continue };
        if let Some(f) = sub.get("form") {
            if found.is_some() {
                return Err(ExtractionError::new(
                    METADATA_LAYOUT_UNSUPPORTED,
                    "the EPF has more than one form; exploration works one form at a time",
                ));
            }
            found = Some(f.raw.clone());
        }
    }
    found.ok_or_else(|| ExtractionError::new(PAYLOAD_MISSING, "the EPF has no form entry"))
}

/// The same EPF with the single form's stream replaced; every other entry, header and compression
/// boundary is preserved.
pub fn repack_epf_form(epf: &[u8], form: &[u8]) -> Result<Vec<u8>> {
    let limits = Limits::default();
    let outer = parse_container(epf, &limits, 0, None)?;
    let mut replaced = false;
    let mut owned: Vec<Vec<u8>> = Vec::with_capacity(outer.entries.len());
    for e in &outer.entries {
        let mut pushed = false;
        if !replaced {
            if let Ok(inner_raw) = inflate(&e.raw, &limits, None) {
                if let Ok(sub) = parse_container(&inner_raw, &limits, 1, None) {
                    if sub.get("form").is_some() {
                        let sub_out: Vec<EntryOut> = sub
                            .entries
                            .iter()
                            .map(|s| {
                                if s.name == "form" {
                                    (s.name.as_str(), form, Some(s.header.as_slice()))
                                } else {
                                    (s.name.as_str(), s.raw.as_slice(), Some(s.header.as_slice()))
                                }
                            })
                            .collect();
                        owned.push(deflate(&serialize_container(&sub_out, 0x200)?));
                        replaced = true;
                        pushed = true;
                    }
                }
            }
        }
        if !pushed {
            owned.push(e.raw.clone());
        }
    }
    if !replaced {
        return Err(ExtractionError::new(PAYLOAD_MISSING, "the EPF has no form entry to replace"));
    }
    let out_entries: Vec<EntryOut> = outer
        .entries
        .iter()
        .zip(&owned)
        .map(|(e, raw)| (e.name.as_str(), raw.as_slice(), Some(e.header.as_slice())))
        .collect();
    serialize_container(&out_entries, 0x200)
}

// ---------------------------------------------------------------- runtime probe (explore-runtime-property-sweep)

/// Candidate ordinary-form control properties probed in `ПриОткрытии()` via `Попытка ... Исключение`.
pub const RUNTIME_PROBED_PROPERTIES: &[&str] = &[
    "Видимость",
    "Доступность",
    "ТолькоПросмотр",
    "ПропускПриВводе",
    "ПропускатьПриВводе",
    "ПервыйВГруппе",
    "АвтоКонтекстноеМеню",
    "ВыбиратьТип",
    "РедактированиеТекста",
    "ГоризонтальноеПоложение",
    "ВертикальноеПоложение",
    "АктивизироватьПоУмолчанию",
    "КнопкаПоУмолчанию",
    "ИзменяетДанные",
    "АвтоПорядокОбхода",
    "ПорядокОбхода",
    "Заголовок",
    "ВыравниваниеНадписи",
    "ПоложениеЗаголовка",
    "Рамка",
    "ПрозрачныйФон",
    "АвтоРазмер",
    "Гиперссылка",
    "РежимОтображенияВыделения",
    "КнопкаВыбора",
    "КнопкаОчистки",
    "КнопкаОткрытия",
    "КнопкаРегулирования",
    "КнопкаВыпадающегоСписка",
    "КнопкаСпискаВыбора",
    "РежимВыбораИзСписка",
    "БыстрыйВыбор",
    "МногострочныйРежим",
    "РасширенноеРедактирование",
    "РежимПароля",
    "Маска",
    "СвязьПоТипу",
    "ЭлементСвязиПоТипу",
    "ВыборГруппИЭлементов",
    "ОграничениеТипа",
    "АвтоОтметкаНезаполненного",
    "АвтоВыборНезаполненного",
    "Подсказка",
    "ПоложениеКоманднойПанели",
    "ТриСостояния",
    "ПервоеСостояние",
    "ПоложениеФлажка",
    "Шапка",
    "Подвал",
    "ГоризонтальныеЛинии",
    "ВертикальныеЛинии",
    "Вывод",
    "РежимВыделения",
    "РежимВыбора",
    "ОтображатьЗаголовки",
    "ОтображатьПодвал",
    "АвтоВводНезаполненного",
    "АвтоВводНовойСтроки",
    "ИзменениеСоставаСтрок",
    "ИзменятьСоставСтрок",
    "ИзменятьПозициюКолонок",
    "ИзменятьНастройкуКолонок",
    "НачальноеОтображениеСписка",
    "ДобавлениеСтрок",
    "ГоризонтальнаяПолосаПрокрутки",
    "ВертикальнаяПолосаПрокрутки",
    "АвтоПереносСтрок",
    "МногострочныйЗаголовок",
    "ОтображениеТаблицы",
    "НачальноеОтображениеДерева",
    "РазрешитьНастройкуСписка",
    "ИспользованиеТекущейСтроки",
    "РежимЗаписиДаты",
    "ВыделятьОтрицательные",
    "РежимМеню",
    "ПоложениеКартинки",
    "ПоложениеКартинкиНадписи",
    "ПоложениеКартинкиПанели",
    "РазмерКартинки",
    "Формат",
    "БегущаяСтрока",
    "ВыбираемоеЗначение",
    "ВысотаСпискаВыбора",
    "ШиринаСпискаВыбора",
    "ВыборНезаполненного",
    "РежимВыбораНезаполненного",
    "ОтметкаНезаполненного",
    "МинимальноеЗначение",
    "МаксимальноеЗначение",
    "РаспределятьПоСтраницам",
    "РежимПрокручиваемыхСтраниц",
    "Свертка",
    "Вспомогательная",
    "Ориентация",
    "ФиксацияСлева",
    "ФиксацияСправа",
    "ВысотаШапки",
    "ВысотаПодвала",
    "ЧередованиеЦветовСтрок",
    "РежимВводаСтрок",
    "РежимВыделенияСтроки",
    "РазрешитьНачалоПеретаскивания",
    "РазрешитьПеретаскивание",
    "Масштабировать",
    "ТекстНевыбраннойКартинки",
    "ИспользоватьКонтекстноеМеню",
    "ОтображатьВыделение",
    "ЦветФонаПоля",
    "ЦветТекстаПоля",
    "ЦветФонаКнопки",
    "ЦветТекстаКнопки",
    "ЦветФонаВыделения",
    "ЦветТекстаВыделения",
    "ЦветФонаЧередованияСтрок",
    "ЦветФонаШапки",
    "ЦветТекстаШапки",
    "ЦветФонаПодвала",
    "ЦветТекстаПодвала",
    "Шаг",
    "БольшойШаг",
    "ШагРазметки",
    "ОтображениеРазметки",
    "ОтображатьПроценты",
    "СтильОтображения",
    "ОтображатьПометку",
    "ОтображатьКартинку",
    "ОтображатьТекущуюДату",
    "ПеремещениеПоКалендарю",
];

pub const FORM_PROBED_PROPERTIES: &[&str] = &[
    "Заголовок",
    "АвтоЗаголовок",
    "ИзменениеРазмера",
    "ПоложениеОкна",
    "СостояниеОкна",
    "СпособОтображенияОкна",
    "ИзменятьСпособОтображенияОкна",
    "ПоведениеКлавишиEnter",
    "ТолькоПросмотр",
    "ЗакрыватьПриВыборе",
    "ЗакрыватьПриЗакрытииВладельца",
    "РазрешитьЗакрытие",
    "РежимРабочегоСтола",
    "КлючСохраненияПоложенияОкна",
    "ИмяСохраненияПоложенияОкна",
    "СоединяемоеОкно",
    "РазрешитьСоединятьОкно",
    "РазрешитьСостояниеОбычное",
    "РазрешитьСостояниеПрикрепленное",
    "РазрешитьСостояниеСвободное",
    "РазрешитьСостояниеПрячущееся",
    "ПоложениеПрикрепленногоОкна",
];

pub const COLUMN_PROBED_PROPERTIES: &[&str] = &[
    "ГоризонтальноеПоложениеВКолонке",
    "ГоризонтальноеПоложениеВШапке",
    "ГоризонтальноеПоложениеВПодвале",
    "Формат",
    "РежимРедактирования",
    "Положение",
    "ИзменениеРазмера",
    "ИзменятьВидимость",
    "ИзменятьПозицию",
    "ИзменятьНастройку",
    "ОтображатьВШапке",
    "ОтображатьВПодвале",
    "ОтображатьИерархию",
    "АвтоВысотаЯчейки",
    "ВысотаЯчейки",
    "АвтоОтметкаНезаполненного",
    "ВыделятьОтрицательные",
    "Гиперссылка",
    "ТриСостоянияФлажка",
    "ДанныеФлажка",
    "ЦветФонаПоля",
    "ЦветТекстаПоля",
    "ЦветФонаШапки",
    "ЦветТекстаШапки",
    "ЦветФонаПодвала",
    "ЦветТекстаПодвала",
];

/// Generate the `Процедура ПриОткрытии()` BSL body that writes `<Control>.<Prop>=<Val>` lines to
/// `out_path` and exits 1C via `ЗавершитьРаботуСистемы(Ложь)`.
pub fn generate_runtime_probe_bsl(out_path: &Path) -> String {
    let escaped = out_path.to_string_lossy().replace('"', "\"\"");
    let mut lines = vec![
        "Процедура ПриОткрытии()".to_string(),
        "\tТ = Новый ТекстовыйДокумент;".to_string(),
    ];
    for prop in FORM_PROBED_PROPERTIES {
        lines.push(format!(
            "\tПопытка Т.ДобавитьСтроку(\"ЭтаФорма.{prop}=\" + Строка(ЭтаФорма.{prop})); Исключение КонецПопытки;"
        ));
    }
    lines.push("\tДля Каждого Э Из ЭлементыФормы Цикл".to_string());
    for prop in RUNTIME_PROBED_PROPERTIES {
        lines.push(format!(
            "\t\tПопытка Т.ДобавитьСтроку(Э.Имя + \".{prop}=\" + Строка(Э.{prop})); Исключение КонецПопытки;"
        ));
    }
    lines.push("\t\tПопытка Для Каждого К Из Э.Колонки Цикл".to_string());
    for prop in COLUMN_PROBED_PROPERTIES {
        lines.push(format!(
            "\t\t\tПопытка Т.ДобавитьСтроку(Э.Имя + \".\" + К.Имя + \".{prop}=\" + Строка(К.{prop})); Исключение КонецПопытки;"
        ));
    }
    lines.push("\t\tКонецЦикла; Исключение КонецПопытки;".to_string());
    lines.push("\tКонецЦикла;".to_string());
    lines.push(format!("\tТ.Записать(\"{escaped}\", КодировкаТекста.UTF8);"));
    lines.push("КонецПроцедуры".to_string());
    lines.join("\r\n")
}

fn patch_bsl_on_open(module_raw: &[u8], out_path: &Path) -> Result<Vec<u8>> {
    let has_bom = module_raw.starts_with(&[0xEF, 0xBB, 0xBF]);
    let body = if has_bom { &module_raw[3..] } else { module_raw };
    let text = String::from_utf8(body.to_vec()).map_err(|e| {
        ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, format!("form module is not valid UTF-8: {e}"))
    })?;
    let probe = generate_runtime_probe_bsl(out_path);
    let patched = if let Some(start) = text.find("Процедура ПриОткрытии(") {
        let rest = &text[start..];
        let end_kw = "КонецПроцедуры";
        let rel_end = rest.find(end_kw).ok_or_else(|| {
            ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, "unterminated Процедура ПриОткрытии in form module")
        })?;
        let mut s = String::with_capacity(text.len() + probe.len());
        s.push_str(&text[..start]);
        s.push_str(&probe);
        s.push_str(&rest[rel_end + end_kw.len()..]);
        s
    } else {
        format!("{text}\r\n\r\n{probe}\r\n")
    };
    let mut out = Vec::with_capacity(patched.len() + 3);
    out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    out.extend_from_slice(patched.as_bytes());
    Ok(out)
}

/// Repack the EPF with both a replaced `form` stream and an instrumented `module` entry whose
/// `ПриОткрытии()` writes runtime control properties to `out_path` and exits 1C.
pub fn repack_epf_form_with_probe(epf: &[u8], form: &[u8], out_path: &Path) -> Result<Vec<u8>> {
    let limits = Limits::default();
    let outer = parse_container(epf, &limits, 0, None)?;
    let mut replaced = false;
    let mut owned: Vec<Vec<u8>> = Vec::with_capacity(outer.entries.len());
    for e in &outer.entries {
        let mut pushed = false;
        if !replaced {
            if let Ok(inner_raw) = inflate(&e.raw, &limits, None) {
                if let Ok(sub) = parse_container(&inner_raw, &limits, 1, None) {
                    if let (Some(_), Some(m)) = (sub.get("form"), sub.get("module")) {
                        let new_mod = patch_bsl_on_open(&m.raw, out_path)?;
                        let sub_out: Vec<EntryOut> = sub
                            .entries
                            .iter()
                            .map(|s| match s.name.as_str() {
                                "form" => (s.name.as_str(), form, Some(s.header.as_slice())),
                                "module" => (s.name.as_str(), new_mod.as_slice(), Some(s.header.as_slice())),
                                _ => (s.name.as_str(), s.raw.as_slice(), Some(s.header.as_slice())),
                            })
                            .collect();
                        owned.push(deflate(&serialize_container(&sub_out, 0x200)?));
                        replaced = true;
                        pushed = true;
                    }
                }
            }
        }
        if !pushed {
            owned.push(e.raw.clone());
        }
    }
    if !replaced {
        return Err(ExtractionError::new(
            PAYLOAD_MISSING,
            "the EPF has no form+module sub-container to instrument",
        ));
    }
    let out_entries: Vec<EntryOut> = outer
        .entries
        .iter()
        .zip(&owned)
        .map(|(e, raw)| (e.name.as_str(), raw.as_slice(), Some(e.header.as_slice())))
        .collect();
    serialize_container(&out_entries, 0x200)
}

/// Result of one `ENTERPRISE /Execute` runtime probe run.
#[derive(Debug, Clone)]
pub struct RuntimeResult {
    pub snapshot: std::collections::BTreeMap<String, String>,
    pub duration_ms: u128,
}

/// Runtime introspection gate: launches `1cv8.exe ENTERPRISE /Execute <epf>` against a disposable
/// copy of the infobase with an instrumented `ПриОткрытии()` handler that writes runtime properties
/// to disk and exits.
pub struct RuntimeGate {
    /// `1cv8.exe` path or its `bin` directory.
    pub platform: String,
    /// Infobase directory whose `1Cv8.1CD` is copied into a disposable working base.
    pub base: std::path::PathBuf,
}

impl RuntimeGate {
    fn exe(&self) -> String {
        let p = Path::new(&self.platform);
        if p.is_dir() {
            p.join("1cv8.exe").to_string_lossy().into_owned()
        } else {
            self.platform.clone()
        }
    }

    /// Run the instrumented EPF for `form` and return its parsed runtime property snapshot.
    pub fn run(&self, epf: &[u8], form: &[u8]) -> Result<RuntimeResult> {
        let started = std::time::Instant::now();
        let work = std::env::temp_dir().join(format!(
            "oneform-rt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let base = work.join("base");
        fs::create_dir_all(&base).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot create {}: {e}", base.display())))?;
        fs::copy(self.base.join("1Cv8.1CD"), base.join("1Cv8.1CD"))
            .map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot copy the infobase: {e}")))?;
        let out_file = work.join("runtime.txt");
        let probed = repack_epf_form_with_probe(epf, form, &out_file)?;
        let epf_path = work.join("probe.epf");
        fs::write(&epf_path, &probed).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot write {}: {e}", epf_path.display())))?;

        let result = self.launch_and_collect(&base, &epf_path, &out_file, started);
        if std::env::var_os("ONEFORM_EXPLORE_KEEP_WORK").is_none() {
            let _ = fs::remove_dir_all(&work);
        }
        result
    }

    fn launch_and_collect(
        &self,
        base: &Path,
        epf_path: &Path,
        out_file: &Path,
        started: std::time::Instant,
    ) -> Result<RuntimeResult> {
        let exe = self.exe();
        let script = format!(
            concat!(
                "Stop-Process -Name 1cv8* -Force -ErrorAction SilentlyContinue\n",
                "$exe = '{}'\n",
                "$arg = 'ENTERPRISE /F \"{}\" /RunModeOrdinaryApplication /Execute \"{}\" /DisableStartupDialogs /DisableStartupMessages'\n",
                "$sess = (Get-Process -Id $PID).SessionId\n",
                "if ($sess -eq 0) {{\n",
                "  $name = 'OneFormRtProbe'\n",
                "  Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue | Out-Null\n",
                "  $action = New-ScheduledTaskAction -Execute $exe -Argument $arg\n",
                "  $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive\n",
                "  Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Force | Out-Null\n",
                "  Start-ScheduledTask -TaskName $name\n",
                "}} else {{\n",
                "  Start-Process -FilePath $exe -ArgumentList $arg | Out-Null\n",
                "}}\n",
                "for ($i = 0; $i -lt 50; $i++) {{\n",
                "  if (Test-Path '{}') {{ Start-Sleep -Milliseconds 150; break }}\n",
                "  Start-Sleep -Milliseconds 250\n",
                "}}\n",
                "Stop-Process -Name 1cv8* -Force -ErrorAction SilentlyContinue\n",
                "Unregister-ScheduledTask -TaskName 'OneFormRtProbe' -Confirm:$false -ErrorAction SilentlyContinue | Out-Null\n",
                "if (-not (Test-Path '{}')) {{\n",
                "  Stop-Process -Name 1cv8* -Force -ErrorAction SilentlyContinue\n",
                "  exit 2\n",
                "}}\n"
            ),
            exe.replace('\'', "''"),
            base.to_string_lossy().replace('\'', "''"),
            epf_path.to_string_lossy().replace('\'', "''"),
            out_file.to_string_lossy().replace('\'', "''"),
            out_file.to_string_lossy().replace('\'', "''"),
        );
        let out = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &script])
            .output()
            .map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot run powershell for RuntimeGate: {e}")))?;
        if !out_file.is_file() {
            return Err(ExtractionError::new(
                METADATA_LAYOUT_UNSUPPORTED,
                format!(
                    "runtime probe did not produce output within timeout (exit={:?}): {}",
                    out.status.code(),
                    String::from_utf8_lossy(&out.stderr)
                ),
            ));
        }
        let raw = fs::read(out_file).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {}: {e}", out_file.display())))?;
        Ok(RuntimeResult {
            snapshot: parse_runtime_snapshot(&raw),
            duration_ms: started.elapsed().as_millis(),
        })
    }
}

/// Parse `<Control>.<Prop>=<Val>` lines from a UTF-8 runtime snapshot file into a sorted map.
pub fn parse_runtime_snapshot(bytes: &[u8]) -> std::collections::BTreeMap<String, String> {
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let text = String::from_utf8_lossy(body);
    let mut map = std::collections::BTreeMap::new();
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some((k, v)) = line.split_once('=') {
            if !k.is_empty() {
                map.insert(k.to_string(), v.to_string());
            }
        }
    }
    map
}

/// Diff two runtime property snapshots (`baseline` vs `current`), optionally filtered to one
/// control prefix (`<control>.`).
pub fn diff_runtime(
    baseline: &std::collections::BTreeMap<String, String>,
    current: &std::collections::BTreeMap<String, String>,
    control: Option<&str>,
) -> Vec<(String, String, String)> {
    let prefix = control.map(|c| format!("{c}."));
    let keep = |k: &str| prefix.as_ref().is_none_or(|p| k.starts_with(p.as_str()));
    let mut keys: Vec<&String> = baseline.keys().chain(current.keys()).filter(|k| keep(k)).collect();
    keys.sort();
    keys.dedup();
    let mut out = Vec::new();
    for k in keys {
        match (baseline.get(k), current.get(k)) {
            (Some(a), Some(b)) if a != b => out.push((k.clone(), a.clone(), b.clone())),
            (Some(a), None) => out.push((k.clone(), a.clone(), "<missing>".into())),
            (None, Some(b)) => out.push((k.clone(), "<missing>".into(), b.clone())),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------- platform gate (design decision 3)

/// Outcome of one experiment after the platform gate and survival check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Accept,
    Reject,
    SilentDrop,
}

impl Outcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Accept => "accept",
            Outcome::Reject => "reject",
            Outcome::SilentDrop => "silent-drop",
        }
    }
}

/// Result of gating one EPF through the platform.
#[derive(Debug)]
pub struct GateResult {
    pub accepted: bool,
    /// Form stream of the platform-normalized EPF (present on accept).
    pub normalized: Option<Vec<u8>>,
    pub log: String,
    pub duration_ms: u128,
}

/// Batch Designer gate: `dump -> load` against a disposable copy of the source infobase.
pub struct PlatformGate {
    /// `1cv8.exe` path or its `bin` directory.
    pub platform: String,
    /// Infobase directory whose `1Cv8.1CD` is copied per gate run (never modified).
    pub base: std::path::PathBuf,
}

impl PlatformGate {
    fn exe(&self) -> String {
        let p = Path::new(&self.platform);
        if p.is_dir() {
            p.join("1cv8.exe").to_string_lossy().into_owned()
        } else {
            self.platform.clone()
        }
    }

    fn designer(&self, base: &Path, extra: &[String], log: &Path) -> Result<(bool, String)> {
        // 1C's own command-line parser requires quoted values: /F"C:\base", /Out"C:\log".
        let q = |s: String| format!("\"{s}\"");
        let mut args: Vec<String> = vec![
            "DESIGNER".into(),
            "/F".into(),
            q(base.to_string_lossy().into_owned()),
        ];
        args.extend(extra.iter().cloned());
        args.push("/Out".into());
        args.push(q(log.to_string_lossy().into_owned()));
        args.push("/DisableStartupDialogs".into());
        let mut cmd = std::process::Command::new(self.exe());
        // 1C's own command-line parser expects pre-quoted values; pass every argument verbatim,
        // bypassing Command's re-quoting (which would escape the embedded quotes 1C requires).
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            for a in &args {
                cmd.raw_arg(a);
            }
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        #[cfg(not(windows))]
        {
            cmd.args(&args);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot start 1cv8: {e}")))?;
        let mut status = None;
        for _ in 0..180 {
            if let Some(st) = child
                .try_wait()
                .map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot wait for 1cv8: {e}")))?
            {
                status = Some(st);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        let Some(status) = status else {
            let _ = child.kill();
            let _ = child.wait();
            return Ok((false, "designer timed out".into()));
        };
        let mut log_text = fs::read_to_string(log).unwrap_or_default();
        if log_text.is_empty() {
            log_text = format!("exit={}", status.code().unwrap_or(-1));
        }
        Ok((status.success() && !log_text.contains("Ошибка"), log_text))
    }

    /// Gate one EPF: copy the base, dump, reload, return the normalized form stream on accept.
    pub fn run(&self, epf: &[u8]) -> Result<GateResult> {
        let started = std::time::Instant::now();
        let work = std::env::temp_dir().join(format!("oneform-explore-{}-{}", std::process::id(), started.elapsed().as_nanos()));
        let base = work.join("base");
        fs::create_dir_all(&base).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot create {}: {e}", base.display())))?;
        fs::copy(self.base.join("1Cv8.1CD"), base.join("1Cv8.1CD"))
            .map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot copy the infobase: {e}")))?;
        let epf_path = work.join("experiment.epf");
        fs::write(&epf_path, epf).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot write the experiment EPF: {e}")))?;
        let dump_dir = work.join("dump");
        fs::create_dir_all(&dump_dir).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot create {}: {e}", dump_dir.display())))?;
        let result = (|| -> Result<GateResult> {
            let (ok, log1) = self.designer(
                &base,
                &[
                    "/DumpExternalDataProcessorOrReportToFiles".into(),
                    format!("\"{}\"", dump_dir.to_string_lossy()),
                    format!("\"{}\"", epf_path.to_string_lossy()),
                    "-Format".into(),
                    "Hierarchical".into(),
                ],
                &work.join("dump_log.txt"),
            )?;
            // exit 0 with an empty dump directory is a false success (platform quirk)
            let dump_ok = ok && fs::read_dir(&dump_dir).map(|mut d| d.next().is_some()).unwrap_or(false);
            if !dump_ok {
                return Ok(GateResult { accepted: false, normalized: None, log: log1, duration_ms: started.elapsed().as_millis() });
            }
            let root_xml = fs::read_dir(&dump_dir)
                .map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {}: {e}", dump_dir.display())))?
                .flatten()
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|x| x == "xml"))
                .ok_or_else(|| ExtractionError::new(PAYLOAD_MISSING, "the platform dump produced no root XML"))?;
            let out_epf = work.join("normalized.epf");
            let (ok2, log2) = self.designer(
                &base,
                &[
                    "/LoadExternalDataProcessorOrReportFromFiles".into(),
                    format!("\"{}\"", root_xml.to_string_lossy()),
                    format!("\"{}\"", out_epf.to_string_lossy()),
                ],
                &work.join("load_log.txt"),
            )?;
            let log = format!("{log1}\n{log2}");
            // same false-success guard for the load step
            if !ok2 || !out_epf.is_file() {
                return Ok(GateResult { accepted: false, normalized: None, log, duration_ms: started.elapsed().as_millis() });
            }
            let normalized = epf_form_stream(&fs::read(&out_epf).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read the normalized EPF: {e}")))?)?;
            Ok(GateResult { accepted: true, normalized: Some(normalized), log, duration_ms: started.elapsed().as_millis() })
        })();
        if std::env::var_os("ONEFORM_EXPLORE_KEEP_WORK").is_none() {
            let _ = fs::remove_dir_all(&work);
        } else {
            eprintln!("explore gate work kept: {}", work.display());
        }
        result
    }
}

/// The mutated control must survive the platform round-trip: same id and type, node not truncated
/// relative to the mutated stream. Returns `false` for a silent drop.
pub fn control_survives(mutated: &[u8], normalized: &[u8], control: &str) -> Result<bool> {
    let before_src = parse_spanned(mutated)?;
    let after_src = parse_spanned(normalized)?;
    let before = ControlIndex::build(&before_src)?;
    let after = ControlIndex::build(&after_src)?;
    let Some(b) = before.entries.iter().find(|e| e.name == control) else {
        return Err(ExtractionError::new(PAYLOAD_MISSING, format!("control '{control}' is missing from the mutated stream")));
    };
    let Some(a) = after.entries.iter().find(|e| e.id == b.id) else { return Ok(false) };
    if a.type_name != b.type_name {
        return Ok(false);
    }
    // Truncation: the platform kept the control but cut its info/geom nodes.
    let info_len = |e: &ControlEntry| e.info().map(|i| i.node.len()).unwrap_or(0);
    let geom_len = |e: &ControlEntry| e.geom().map(|g| g.node.len()).unwrap_or(0);
    Ok(info_len(a) >= info_len(b) && geom_len(a) >= geom_len(b))
}

// ---------------------------------------------------------------- baseline and net diff (design decision 3)

/// UUID-shaped diff values are regenerated by the platform on every run; mask them for comparisons.
fn mask_uuid(v: &str) -> String {
    let is_uuid = v.len() == 36
        && v.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        });
    if is_uuid { "<uuid>".to_string() } else { v.to_string() }
}

fn mask_row(r: &(String, String, String)) -> (String, String, String) {
    (r.0.clone(), mask_uuid(&r.1), mask_uuid(&r.2))
}

/// `observed` minus `baseline`, key-by-key on UUID-masked `(path, old, new)` rows; rows under a
/// volatile path prefix (baseline instability, see [`measure_baseline`]) are excluded from both
/// sides. Returned rows keep their original (unmasked) values for forensics.
pub fn net_diff(
    observed: &[(String, String, String)],
    baseline: &[(String, String, String)],
    volatile: &[String],
) -> Vec<(String, String, String)> {
    let stable = |r: &(String, String, String)| !volatile.iter().any(|p| r.0.starts_with(p.as_str()));
    let base: Vec<(String, String, String)> = baseline.iter().filter(|r| stable(r)).map(mask_row).collect();
    observed.iter().filter(|r| stable(r)).filter(|r| !base.contains(&mask_row(r))).cloned().collect()
}

// ---------------------------------------------------------------- semantic observation

/// Deep JSON diff of two values: `(path, old, new)` leaves that differ.
fn json_diff(a: &Value, b: &Value, path: &str, out: &mut Vec<(String, String, String)>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                match (x.get(k), y.get(k)) {
                    (Some(vx), Some(vy)) => json_diff(vx, vy, &format!("{path}.{k}"), out),
                    (Some(vx), None) => out.push((format!("{path}.{k}"), vx.to_string(), "<missing>".into())),
                    (None, Some(vy)) => out.push((format!("{path}.{k}"), "<missing>".into(), vy.to_string())),
                    _ => {}
                }
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (vx, vy)) in x.iter().zip(y).enumerate() {
                json_diff(vx, vy, &format!("{path}[{i}]"), out);
            }
        }
        _ => {
            if a != b {
                out.push((path.to_string(), a.to_string(), b.to_string()));
            }
        }
    }
}

/// Semantic diff of one control between two streams, through the normal OOF decoder: when the
/// mutated slot is a property the decoder already knows, the change shows up here by name.
pub fn decoded_control_diff(original: &[u8], mutated: &[u8], control: &str) -> Result<Vec<(String, String, String)>> {
    let find = |raw: &[u8]| -> Result<Value> {
        let model = crate::oof::decode_oof_stream(raw, "explore", None).map_err(|e| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, e.to_string()))?;
        let mut stack: Vec<&Value> = model["controls"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
        while let Some(c) = stack.pop() {
            if c["name"].as_str() == Some(control) {
                return Ok(c.clone());
            }
            if let Some(ch) = c["children"].as_array() {
                stack.extend(ch.iter());
            }
        }
        Err(ExtractionError::new(PAYLOAD_MISSING, format!("control '{control}' not found in the decoded form")))
    };
    let mut out = Vec::new();
    json_diff(&find(original)?, &find(mutated)?, control, &mut out);
    Ok(out)
}

// ---------------------------------------------------------------- conveyor (`explore run`)

/// Everything one experiment leaves on disk (design decision 4).
pub struct Fixture {
    pub manifest: Value,
    pub original: Vec<u8>,
    pub mutated: Vec<u8>,
    pub normalized: Option<Vec<u8>>,
    pub observed: Vec<(String, String, String)>,
    pub net: Vec<(String, String, String)>,
}

fn diff_to_json(rows: &[(String, String, String)]) -> Value {
    Value::Array(rows.iter().map(|(p, a, b)| serde_json::json!({"path": p, "from": a, "to": b})).collect())
}

/// Run one experiment (no baseline handling here) and write its fixture under `corpus/<id>/`.
pub fn run_experiment(
    gate: Option<&PlatformGate>,
    epf: &[u8],
    original: &[u8],
    spec: &ExperimentSpec,
    baseline: &Baseline,
    corpus: &Path,
    platform_version: &str,
) -> Result<Outcome> {
    run_experiment_with_runtime(gate, None, epf, original, spec, baseline, corpus, platform_version)
}

/// Run one experiment with an optional [`RuntimeGate`] (`ENTERPRISE /Execute`) and write its fixture.
#[allow(clippy::too_many_arguments)]
pub fn run_experiment_with_runtime(
    gate: Option<&PlatformGate>,
    runtime_gate: Option<&RuntimeGate>,
    epf: &[u8],
    original: &[u8],
    spec: &ExperimentSpec,
    baseline: &Baseline,
    corpus: &Path,
    platform_version: &str,
) -> Result<Outcome> {
    let source = parse_spanned(original)?;
    let (mutated, old) = mutate_stream(&source, spec)?;
    let dir = corpus.join(&spec.id);
    fs::create_dir_all(&dir).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot create {}: {e}", dir.display())))?;
    let write = |name: &str, data: &[u8]| -> Result<()> {
        fs::write(dir.join(name), data).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot write {name}: {e}")))
    };
    write("original.bin", original)?;
    write("mutated.bin", &mutated)?;
    let (outcome, normalized, observed, duration) = match gate {
        None => {
            let obs = diff_props(original, &mutated, Some(&spec.control))
                .map_err(|e| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, e.0))?;
            (Outcome::Accept, None, obs, 0)
        }
        Some(g) => {
            let repacked = repack_epf_form(epf, &mutated)?;
            let gres = g.run(&repacked)?;
            if !gres.accepted {
                write("designer.log", gres.log.as_bytes())?;
                (Outcome::Reject, None, Vec::new(), gres.duration_ms)
            } else {
                let norm = gres.normalized.expect("accepted gate has a normalized stream");
                let survived = control_survives(&mutated, &norm, &spec.control)?;
                let obs = diff_props(&mutated, &norm, Some(&spec.control))
                    .map_err(|e| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, e.0))?;
                let outcome = if survived { Outcome::Accept } else { Outcome::SilentDrop };
                (outcome, Some(norm), obs, gres.duration_ms)
            }
        }
    };
    if let Some(n) = &normalized {
        write("normalized.bin", n)?;
    }
    let net = net_diff(&observed, &baseline.rows, &baseline.volatile);
    write("observed.json", serde_json::to_string_pretty(&diff_to_json(&observed)).unwrap().as_bytes())?;
    write("net-diff.json", serde_json::to_string_pretty(&diff_to_json(&net)).unwrap().as_bytes())?;
    // Semantic observation: what the decoder already understands about the mutated slot.
    let decoded = decoded_control_diff(original, &mutated, &spec.control)?;
    write("decoded-diff.json", serde_json::to_string_pretty(&diff_to_json(&decoded)).unwrap().as_bytes())?;
    let runtime_val = match (outcome, runtime_gate, &baseline.runtime) {
        (Outcome::Accept, Some(rg), Some(base_rt)) => {
            let rt_stream = normalized.as_deref().unwrap_or(&mutated);
            let rres = rg.run(epf, rt_stream)?;
            let rdiff = diff_runtime(base_rt, &rres.snapshot, Some(&spec.control));
            write("runtime-diff.json", serde_json::to_string_pretty(&diff_to_json(&rdiff)).unwrap().as_bytes())?;
            serde_json::json!({
                "platform": platform_version,
                "durationMs": rres.duration_ms as u64,
                "changedCount": rdiff.len(),
            })
        }
        _ => Value::Null,
    };
    let kind = match &spec.new_value {
        SlotValue::Bare(_) => "bare",
        SlotValue::Quoted(_) => "quoted",
    };
    let manifest = serde_json::json!({
        "schema": 1,
        "id": spec.id,
        "control": spec.control,
        "slot": format!("{}[{}]", spec.slot.section.as_str(), spec.slot.indices.iter().map(|i| i.to_string()).collect::<Vec<_>>().join("][")),
        "oldValue": {"kind": kind, "value": old},
        "newValue": {"kind": kind, "value": match &spec.new_value { SlotValue::Bare(v) | SlotValue::Quoted(v) => v.clone() }},
        "hypothesis": spec.hypothesis,
        "outcome": outcome.as_str(),
        "controlSurvived": outcome != Outcome::SilentDrop,
        "designer": {"platform": platform_version, "durationMs": duration as u64},
        "runtime": runtime_val,
    });
    write("manifest.json", serde_json::to_string_pretty(&manifest).unwrap().as_bytes())?;
    Ok(outcome)
}

/// Result of the baseline measurement: the platform's own normalization delta plus the volatile
/// path prefixes (`<control>.info[...]` / `<control>.geom[...]` subtrees) in which two runs of the
/// unchanged donor disagree (the platform regenerates some orderings from random GUIDs).
#[derive(Debug, Clone)]
pub struct Baseline {
    pub rows: Vec<(String, String, String)>,
    pub volatile: Vec<String>,
    pub runtime: Option<std::collections::BTreeMap<String, String>>,
}

/// The baseline (experiment zero): gate the unmutated donor twice. Rows that differ between the two
/// runs (after UUID masking) mark their `<control>.<section>[i]` subtree volatile and are excluded
/// from every experiment's net diff. Aborts when more than half of all baseline rows are volatile —
/// that much noise makes the corpus meaningless.
pub fn measure_baseline(gate: &PlatformGate, epf: &[u8], original: &[u8]) -> Result<Baseline> {
    let mut runs: Vec<Vec<(String, String, String)>> = Vec::new();
    for attempt in 1..=2 {
        let g = gate.run(epf)?;
        if !g.accepted {
            return Err(ExtractionError::new(
                METADATA_LAYOUT_UNSUPPORTED,
                format!("baseline run {attempt} was rejected by the platform; the donor itself must round-trip before experiments can run\n{}", g.log),
            ));
        }
        let norm = g.normalized.expect("accepted");
        runs.push(diff_props(original, &norm, None).map_err(|e| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, e.0))?);
    }
    let masked: Vec<Vec<(String, String, String)>> = runs.iter().map(|r| r.iter().map(mask_row).collect()).collect();
    // one-direction symmetric difference, deduplicated: rows of run 1 that run 2 does not have
    let mut unstable: Vec<(String, String, String)> = Vec::new();
    for r in masked[0].iter().chain(&masked[1]) {
        if (!masked[0].contains(r) || !masked[1].contains(r)) && !unstable.contains(r) {
            unstable.push(r.clone());
        }
    }
    // volatile prefix = control label + section + first index (e.g. "КоманднаяПанель2.info[1]")
    let mut volatile: Vec<String> = unstable
        .iter()
        .filter_map(|r| {
            let (ctl, rest) = r.0.split_once('.')?;
            let end = rest.find(']')?;
            Some(format!("{ctl}.{}", &rest[..=end]))
        })
        .collect();
    volatile.sort();
    volatile.dedup();
    // Abort only when the noise is systemic: every control with a baseline row is volatile.
    // A few volatile subtrees (CommandBar button order, regenerated GUIDs) are excluded instead.
    let controls: std::collections::BTreeSet<&str> = runs[0].iter().filter_map(|r| r.0.split('.').next()).collect();
    let volatile_controls: std::collections::BTreeSet<&str> = volatile.iter().filter_map(|v| v.split('.').next()).collect();
    if !controls.is_empty() && controls == volatile_controls {
        return Err(ExtractionError::new(
            METADATA_LAYOUT_UNSUPPORTED,
            format!("the platform gate is too noisy: every control's baseline rows are volatile ({volatile:?})"),
        ));
    }
    Ok(Baseline { rows: runs.pop().unwrap(), volatile, runtime: None })
}

/// Leaf slots of a control's `info` node holding a bare `0`/`1` (flag candidates), as
/// `{"slot": "info[2][0][3]", "value": "1"}` entries, for experiment matrix generation.
pub fn flag_candidates(source: &SpannedMetadata, control: &str) -> Result<Vec<Value>> {
    let index = ControlIndex::build(source)?;
    let entry = find_control(&index, control)?;
    let info = entry
        .info()
        .ok_or_else(|| ExtractionError::new(METADATA_LAYOUT_UNSUPPORTED, format!("control '{control}' has no info node")))?;
    let mut out = Vec::new();
    fn walk(n: &crate::brace::Node, path: &str, out: &mut Vec<Value>) {
        for (i, c) in n.children.iter().enumerate() {
            let p = format!("{path}[{i}]");
            match c {
                Item::Node(x) => walk(x, &p, out),
                Item::Atom(a) if a.kind == AtomKind::Bare && matches!(a.value.as_str(), "0" | "1") => {
                    out.push(serde_json::json!({"slot": p, "value": a.value}));
                }
                _ => {}
            }
        }
    }
    walk(info.node, "info", &mut out);
    Ok(out)
}

fn execute_candidates(args: &[String]) -> Result<String> {
    let (mut stream, mut control) = (None, None);
    let mut i = 0;
    while i < args.len() {
        let Some(value) = args.get(i + 1) else {
            return Err(ExtractionError::new(CONFIG_INVALID, format!("argument {}: expected one argument", args[i])));
        };
        match args[i].as_str() {
            "--stream" => stream = Some(value.clone()),
            "--control" => control = Some(value.clone()),
            other => return Err(ExtractionError::new(CONFIG_INVALID, format!("unrecognized argument {other}"))),
        }
        i += 2;
    }
    let (Some(stream), Some(control)) = (stream, control) else {
        return Err(ExtractionError::new(CONFIG_INVALID, "usage: oneform explore candidates --stream <FormStream.bin> --control <name>"));
    };
    let data = fs::read(&stream).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {stream}: {e}")))?;
    let source = parse_spanned(&data)?;
    let candidates = flag_candidates(&source, &control)?;
    Ok(serde_json::to_string_pretty(&candidates).unwrap() + "\n")
}

fn execute_run(args: &[String]) -> Result<String> {
    let mut matrix: Option<String> = None;
    let mut epf: Option<String> = None;
    let mut base: Option<String> = None;
    let mut platform: Option<String> = None;
    let mut corpus: Option<String> = None;
    let mut runtime = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--runtime" {
            runtime = true;
            i += 1;
            continue;
        }
        let Some(value) = args.get(i + 1) else {
            return Err(ExtractionError::new(CONFIG_INVALID, format!("argument {a}: expected one argument")));
        };
        match a.as_str() {
            "--matrix" => matrix = Some(value.clone()),
            "--epf" => epf = Some(value.clone()),
            "--base" => base = Some(value.clone()),
            "--platform" => platform = Some(value.clone()),
            "--corpus" => corpus = Some(value.clone()),
            other => return Err(ExtractionError::new(CONFIG_INVALID, format!("unrecognized argument {other}"))),
        }
        i += 2;
    }
    let (Some(matrix), Some(epf), Some(base), Some(platform)) = (matrix, epf, base, platform) else {
        return Err(ExtractionError::new(
            CONFIG_INVALID,
            "usage: oneform explore run --matrix <spec.json|dir> --epf <donor.epf> --base <infobase> --platform <1cv8-bin> [--corpus <dir>] [--runtime]",
        ));
    };
    let corpus = Path::new(&corpus.unwrap_or_else(|| "tests/explore".to_string())).to_path_buf();
    let epf_bytes = fs::read(&epf).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {epf}: {e}")))?;
    let original = epf_form_stream(&epf_bytes)?;
    let mut specs: Vec<ExperimentSpec> = Vec::new();
    let matrix_path = Path::new(&matrix);
    if matrix_path.is_dir() {
        let mut files: Vec<_> = fs::read_dir(matrix_path)
            .map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {matrix}: {e}")))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        for f in files {
            specs.push(ExperimentSpec::parse_json(&fs::read(&f)?).map_err(|e| e.with("spec", f.to_string_lossy().into_owned()))?);
        }
    } else {
        specs.push(ExperimentSpec::parse_json(&fs::read(matrix_path).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot read {matrix}: {e}")))?)?);
    }
    if specs.is_empty() {
        return Err(ExtractionError::new(CONFIG_INVALID, "the matrix contains no experiments"));
    }
    let gate = PlatformGate { platform: platform.clone(), base: Path::new(&base).to_path_buf() };
    let runtime_gate = runtime.then(|| RuntimeGate { platform, base: Path::new(&base).to_path_buf() });
    let mut baseline = measure_baseline(&gate, &epf_bytes, &original)?;
    let base_dir = corpus.join("baseline");
    fs::create_dir_all(&base_dir).map_err(|e| ExtractionError::new(IO_ERROR, format!("cannot create {}: {e}", base_dir.display())))?;
    fs::write(base_dir.join("baseline-diff.json"), serde_json::to_string_pretty(&diff_to_json(&baseline.rows)).unwrap())?;
    if let Some(rg) = &runtime_gate {
        let rt = rg.run(&epf_bytes, &original)?;
        fs::write(
            base_dir.join("baseline-runtime.json"),
            serde_json::to_string_pretty(&rt.snapshot).unwrap(),
        )?;
        baseline.runtime = Some(rt.snapshot);
    }
    fs::write(
        base_dir.join("manifest.json"),
        serde_json::to_string_pretty(&serde_json::json!({"schema": 1, "id": "baseline", "volatile": baseline.volatile})).unwrap(),
    )?;
    let mut lines = vec![format!(
        "baseline: {} normalization diff row(s), {} volatile subtree(s)",
        baseline.rows.len(),
        baseline.volatile.len()
    )];
    for spec in &specs {
        let outcome = run_experiment_with_runtime(Some(&gate), runtime_gate.as_ref(), &epf_bytes, &original, spec, &baseline, &corpus, "8.3")?;
        lines.push(format!("{}: {}", spec.id, outcome.as_str()));
    }
    Ok(lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, control: &str, slot: &str, kind: &str, value: &str) -> ExperimentSpec {
        ExperimentSpec::parse_json(
            format!(r#"{{"id":"{id}","control":"{control}","slot":"{slot}","newValue":{{"kind":"{kind}","value":"{value}"}},"hypothesis":"h"}}"#).as_bytes(),
        )
        .unwrap()
    }

    fn donor() -> SpannedMetadata {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms/DonorForm/FormStream.bin");
        parse_spanned(&fs::read(p).unwrap()).unwrap()
    }

    #[test]
    fn spec_parse_and_validation() {
        let s = spec("e1", "Надпись1", "geom[5]", "bare", "0");
        assert_eq!(s.slot, SlotPath { section: SlotSection::Geom, indices: vec![5] });
        assert_eq!(s.new_value, SlotValue::Bare("0".into()));
        assert_eq!(s.hypothesis, "h");
        let no_hyp = ExperimentSpec::parse_json(br#"{"id":"e","control":"c","slot":"info[0]","newValue":{"kind":"quoted","value":"x"}}"#).unwrap();
        assert_eq!(no_hyp.hypothesis, "");
        for bad in [
            &br#"{"id":"e","control":"c","slot":"info[0]"}"#[..],                    // missing newValue
            br#"{"id":"","control":"c","slot":"info[0]","newValue":{"kind":"bare","value":"1"}}"#, // empty id
            br#"{"id":"e","control":"c","slot":"info","newValue":{"kind":"bare","value":"1"}}"#,   // no indices
            br#"{"id":"e","control":"c","slot":"app[0]","newValue":{"kind":"bare","value":"1"}}"#, // bad section
            br#"{"id":"e","control":"c","slot":"info[x]","newValue":{"kind":"bare","value":"1"}}"#,// bad index
            br#"{"id":"e","control":"c","slot":"info[0]","newValue":{"kind":"enum","value":"1"}}"#,// bad kind
            br#"{"id":"e","control":"c","slot":"info[0]","newValue":{"kind":"bare"}}"#,            // missing value
            b"[1,2]",                                                               // not an object
            b"{invalid",                                                            // not JSON
        ] {
            assert!(ExperimentSpec::parse_json(bad).is_err(), "{:?}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn slot_value_rendering() {
        assert_eq!(SlotValue::Bare("123".into()).render().unwrap(), b"123");
        assert_eq!(SlotValue::Bare("-1".into()).render().unwrap(), b"-1");
        assert_eq!(SlotValue::Quoted("a\"b".into()).render().unwrap(), b"\"a\"\"b\"");
        for bad in ["", "1 2", "a,b", "{", "x\"y", "1\n2"] {
            assert!(SlotValue::Bare(bad.into()).render().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn mutation_replaces_exactly_one_leaf() {
        let source = donor();
        let s = spec("e1", "Надпись1", "geom[5]", "bare", "0");
        let (patched, old) = mutate_stream(&source, &s).unwrap();
        assert_eq!(old, "1");
        let (a, b) = (source.source(), patched.as_slice());
        assert_eq!(a.len(), b.len());
        let diffs: Vec<usize> = a.iter().zip(b).enumerate().filter(|(_, (x, y))| x != y).map(|(i, _)| i).collect();
        assert_eq!(diffs.len(), 1);
        assert_eq!(b[diffs[0]], b'0');
        // the patched stream parses and the control now decodes as invisible
        let reparsed = parse_spanned(&patched).unwrap();
        let index = ControlIndex::build(&reparsed).unwrap();
        let e = find_control(&index, "Надпись1").unwrap();
        let g = e.geom().unwrap();
        let (atom, _) = g.atom(5).unwrap();
        assert_eq!(atom.value, "0");
    }

    #[test]
    fn mutation_rejections() {
        let source = donor();
        // unknown control
        assert_eq!(mutate_stream(&source, &spec("e", "НетТакого", "geom[5]", "bare", "0")).unwrap_err().code, PAYLOAD_MISSING);
        // path runs past the end / into an atom
        assert!(mutate_stream(&source, &spec("e", "Надпись1", "geom[5][0]", "bare", "0")).is_err());
        assert!(mutate_stream(&source, &spec("e", "Надпись1", "geom[99]", "bare", "0")).is_err());
        // node target (info[1] is the label block node of the donor's LabelDecoration)
        let err = mutate_stream(&source, &spec("e", "Надпись1", "info[1]", "bare", "0")).unwrap_err();
        assert_eq!(err.code, METADATA_LAYOUT_UNSUPPORTED);
        assert!(err.message.contains("node"), "{err}");
        // nothing was written: source untouched
        let _ = source;
    }

    // ------------------------------------------------ 2.x conveyor tests

    use super::{control_survives, epf_form_stream, net_diff, repack_epf_form};
    use crate::container::{inflate, parse_container, Limits};

    fn donor_epf() -> Vec<u8> {
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/donor.epf")).unwrap()
    }

    #[test]
    fn epf_form_stream_roundtrip_and_repack_preserves_other_entries() {
        let epf = donor_epf();
        let form = epf_form_stream(&epf).unwrap();
        let expected = fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/forms/DonorForm/FormStream.bin")).unwrap();
        assert_eq!(form, expected);
        // repack with the same stream: every entry's inflated content and header is preserved
        let repacked = repack_epf_form(&epf, &form).unwrap();
        let l = Limits::default();
        let (a, b) = (parse_container(&epf, &l, 0, None).unwrap(), parse_container(&repacked, &l, 0, None).unwrap());
        assert_eq!(a.names(), b.names());
        for (ea, eb) in a.entries.iter().zip(&b.entries) {
            assert_eq!(ea.name, eb.name);
            assert_eq!(ea.header, eb.header, "{}", ea.name);
            if let (Ok(ia), Ok(ib)) = (inflate(&ea.raw, &l, None), inflate(&eb.raw, &l, None)) {
                if let (Ok(sa), Ok(sb)) = (parse_container(&ia, &l, 1, None), parse_container(&ib, &l, 1, None)) {
                    // the form-bearing sub-container is re-serialized: compare it entry by entry
                    assert_eq!(sa.names(), sb.names(), "{}", ea.name);
                    for (fa, fb) in sa.entries.iter().zip(&sb.entries) {
                        assert_eq!(fa.name, fb.name);
                        assert_eq!(fa.header, fb.header, "{}.{}", ea.name, fa.name);
                        assert_eq!(fa.raw, fb.raw, "{}.{}", ea.name, fa.name);
                    }
                } else {
                    assert_eq!(ia, ib, "{}", ea.name);
                }
            } else {
                assert_eq!(ea.raw, eb.raw, "{}", ea.name);
            }
        }
        // repack with a mutation: only the form entry differs
        let source = donor();
        let (mutated, _) = mutate_stream(&source, &spec("e1", "Надпись1", "geom[5]", "bare", "0")).unwrap();
        let repacked = repack_epf_form(&epf, &mutated).unwrap();
        assert_eq!(epf_form_stream(&repacked).unwrap(), mutated);
    }

    #[test]
    fn net_diff_subtracts_baseline_key_by_key() {
        let row = |p: &str, a: &str, b: &str| (p.to_string(), a.to_string(), b.to_string());
        let baseline = vec![row("c.geom[1]", "0", "1"), row("c.geom[2]", "0", "1")];
        let observed = vec![row("c.geom[1]", "0", "1"), row("c.geom[5]", "1", "0")];
        assert_eq!(net_diff(&observed, &baseline, &[]), vec![row("c.geom[5]", "1", "0")]);
        assert!(net_diff(&baseline, &baseline, &[]).is_empty());
        // UUID-masked subtraction and volatile prefixes
        let obs2 = vec![row("c.info[1][7][1]", "e5800bff-f676-47f9-b863-055300f8535f", "2d9710ff-253e-4dca-b0d8-205da7d05e01")];
        let base2 = vec![row("c.info[1][7][1]", "084222af-f30d-4f1a-8e24-5cd08b233476", "620a611c-5e97-472f-84b7-0afd8b030f93")];
        assert!(net_diff(&obs2, &base2, &[]).is_empty(), "uuid-only change is masked");
        let obs3 = vec![row("bar.info[1][7][1]", "1", "2"), row("c.geom[5]", "1", "0")];
        let vol = vec!["bar.info[1]".to_string()];
        assert_eq!(net_diff(&obs3, &[], &vol), vec![row("c.geom[5]", "1", "0")]);
    }

    #[test]
    fn control_survival_detects_silent_drop() {
        let source = donor();
        let (mutated, _) = mutate_stream(&source, &spec("e1", "Надпись1", "geom[5]", "bare", "0")).unwrap();
        // unchanged normalized stream: survives
        assert!(control_survives(&mutated, &mutated, "Надпись1").unwrap());
        // a stream whose control is truncated: silent drop.
        // Build one by replacing the control record with a minimal {guid,id} stub at its span.
        let index = ControlIndex::build(&source).unwrap();
        let entry = index.entries.iter().find(|e| e.name == "Надпись1").unwrap();
        let span = entry.control.span.span;
        let guid = entry.guid.clone();
        let id = entry.id;
        let stub = format!("{{{guid},{id},{{0}},{{0}},{{0,0,0,0,0,0}},{{0,\"Надпись1\"}},{{0}}}}");
        let truncated = source
            .apply(&[crate::brace::Replacement { range: span, bytes: stub.into_bytes() }])
            .unwrap();
        // The stub parses as a control but its record is far shorter than the original.
        let info_len = |s: &[u8]| -> usize {
        let src = parse_spanned(s).unwrap();
        let idx = ControlIndex::build(&src).unwrap();
        let e = idx.entries.iter().find(|e| e.name == "Надпись1").unwrap();
        e.info().unwrap().node.len()
    };
    assert!(!control_survives(&mutated, &truncated, "Надпись1").unwrap());
    assert!(info_len(&mutated) > info_len(&truncated));
        // a control absent from the mutated stream is a caller error, not a silent drop
        assert!(control_survives(&mutated, &mutated, "НетТакого").is_err());
    }

    #[test]
    fn runtime_probe_repack_and_snapshot_diff() {
        let epf = donor_epf();
        let form = epf_form_stream(&epf).unwrap();
        let out_path = Path::new(r"C:\temp\probe_out.txt");
        let probed = repack_epf_form_with_probe(&epf, &form, out_path).unwrap();
        assert_eq!(epf_form_stream(&probed).unwrap(), form);
        // Verify the inner module entry now contains our generated ПриОткрытии probe and UTF-8 BOM.
        let limits = Limits::default();
        let outer = parse_container(&probed, &limits, 0, None).unwrap();
        let mut found_mod = None;
        for e in &outer.entries {
            if let Ok(inner_raw) = inflate(&e.raw, &limits, None) {
                if let Ok(sub) = parse_container(&inner_raw, &limits, 1, None) {
                    if let (Some(_), Some(m)) = (sub.get("form"), sub.get("module")) {
                        found_mod = Some(m.raw.clone());
                    }
                }
            }
        }
        let mod_raw = found_mod.expect("instrumented module entry present");
        assert!(mod_raw.starts_with(&[0xEF, 0xBB, 0xBF]));
        let mod_str = String::from_utf8_lossy(&mod_raw[3..]);
        assert!(mod_str.contains(r"C:\temp\probe_out.txt"), "{mod_str}");
        assert!(mod_str.contains("КодировкаТекста.UTF8"), "{mod_str}");
        assert!(mod_str.contains("Попытка Т.ДобавитьСтроку(Э.Имя + \".Доступность=\""), "{mod_str}");

        let snap_a = parse_runtime_snapshot("\u{feff}ПолеВвода1.Доступность=Да\r\nПолеВвода1.ТолькоПросмотр=Нет\r\nКнопка1.Доступность=Да\r\n".as_bytes());
        let snap_b = parse_runtime_snapshot("ПолеВвода1.Доступность=Нет\r\nПолеВвода1.ТолькоПросмотр=Нет\r\nКнопка1.Доступность=Нет\r\n".as_bytes());
        assert_eq!(
            diff_runtime(&snap_a, &snap_b, Some("ПолеВвода1")),
            vec![("ПолеВвода1.Доступность".to_string(), "Да".to_string(), "Нет".to_string())]
        );
        assert_eq!(diff_runtime(&snap_a, &snap_b, None).len(), 2);
    }
}
