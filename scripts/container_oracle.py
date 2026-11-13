#!/usr/bin/env python3
"""Independent reference reader of 1C containers (.cf, .cfe, .epf, .erf). Python stdlib only.

It has its own parser (no onec_dtools) and is the source of the expected values in
tests/fixtures/containers/expected.json. Everything is read into memory: it is a reference, not a tool.

Usage:
  python scripts/container_oracle.py entries FILE      JSON for one file
  python scripts/container_oracle.py diff A B          JSON: counts and lists, records paired by name, compared
                                                       by the SHA-256 of the stored bytes
  python scripts/container_oracle.py expected DIR      JSON of expected.json for the six fixtures in DIR
"""
import datetime
import hashlib
import json
import os
import re
import struct
import sys
import zlib

SIGNATURE = b"\xff\xff\xff\x7f"
FIXTURES = ["cf-32bit.cf", "cf-64bit-stub.cf", "cfe-32bit.cfe", "epf-803.epf", "epf-802.epf", "erf-803.erf"]
PAIR = ("cf-32bit.cf", "cf-64bit-stub.cf")


class ContainerError(Exception):
    pass


class Geometry:
    """Geometry of one layout: hex field width, file header, block header, toc record, end marker."""

    def __init__(self, bits):
        if bits == 32:
            self.hexw, self.fhdr, self.bhdr, self.rec, self.end = 8, 16, 31, 12, 0x7FFFFFFF
            self.word = "<I"
        else:
            self.hexw, self.fhdr, self.bhdr, self.rec, self.end = 16, 20, 55, 24, 2**64 - 1
            self.word = "<Q"
        self.wsize = struct.calcsize(self.word)


G32, G64 = Geometry(32), Geometry(64)


def block_header(data, pos, g):
    """(doc size, page size, next) of the block header at absolute `pos`, or None when it is malformed."""
    h = data[pos:pos + g.bhdr]
    w = g.hexw
    if len(h) != g.bhdr or h[:2] != b"\r\n" or h[-2:] != b"\r\n":
        return None
    if h[2 + w:3 + w] != b" " or h[3 + 2 * w:4 + 2 * w] != b" " or h[4 + 3 * w:5 + 3 * w] != b" ":
        return None
    try:
        vals = [int(h[2 + i * (w + 1):2 + i * (w + 1) + w], 16) for i in range(3)]
        for i in range(3):  # int() accepts signs, underscores and spaces: refuse them
            if not re.fullmatch(rb"[0-9a-fA-F]+", h[2 + i * (w + 1):2 + i * (w + 1) + w]):
                return None
    except ValueError:
        return None
    return tuple(vals)


def read_document(data, base, off, g, blocks):
    """Content of the document whose first block is at `base + off`; every block is appended to `blocks`."""
    seen = set()
    out = bytearray()
    left = None
    while True:
        pos = base + off
        if pos < base + g.fhdr or pos + g.bhdr > len(data):
            raise ContainerError("block offset out of range: %d" % pos)
        if pos in seen:
            raise ContainerError("block chain cycle at %d" % pos)
        seen.add(pos)
        bh = block_header(data, pos, g)
        if bh is None:
            raise ContainerError("bad block header at %d" % pos)
        doc, page, nxt = bh
        if left is None:
            left = doc
        take = min(page, left)
        end = pos + g.bhdr + page
        if end > len(data):
            raise ContainerError("truncated block at %d" % pos)
        blocks.append(end)
        out += data[pos + g.bhdr:pos + g.bhdr + take]
        left -= take
        if left == 0:
            return bytes(out)
        if take == 0 or nxt == g.end:
            raise ContainerError("document shorter than declared at %d" % pos)
        off = nxt


