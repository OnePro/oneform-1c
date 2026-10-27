#!/usr/bin/env python3
"""Library: read/write the 1C container (EPF/ERF, nested form containers) byte-preserving.

Used by build_launcher.py (swaps the module of dump-base.epf). Not a CLI.
"""
import struct
import zlib

END = 0x7FFFFFFF


def _read_doc(data, off):
    out = bytearray(); cur = off; total = None
    while True:
        hdr = data[cur:cur+31].decode("ascii", errors="replace").strip().split()
        doc_size, blk_size, nxt = int(hdr[0], 16), int(hdr[1], 16), int(hdr[2], 16)
        if total is None:
            total = doc_size
        take = min(blk_size, total - len(out))
        out += data[cur+31:cur+31+take]
        if nxt == END or len(out) >= total:
            break
        cur = nxt
    return bytes(out)


def read_container(data):
    """-> (header_fields, [(name, header20, raw_data)])"""
    magic = struct.unpack_from("<iiii", data, 0)
    toc = _read_doc(data, 16)
    entries = []
    for i in range(0, len(toc), 12):
        ho, do, mk = struct.unpack_from("<iii", toc, i)
        hb = _read_doc(data, ho)
        name = hb[20:].decode("utf-16-le", errors="replace").rstrip("\x00")
        entries.append((name, hb[:20], _read_doc(data, do)))
    return magic, entries


def _block(doc_size, block_size, payload):
    actual = max(block_size, len(payload))
    hdr = f"\r\n{doc_size:08x} {actual:08x} {END:08x} \r\n".encode("ascii")
    return hdr + payload + b"\x00" * (actual - len(payload))


def write_container(magic, entries, block=0x200):
    out = bytearray(struct.pack("<iiii", magic[0], magic[1], len(entries), magic[3]))
    toc_size = 12 * len(entries)
    toc_at = len(out)
    out += _block(toc_size, max(block, toc_size), b"\x00" * toc_size)
    toc = bytearray()
    for name, h20, data in entries:
        hdr = bytes(h20) + name.encode("utf-16-le") + b"\x00\x00\x00\x00"
        h_off = len(out)
        out += _block(len(hdr), len(hdr), hdr)
        d_off = len(out)
        out += _block(len(data), max(block, len(data)), data)
        toc += struct.pack("<iii", h_off, d_off, END)
    out[toc_at + 31: toc_at + 31 + toc_size] = toc
    return bytes(out)


def inflate(b):
    try:
        return zlib.decompress(b, -15)
    except Exception:
        return b


def deflate(b):
    c = zlib.compressobj(9, zlib.DEFLATED, -15)
    return c.compress(b) + c.flush()


def replace_form_streams(epf_bytes, streams_by_uuid):
    magic, entries = read_container(epf_bytes)
    out = []
    for name, h20, data in entries:
        base = name[:-2] if name.endswith(".0") else None
        if base in streams_by_uuid:
            inner_raw = inflate(data)
            imagic, ients = read_container(inner_raw)
            new_ients = []
            for n2, h2, d2 in ients:
                if n2 == "form":
                    d2 = streams_by_uuid[base]  # inner entries are stored RAW (only the outer .0 entry is deflated)
                new_ients.append((n2, h2, d2))
            data = deflate(write_container(imagic, new_ients, block=0x200))
        out.append((name, h20, data))
    return write_container(magic, out)
