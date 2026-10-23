#!/usr/bin/env python3
"""Actual original hash method, bounded owned files; deterministic read-boundary mutation."""
from pathlib import Path
import base64
import builtins
import hashlib
import json
import os
import sys
import tempfile
from types import SimpleNamespace

checkout, work = [Path(p).resolve() for p in sys.argv[1:]]
work.mkdir(exist_ok=False)
sys.path.insert(0, str(checkout))
source = checkout / "copyparty/up2k.py"
b = source.read_bytes()
tree = json.loads((Path(__file__).parent / "evidence/9001--copyparty/tree.json").read_text())
expected = next(x["sha"] for x in tree["tree"] if x["path"] == "copyparty/up2k.py")
assert hashlib.sha1(b"blob " + str(len(b)).encode() + b"\0" + b).hexdigest() == expected
from copyparty import up2k

obj = up2k.Up2k.__new__(up2k.Up2k)
obj.args = SimpleNamespace(iobuf=256 * 1024)
obj.mth = obj.pp = None
obj.stop = False
digest = lambda data: base64.urlsafe_b64encode(hashlib.sha512(data).digest()[:33]).decode("ascii")
with tempfile.TemporaryDirectory(prefix="hash-owned-", dir=work) as td:
    path = Path(td) / "input.bin"
    path.write_bytes(b"one")
    original, st = obj._hashlist_from_file(str(path))
    assert original == [digest(b"one")] and st.st_size == 3

    # Hook only the actual stream boundary, appending after a real read has returned.
    # Method body, hashing, stat and file data access remain original/native.
    class GrowingStream:
        def __init__(self, stream):
            self.stream = stream
            self.grown = False
        def __enter__(self):
            self.stream.__enter__()
            return self
        def __exit__(self, *args):
            return self.stream.__exit__(*args)
        def read(self, size):
            data = self.stream.read(size)
            if data and not self.grown:
                with builtins.open(path, "ab") as out:
                    out.write(b"-appended")
                self.grown = True
            return data
        def tell(self):
            return self.stream.tell()
    had_open = hasattr(up2k, "open")
    previous_open = getattr(up2k, "open", None)
    up2k.open = lambda *a, **k: GrowingStream(builtins.open(*a, **k))
    try:
        after, observed = obj._hashlist_from_file(str(path))
    finally:
        if had_open:
            up2k.open = previous_open
        else:
            del up2k.open
    assert after == original and observed.st_size == 3 and path.stat().st_size == 12
    assert after != [digest(path.read_bytes())]
    obj.stop = True
    stopped, stopped_stat = obj._hashlist_from_file(str(path))
    assert stopped == [] and stopped_stat.st_size == 12
    bounds = []
    for size in [0, 1, 1024**2, 256*1024**2, 256*1024**2+1, 1024**3, 1024**4, 2**63-1]:
        chunk = up2k.up2k_chunksize(size)
        count = (size + chunk - 1) // chunk
        assert chunk >= 1024**2
        assert count <= 256 or chunk >= 32*1024**2 and count <= 4096
        bounds.append({"bytes": size, "chunk_size": chunk, "chunks": count})
result = {"source_sha256": hashlib.sha256(b).hexdigest(), "controls": 11, "passed": 11,
          "baseline_hash": original, "growth_returns_original_prefix_digest": after == original,
          "growth_returned_size": observed.st_size, "growth_actual_size": 12,
          "stopped_hashes": stopped, "stopped_stat_size": stopped_stat.st_size,
          "chunk_bounds": bounds,
          "scope": "Original Up2k._hashlist_from_file body on actual owned files with constructor-free single-thread config; read wrapper deterministically appends after first native read. Original pure chunk-size function. Not high-level dedup acceptance, multithread hasher, upload, all races or server qualification."}
(work / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
