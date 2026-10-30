#!/usr/bin/env python3
"""Builds the blank-form templates of `oneform new-form` out of real Configurator forms (read-only on the corpora).

    python scripts/mine_blank_forms.py [--oneform target/debug/oneform] [--corpus target/corpus]
        [--epf target/tp/РабочееМестоМенеджера.epf] [--out src/templates/new_form] [--work <dir>]

How the sources were chosen (the smallest real form of the kind that holds only the standard command panels; the
form class is told by the name of the main attribute and by the `FormExtension` record, both constant per kind):

  catalog-item  ut2 Catalogs/Производители/ФормаЭлемента   (СправочникОбъект, 7149 bytes; panels ДействияФормы + ОсновныеДействияФормы)
  catalog-list  ut2 Catalogs/Производители/ФормаСписка     (СправочникСписок, 5356 bytes; the list Table with the picture and name
                                                            columns + ДействияФормы; its RowSource is the same generic record in
                                                            330 list forms)
  document      ut2 Documents/ЗаписьВКассовыйЖурнал/ФормаДокумента (ДокументОбъект; the smallest with both standard panels,
                                                            all fields removed)
  external      the external data processor РабочееМестоМенеджера.epf (made by the platform; real ОбработкаОбъект form
                with the plain `FormExtension` c5251418 as in the forms of an external processor), form ФормаВыбораПоиска:
                the panel ОсновныеДействияФормы with the standard «Закрыть» only. Used for `processor`.

Cleaning goes through the oneform patcher itself (extract, edit Form.xml, pack): all controls except the kept panels
are removed together with their events, the form events are removed, the module is empty, the caption is neutral.
The form attributes other than the main one (the unused value table ТаблицаНоменклатуры of the source processor) go
through the patcher as well.

  report        the report form made by the Configurator, tests/fixtures/configurator-report.erf (`erf-dump` of it, passed as
                --report-dump): ФормаОтчета with ОтчетОбъект (id 3), the extension 343b4e00, `Результат`, `ДействияФормы`,
                `Отбор`; the user's ПолеВвода1 / Надпись1 / Кнопка1, the attribute ПолеВвода1 and the handler are removed.
                The caption is `Отчет  %ИмяОбъекта%`, `new-form` puts the object name there.

The main attribute of an ordinary form holds the TypeId of the owner's generated type (CatalogObject / CatalogList /
DocumentObject / ExternalDataProcessorObject ...) as `{"Pattern",{"#",<TypeId>}}`. The script checks that the source
stream carries exactly one such TypeId, replaces it by a placeholder (`new-form` puts the TypeId of the target object
there), audits every other GUID of the stream (it must occur in forms of >= 3 objects of the corpora, i.e. be a
platform constant) and writes `<kind>.form.bin`: the whole `Ext/Form.bin` container with an empty module.
"""
import argparse
import collections
import glob
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GUID_RE = r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}"
GUID = re.compile(GUID_RE)
# must equal the constants in src/new_form.rs
PLACEHOLDER = {
    "catalog-item": "7f5a1c00-0000-4000-8000-000000000001",
    "catalog-list": "7f5a1c00-0000-4000-8000-000000000002",
    "document": "7f5a1c00-0000-4000-8000-000000000003",
    "external": "7f5a1c00-0000-4000-8000-000000000004",
    "report": "7f5a1c00-0000-4000-8000-000000000005",
}
# the caption of the report template: `new-form` puts `Отчет  <object name>` there (two spaces, as the Configurator does)
REPORT_CAPTION = "Отчет  %ИмяОбъекта%"
REPORT_KEEP = ["Результат", "ДействияФормы", "Отбор"]
SPECS = {
    "catalog-item": dict(corpus="ut2", family="Catalogs", root="Catalog", obj="Производители", form="ФормаЭлемента",
                         keep=["ДействияФормы", "ОсновныеДействияФормы"], caption="Форма элемента", type_name="CatalogObject"),
    "catalog-list": dict(corpus="ut2", family="Catalogs", root="Catalog", obj="Производители", form="ФормаСписка",
                         keep=["СправочникСписок", "ДействияФормы"], caption="Форма списка", type_name="CatalogList"),
    "document": dict(corpus="ut2", family="Documents", root="Document", obj="ЗаписьВКассовыйЖурнал", form="ФормаДокумента",
                     keep=["ДействияФормы", "ОсновныеДействияФормы"], caption="Форма документа", type_name="DocumentObject"),
}


