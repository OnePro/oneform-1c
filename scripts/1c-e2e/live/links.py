#!/usr/bin/env python3
"""Print the data-path links (field `Связи`) of a form stream, to compare with `Form.xml` `steps=`.

  python links.py <Form.bin of the configuration object> [link id ...]
"""
import struct
import sys
import zlib

from brace import dump, parse


def read_v8(data):
    """Container document reader: -> {entry name: bytes}."""
    def read_doc(off):
        out = bytearray()
        cur = off
        total = None
        while True:
            hdr = data[cur:cur + 31].decode("ascii", errors="replace").strip().split()
            doc_size, blk_size, nxt = int(hdr[0], 16), int(hdr[1], 16), int(hdr[2], 16)
            if total is None:
                total = doc_size
            out += data[cur + 31:cur + 31 + min(blk_size, total - len(out))]
            if nxt == 0x7fffffff or len(out) >= total:
                break
            cur = nxt
        return bytes(out)
    toc = read_doc(16)
    res = {}
    for i in range(0, len(toc), 12):
        ho, do, _ = struct.unpack_from("<iii", toc, i)
        hb = read_doc(ho)
        res[hb[20:].decode("utf-16-le", errors="replace").rstrip("\x00")] = read_doc(do)
    return res


def stream(path):
    s = read_v8(open(path, "rb").read())["form"]
    try:
        s = zlib.decompress(s, -15)
    except zlib.error:
        pass
    return s.decode("utf-8-sig")


if __name__ == "__main__":
    root = parse(stream(sys.argv[1]))
    for link in root[2][3][1:]:
        if len(sys.argv) < 3 or int(link[0]) in map(int, sys.argv[2:]):
            print(link[0], dump(link[1]))
