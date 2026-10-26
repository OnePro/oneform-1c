#!/usr/bin/env python3
"""Build target/std-codes/a1-launcher.epf = dump-cfg.epf with the module replaced by launcher_module.bsl"""
import os, sys
here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(here, "..")) 
from epf_pack import read_container, write_container, inflate, deflate
src = os.path.join(here, "..", "dump-ws", "dump-cfg.epf")
bsl = open(os.path.join(here, "launcher_module.bsl"), encoding="utf-8").read().replace("\r\n", "\n").replace("\n", "\r\n")
mod = b"\xef\xbb\xbf" + bsl.encode("utf-8")
magic, entries = read_container(open(src, "rb").read())
out = []
for name, h20, data in entries:
    if name.endswith(".0"):
        raw = inflate(data)
        if raw[:4] == bytes.fromhex("ffffff7f"):
            im, ients = read_container(raw)
            if any(n == "module" for n, _, _ in ients):
                ients = [(n, h, mod if n == "module" else d) for n, h, d in ients]
                data = deflate(write_container(im, ients, block=0x200))
    out.append((name, h20, data))
open(os.path.join(here, "a1-launcher.epf"), "wb").write(write_container(magic, out))
print("built a1-launcher.epf")