class Tool:
    def __init__(self, exe):
        self.exe = str(exe)

    def run(self, *args):
        r = subprocess.run([self.exe, *map(str, args), "--no-progress"], capture_output=True, text=True, encoding="utf-8")
        if r.returncode != 0:
            sys.exit(f"oneform {' '.join(map(str, args))} failed:\n{r.stdout}{r.stderr}")
        return r.stdout + r.stderr


def blocks(lines, indent):
    """Top-level elements at the given indentation: (first line, last line, tag, name)."""
    pad = " " * indent
    out, i = [], 0
    while i < len(lines):
        m = re.match(pad + r'<(\w+)( name="([^"]*)")?[^>]*?(/?)>$', lines[i])
        if m:
            tag, name, selfclosed = m.group(1), m.group(3), m.group(4)
            j = i if selfclosed else next(k for k in range(i + 1, len(lines)) if lines[k] == pad + f"</{tag}>")
            out.append((i, j, tag, name))
            i = j + 1
        else:
            i += 1
    return out


def drop_button(text, name):
    """Remove the button `name` of a command panel (one line if self-closing, else up to its </Button>)."""
    lines = text.split("\n")
    start = next(i for i, line in enumerate(lines) if line.startswith(f'        <Button name="{name}"'))
    end = start if lines[start].endswith("/>") else next(i for i in range(start, len(lines)) if lines[i] == "        </Button>")
    return "\n".join(lines[:start] + lines[end + 1:])


def clean_form_xml(text, keep, caption, drop_buttons=()):
    """Keep only the named top-level controls, drop every <Events>, drop buttons, set a neutral caption."""
    lines = text.split("\n")
    first, last = lines.index("  <ChildItems>"), lines.index("  </ChildItems>")
    body = lines[first + 1:last]
    kill = set()
    for a, b, _tag, name in blocks(body, 4):
        if name not in keep:
            kill.update(range(a, b + 1))
    body = [line for i, line in enumerate(body) if i not in kill]
    text = "\n".join(lines[:first + 1] + body + lines[last:])
    for name in drop_buttons:
        text = drop_button(text, name)
    text = re.sub(r"\n  <Events>.*?\n  </Events>", "", text, flags=re.S)
    return re.sub(r'(<Caption>\s*<Item language="ru">)[^<]*(</Item>)', rf"\g<1>{caption}\g<2>", text, count=1)


def drop_other_attributes(text):
    """Remove the form attributes other than the main one (nothing is bound to them after `clean_form_xml`)."""
    main = re.search(r'<MainAttribute attributeId="(\d+)"', text).group(1)
    lines = text.split("\n")
    first, last = lines.index("  <Attributes>"), lines.index("  </Attributes>")
    kill = set()
    for a, b, tag, _name in blocks(lines[first + 1:last], 4):
        if tag == "Attribute" and f'id="{main}"' not in lines[first + 1 + a]:
            kill.update(range(first + 1 + a, first + 2 + b))
    return "\n".join(line for i, line in enumerate(lines) if i not in kill)


def object_xml(root, name):
    return ('<?xml version="1.0" encoding="UTF-8"?>\n<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" '
            f'xmlns:v8="http://v8.1c.ru/8.1/data/core"><{root} uuid="a63b4d77-d77e-5295-913d-cba0fe9ee943"><Properties>'
            f"<Name>{name}</Name></Properties></{root}></MetaDataObject>")


def round_trip(tool, ws):
    tool.run("extract", ws)
    out = tool.run("pack", ws, "--dry-run")
    if "NO CHANGES" not in out:
        sys.exit(f"round trip is not stable in {ws}:\n{out}")


def stage_config_form(work, name, src, obj_name, root, form, source_form=None):
    """A synthetic one-form workspace `<work>/<name>/<Family>/<obj>`; returns (workspace, object dir)."""
    ws = work / name
    od = ws / src.parent.name / obj_name
    (od / "Forms" / form / "Ext").mkdir(parents=True)
    source_form = source_form or form
    shutil.copy(src / "Forms" / f"{source_form}.xml", od / "Forms" / f"{form}.xml")
    shutil.copy(src / "Forms" / source_form / "Ext" / "Form.bin", od / "Forms" / form / "Ext" / "Form.bin")
    (ws / src.parent.name / f"{obj_name}.xml").write_text(object_xml(root, obj_name), encoding="utf-8")
    return ws, od


