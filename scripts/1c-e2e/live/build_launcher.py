#!/usr/bin/env python3
"""Build a launcher processor: dump-base.epf with its module replaced by a .bsl file.

  python build_launcher.py [module.bsl [out.epf]]
Defaults: launcher_module.bsl -> $ONEFORM_LIVE_WORK/a1-launcher.epf (one form per run, used by live.py).
  python build_launcher.py std_codes_module.bsl <work>/std-codes.epf   (batch mode, used by run_batch.py)
"""
import os
import sys

from epf_pack import deflate, inflate, read_container, write_container
from onec import HERE, WORK

bsl_path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "launcher_module.bsl")
out_path = sys.argv[2] if len(sys.argv) > 2 else os.path.join(WORK, "a1-launcher.epf")
bsl = open(bsl_path, encoding="utf-8-sig").read().replace("\r\n", "\n").replace("\n", "\r\n")
mod = b"\xef\xbb\xbf" + bsl.encode("utf-8")  # the module is stored with a BOM
magic, entries = read_container(open(os.path.join(HERE, "dump-base.epf"), "rb").read())
out = []
for name, h20, data in entries:
    if name.endswith(".0"):
        raw = inflate(data)
        if raw[:4] == bytes.fromhex("ffffff7f"):  # nested container of the processor
            im, ients = read_container(raw)
            if any(n == "module" for n, _, _ in ients):
                ients = [(n, h, mod if n == "module" else d) for n, h, d in ients]
                data = deflate(write_container(im, ients, block=0x200))
    out.append((name, h20, data))
open(out_path, "wb").write(write_container(magic, out))
print("built", out_path)