def parse(data, base, g):
    """-> (header entry count, {name: (created, modified, stored)}, end of the last block)."""
    if len(data) < base + g.fhdr:
        raise ContainerError("file shorter than the file header")
    count = struct.unpack_from("<I", data, base + g.fhdr - 8)[0]
    blocks = []
    toc = read_document(data, base, g.fhdr, g, blocks)
    if len(toc) % g.rec:
        raise ContainerError("toc size is not a multiple of %d" % g.rec)
    entries = {}
    for i in range(len(toc) // g.rec):
        fields = struct.unpack_from("<3" + g.word[1], toc, i * g.rec)
        if fields[2] != g.end:
            raise ContainerError("bad toc record %d" % i)
        header = read_document(data, base, fields[0], g, blocks)
        stored = read_document(data, base, fields[1], g, blocks)
        if len(header) < 22:
            raise ContainerError("record header too short")
        created, modified = struct.unpack_from("<QQ", header, 0)
        raw = header[20:]
        raw = raw[:len(raw) // 2 * 2]
        units = [raw[j:j + 2] for j in range(0, len(raw), 2)]
        if b"\0\0" not in units:
            raise ContainerError("name is not terminated")
        name = b"".join(units[:units.index(b"\0\0")]).decode("utf-16-le")
        if name in entries:
            raise ContainerError("duplicate name %r" % name)
        entries[name] = (created, modified, stored)
    return count, entries, max(blocks)


def well_formed_container(b):
    """Signature + well-formed first block header of a 32-bit container (CRLF, spaces, hex digits)."""
    return b[:4] == SIGNATURE and len(b) >= 16 + 31 and block_header(b, 16, G32) is not None


def ticks_text(ticks):
    try:
        d = datetime.datetime(1, 1, 1) + datetime.timedelta(microseconds=ticks * 100)
    except OverflowError:
        return None
    return "%04d-%02d-%02dT%02d:%02d:%02d.%03d" % (d.year, d.month, d.day, d.hour, d.minute, d.second, d.microsecond // 1000)


def inflate_exact(stored):
    """Inflated bytes when `stored` is one raw DEFLATE stream ending at its last byte, else None."""
    d = zlib.decompressobj(-15)
    try:
        out = d.decompress(stored)
    except zlib.error:
        return None
    if d.eof and not d.unused_data:
        return out
    return None


def classify(stored):
    inflated = inflate_exact(stored)
    if inflated is not None:
        return "deflate", inflated
    if len(stored) == 0 or well_formed_container(stored):
        return "none", None
    return "invalid", None


def sha(b):
    return hashlib.sha256(b).hexdigest()


def read_file(path):
    with open(path, "rb") as f:
        data = f.read()
    if len(data) < 16:
        raise ContainerError("file shorter than the file header")
    if data[:8] == b"\xff" * 8:
        layout, base, g = "64bit", 0, G64
        if block_header(data, 20, G64) is None:
            raise ContainerError("64-bit container without a valid first block header")
    else:
        _, _, end32 = parse(data, 0, G32)
        if end32 == len(data):
            layout, base, g = "32bit", 0, G32
        elif end32 < len(data):
            if data[end32:end32 + 8] != b"\xff" * 8 or block_header(data, end32 + 20, G64) is None:
                raise ContainerError("trailing data at %d" % end32)
            layout, base, g = "64bit-stub", end32, G64
        else:
            raise ContainerError("truncated: end of blocks %d beyond file size %d" % (end32, len(data)))
    count, entries, end = parse(data, base, g)
    if end != len(data):
        raise ContainerError("trailing data at %d" % end)
    return layout, base, count, entries


def entries_json(path):
    layout, base, count, entries = read_file(path)
    out = []
    compat = None
    for name in sorted(entries, key=lambda n: n.encode("utf-8")):
        created, modified, stored = entries[name]
        kind, inflated = classify(stored)
        probe = inflated if kind == "deflate" else stored if kind == "none" else b""
        e = {
            "name": name,
            "storedSize": len(stored),
            "storedSha256": sha(stored),
            "compression": kind,
            "inflatedSize": len(inflated) if kind == "deflate" else None,
            "inflatedSha256": sha(inflated) if kind == "deflate" else None,
            "nested": well_formed_container(probe),
            "created": created,
            "modified": modified,
            "createdText": ticks_text(created),
            "modifiedText": ticks_text(modified),
        }
        out.append(e)
        if name == "version" and kind == "deflate" and compat is None:
            m = re.search(rb"\{(8\d{4}),", inflated)
            compat = int(m.group(1)) if m else None
    return {
        "layout": layout,
        "containerOffset": base,
        "headerEntryCount": count,
        "entryCount": len(out),
        "compatibilityVersion": compat,
        "entries": out,
    }


def diff_json(a, b):
    ea = {e["name"]: e["storedSha256"] for e in entries_json(a)["entries"]}
    eb = {e["name"]: e["storedSha256"] for e in entries_json(b)["entries"]}
    key = lambda n: n.encode("utf-8")
    common = [n for n in ea if n in eb]
    differ = sorted((n for n in common if ea[n] != eb[n]), key=key)
    only_a = sorted((n for n in ea if n not in eb), key=key)
    only_b = sorted((n for n in eb if n not in ea), key=key)
    return {
        "counts": {
            "a": len(ea), "b": len(eb), "same": len(common) - len(differ),
            "differ": len(differ), "onlyA": len(only_a), "onlyB": len(only_b),
        },
        "differ": differ,
        "onlyA": only_a,
        "onlyB": only_b,
    }


def expected_json(d):
    out = {name: entries_json(os.path.join(d, name)) for name in FIXTURES}
    out["pair:%s|%s" % PAIR] = diff_json(os.path.join(d, PAIR[0]), os.path.join(d, PAIR[1]))
    return out


def dump(obj):
    return json.dumps(obj, sort_keys=True, indent=2, ensure_ascii=False)


def main(argv):
    try:
        if len(argv) == 3 and argv[1] == "entries":
            text = dump(entries_json(argv[2]))
        elif len(argv) == 4 and argv[1] == "diff":
            text = dump(diff_json(argv[2], argv[3]))
        elif len(argv) == 3 and argv[1] == "expected":
            text = dump(expected_json(argv[2]))
        else:
            sys.stderr.write(__doc__)
            return 2
    except (ContainerError, OSError) as e:
        sys.stderr.write("error: %s\n" % e)
        return 1
    sys.stdout.buffer.write((text + "\n").encode("utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