def mine_config_form(tool, corpus, work, kind, spec):
    src = corpus / spec["corpus"] / spec["family"] / spec["obj"]
    ws, od = stage_config_form(work, kind, src, spec["obj"], spec["root"], spec["form"])
    tool.run("extract", ws)
    form_dir = od / "src" / "Forms" / spec["form"]
    original = (form_dir / "Form.xml").read_text("utf-8")
    handlers = set(re.findall(r">([^<>]+)</", "".join(re.findall(r"<Events>.*?</Events>", original, flags=re.S))))
    (form_dir / "Form.xml").write_text(clean_form_xml(original, spec["keep"], spec["caption"]), encoding="utf-8", newline="")
    (form_dir / "FormModule.bsl").write_text("", encoding="utf-8")
    tool.run("pack", ws)
    round_trip(tool, ws)
    if "<Events>" in (form_dir / "Form.xml").read_text("utf-8"):
        sys.exit(f"{kind}: events are left")
    stream = (form_dir / "Reference" / "FormStream.bin").read_bytes()
    left = sorted(h for h in handlers if h.encode() in stream)
    if left:
        sys.exit(f"{kind}: handler names are left in the stream: {left}")
    obj_xml = (src.parent / f"{spec['obj']}.xml").read_text("utf-8-sig")
    type_id = re.search(rf'GeneratedType name="{spec["type_name"]}\.[^"]*" category="\w+">\s*<xr:TypeId>([^<]*)', obj_xml).group(1)
    return (od / "Forms" / spec["form"] / "Ext" / "Form.bin").read_bytes(), stream, type_id, set(GUID.findall(obj_xml))


def mine_external(tool, corpus, work, epf):
    """Clean ФормаВыбораПоиска of the external processor, then wrap its stream into an empty-module Form.bin."""
    ws = work / "epf"
    ws.mkdir()
    shutil.copy(epf, ws / "obj.epf")
    tool.run("extract", ws)
    form_dir = ws / "src" / "Forms" / "ФормаВыбораПоиска"
    xml = clean_form_xml((form_dir / "Form.xml").read_text("utf-8"), ["ОсновныеДействияФормы"], "Форма",
                         drop_buttons=["ОсновныеДействияФормыВыполнить", "Разделитель"])
    (form_dir / "Form.xml").write_text(drop_other_attributes(xml), encoding="utf-8", newline="")
    (form_dir / "FormModule.bsl").write_text("", encoding="utf-8")
    tool.run("pack", ws)
    stream = (form_dir / "Reference" / "FormStream.bin").read_bytes()
    # the container: a config form of the same layout, the stream put in, an empty module
    spec = SPECS["catalog-item"]
    src = corpus / spec["corpus"] / spec["family"] / spec["obj"]
    cws, od = stage_config_form(work, "wrap", src, "Х", "Catalog", spec["form"])
    tool.run("extract", cws)
    fd = od / "src" / "Forms" / spec["form"]
    (fd / "Reference" / "FormStream.bin").write_bytes(stream)
    (fd / "FormModule.bsl").write_text("", encoding="utf-8")
    tool.run("pack", cws)
    round_trip(tool, cws)
    ext_stream = (fd / "Reference" / "FormStream.bin").read_bytes()
    # the TypeId of ExternalDataProcessorObject of the source .epf = the main attribute type of its forms
    ext_type = re.search(r'"ОбработкаОбъект",\s*\{"Pattern",\s*\{"#",\s*(' + GUID_RE + ")", ext_stream.decode("utf-8-sig")).group(1)
    return (od / "Forms" / spec["form"] / "Ext" / "Form.bin").read_bytes(), ext_stream, ext_type, set()


