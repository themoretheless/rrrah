#!/usr/bin/env python3
"""Original virtual-HTTP dedup pipeline on owned files, with explicit rehash policy."""
from pathlib import Path
import builtins
import hashlib
import json
import os
import sys
import tempfile

checkout, work = [Path(p).resolve() for p in sys.argv[1:]]
work.mkdir(exist_ok=False)
(work / "config").mkdir()
os.environ["XDG_CONFIG_HOME"] = str(work / "config")
sys.path.insert(0, str(checkout))
from tests import util as tu
tu.get_ramdisk = lambda: tempfile.mkdtemp(prefix="owned-", dir=work)
from tests import test_dedup as original
from copyparty import up2k

rows = []
for mode in ["same_size_restored_mtime", "growth_during_rehash"]:
    tc = original.TestDedup("test_a")
    tc.setUp()
    tc.conn = None
    tc.fstab = None
    tc.ctr = 1
    had_open = hasattr(up2k, "open")
    old_open = getattr(up2k, "open", None)
    try:
        tc.args = tu.Cfg(v=[".::A"], a=[], e2d=True, safe_dedup=50)
        root = Path(tc.reset())
        tc.cinit()
        fi = tc.files[0]
        tc.do_post("d", "f1", fi, True)
        source = root / "d/f1"
        before = source.stat()
        trigger = [0]
        if mode == "same_size_restored_mtime":
            source.write_bytes(b"two")
            os.utime(source, ns=(before.st_atime_ns, before.st_mtime_ns))
            assert source.stat().st_size == 3
            assert source.stat().st_mtime_ns == before.st_mtime_ns
        else:
            class GrowingStream:
                def __init__(self, stream): self.stream = stream
                def __enter__(self): self.stream.__enter__(); return self
                def __exit__(self, *a): return self.stream.__exit__(*a)
                def read(self, size):
                    data = self.stream.read(size)
                    if data and not trigger[0]:
                        with builtins.open(source, "ab") as output: output.write(b"-appended")
                        trigger[0] += 1
                    return data
                def tell(self): return self.stream.tell()
            def intercepted_open(path, *args, **kwargs):
                stream = builtins.open(path, *args, **kwargs)
                if os.fsdecode(path) == str(source) and args and args[0] == "rb":
                    return GrowingStream(stream)
                return stream
            up2k.open = intercepted_open
        header, body = tc.handshake("d", "f2", fi)
        handshake = json.loads(body)
        # Restore interception before any subsequent requests/cleanup.
        if had_open: up2k.open = old_open
        elif hasattr(up2k, "open"): del up2k.open
        destination = root / "d" / handshake["name"]
        actual = destination.read_bytes() if destination.exists() else None
        if mode == "same_size_restored_mtime":
            assert handshake["hash"] == [fi[1]]
            tc.do_post_data("d", "f2", fi, False, handshake["name"], handshake)
            assert source.read_bytes() == b"two" and destination.read_bytes() == b"one"
        else:
            assert trigger[0] == 1
        rows.append({"mode": mode, "effective_safe_dedup": tc.asrv.vfs.all_vols[""].flags.get("safededup"),
                     "http_status": header.splitlines()[0], "handshake": handshake,
                     "appends_during_read": trigger[0], "source_after": source.read_bytes().decode(),
                     "destination_at_handshake": actual.decode() if actual is not None else None,
                     "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                     "published_without_chunks": not handshake["hash"],
                     "published_bytes_match_requested": actual == b"one" if actual is not None else None})
    finally:
        if had_open: up2k.open = old_open
        elif hasattr(up2k, "open"): del up2k.open
        tc.tearDown()
result = {"rows": rows,
          "scope": "Actual original up2k/SQLite/filesystem through original virtual-HTTP fixture; explicit safededup50, owned tempfile allocator. One deterministic append at native read boundary. No real network server, multi-thread hasher, user media or universal race proof."}
(work / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