def mine_report(tool, work, dump):
    """The report form of the Configurator (`erf-dump` of tests/fixtures/configurator-report.erf): the user's controls,
    the user's attribute and the handler are removed by the oneform patcher; the stream keeps `ОтчетОбъект`, the
    extension 343b4e00, `Результат`, `ДействияФормы` and `Отбор`. The container is the dump's own, repacked by `pack`."""
    ws = work / "report"
    shutil.copytree(dump, ws, ignore=shutil.ignore_patterns("src", ".oneform", ".extract-work"))
    od = next(p for p in ws.iterdir() if p.is_dir())
    form = next((od / "Forms").glob("*.xml")).stem
    tool.run("extract", ws)
    fd = od / "src" / "Forms" / form
    text = (fd / "Form.xml").read_text("utf-8")
    # attributes other than the main one (`ПолеВвода1` of the sample) go with their controls
    xml = drop_other_attributes(clean_form_xml(text, REPORT_KEEP, REPORT_CAPTION))
    (fd / "Form.xml").write_text(xml, encoding="utf-8", newline="")
    (fd / "FormModule.bsl").write_text("", encoding="utf-8")
    tool.run("pack", ws)
    round_trip(tool, ws)
    stream = (fd / "Reference" / "FormStream.bin").read_bytes()
    type_id = re.search(r'"ОтчетОбъект",\s*\{"Pattern",\s*\{"#",\s*(' + GUID_RE + ")", stream.decode("utf-8-sig")).group(1)
    object_xml_text = (ws / f"{od.name}.xml").read_text("utf-8-sig")
    return (od / "Forms" / form / "Ext" / "Form.bin").read_bytes(), stream, type_id, set(GUID.findall(object_xml_text))


def corpus_guid_counts(corpus, limit=1500):
    """GUID -> number of distinct objects (of the corpora) whose form streams contain it (a sample of the forms)."""
    seen = collections.defaultdict(set)
    paths = sorted(glob.glob(str(corpus / "*" / "*" / "*" / "src" / "Forms" / "*" / "Reference" / "FormStream.bin")))
    for p in paths[::max(1, len(paths) // limit)]:
        obj = str(Path(p).parents[3])
        for g in set(GUID.findall(Path(p).read_bytes().decode("utf-8-sig", "replace"))):
            seen[g].add(obj)
    return {g: len(v) for g, v in seen.items()}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--oneform", default=str(ROOT / "target" / "debug" / "oneform"))
    ap.add_argument("--corpus", default=str(ROOT / "target" / "corpus"))
    ap.add_argument("--epf", default=str(ROOT / "target" / "tp" / "РабочееМестоМенеджера.epf"))
    ap.add_argument("--report-dump", default=None,
                    help="erf-dump of tests/fixtures/configurator-report.erf (the report template is mined only with it)")
    ap.add_argument("--out", default=str(ROOT / "src" / "templates" / "new_form"))
    ap.add_argument("--work", default=None)
    args = ap.parse_args()
    tool, corpus, out = Tool(args.oneform), Path(args.corpus), Path(args.out)
    work = Path(args.work) if args.work else Path(tempfile.mkdtemp(prefix="blank-forms-"))
    shutil.rmtree(work, ignore_errors=True)
    work.mkdir(parents=True)
    out.mkdir(parents=True, exist_ok=True)
    counts = corpus_guid_counts(corpus)
    results = {kind: mine_config_form(tool, corpus, work, kind, spec) for kind, spec in SPECS.items()}
    results["external"] = mine_external(tool, corpus, work, Path(args.epf))
    if args.report_dump:
        results["report"] = mine_report(tool, work, Path(args.report_dump))
    for kind, (bin_, stream, type_id, owner_guids) in results.items():
        text = stream.decode("utf-8-sig")
        if text.count(type_id) < 1 or bin_.count(type_id.encode()) != text.count(type_id):
            sys.exit(f"{kind}: expected the owner TypeId {type_id} in the stream and the same times in Form.bin, found {text.count(type_id)} / {bin_.count(type_id.encode())}")
        guids = set(GUID.findall(text))
        # besides the TypeId a stream holds platform constants (seen in forms of many objects) and instance ids of the
        # form's own buttons/panels (random, in no object); a GUID of the owner's metadata would be a foreign reference
        foreign = sorted(g for g in guids if g != type_id and g in owner_guids)
        if foreign:
            sys.exit(f"{kind}: GUIDs of the source object are left in the stream: {foreign}")
        instance = sorted(g for g in guids if g != type_id and counts.get(g, 0) < 3)
        (out / f"{kind}.form.bin").write_bytes(bin_.replace(type_id.encode(), PLACEHOLDER[kind].encode()))
        print(f"{kind}: {len(bin_)} bytes, owner TypeId {type_id} -> {PLACEHOLDER[kind]}, {len(guids)} GUIDs, {len(instance)} form instance ids")
    if not args.work:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
