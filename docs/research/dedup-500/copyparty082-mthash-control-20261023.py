#!/usr/bin/env python3
"""Actual original MTHash/Up2k file methods on bounded owned inputs."""
from pathlib import Path
import base64
import faulthandler
import hashlib
import json
import sys
import tempfile
from types import SimpleNamespace

checkout, work = [Path(p).resolve() for p in sys.argv[1:]]
work.mkdir(exist_ok=False)
sys.path.insert(0, str(checkout))
faulthandler.dump_traceback_later(30, exit=True)
from copyparty.util import MTHash
from copyparty.up2k import Up2k

tree = json.loads((Path(__file__).parent / "evidence/9001--copyparty/tree.json").read_text())
pins = {x["path"]: x["sha"] for x in tree["tree"] if x["type"] == "blob"}
source_pins = {}
for name in ["copyparty/util.py", "copyparty/up2k.py"]:
    b = (checkout / name).read_bytes()
    assert hashlib.sha1(b"blob " + str(len(b)).encode() + b"\0" + b).hexdigest() == pins[name]
    source_pins[name] = hashlib.sha256(b).hexdigest()
chunk = 1024**2
pattern = bytes(range(256)) * (chunk // 256)
digest = lambda data: base64.urlsafe_b64encode(hashlib.sha512(data).digest()[:33]).decode()
rows = []
with tempfile.TemporaryDirectory(prefix="owned-", dir=work) as td:
    path = Path(td) / "input.bin"
    for cores in [2, 4]:
        pool = MTHash(cores)
        assert len(pool.thrs) == cores and all(t.daemon for t in pool.thrs)
        obj = Up2k.__new__(Up2k)
        obj.args = SimpleNamespace(iobuf=256*1024)
        obj.mth, obj.pp, obj.stop = pool, None, False
        for size in [0, 1, chunk//2-1, chunk//2, chunk, chunk+3, 2*chunk+17]:
            data = (pattern * 3)[:size]
            path.write_bytes(data)
            actual, stat = obj._hashlist_from_file(str(path))
            expected = [digest(data[n:n+chunk]) for n in range(0, size, chunk)]
            assert actual == expected and stat.st_size == size
            assert pool.f is None
            rows.append({"workers": cores, "bytes": size, "chunks": len(actual), "sha256": hashlib.sha256(data).hexdigest()})

        # Declared length exceeds actual bytes: exercise worker error propagation/drain.
        path.write_bytes(b"x")
        with path.open("rb") as f:
            try:
                pool.hash(f, chunk+1, chunk)
            except Exception as error:
                assert "EOF at" in str(error)
                retained_after_error = pool.f is f and pool.sz == chunk+1
            else:
                raise AssertionError("expected EOF worker exception")
        assert pool.work_q.empty() and pool.done_q.empty()
        path.write_bytes(pattern)
        with path.open("rb") as f:
            recovered = pool.hash(f, chunk, chunk)
        assert recovered == [(digest(pattern), 0, chunk)] and pool.f is None

        pool.stop = obj.stop = True
        stopped, stat = obj._hashlist_from_file(str(path))
        assert stopped == [""] and stat.st_size == chunk
        pool.stop = obj.stop = False
        retry, _ = obj._hashlist_from_file(str(path))
        assert retry == [digest(pattern)]
        rows.append({"workers": cores, "worker_EOF_propagated": True,
                     "queues_drained_after_error": True,
                     "file_reference_retained_until_retry": retained_after_error,
                     "retry_restores_hash_and_clears_state": True,
                     "prelatched_stop_returns_nonempty_blank_hashes": stopped,
                     "stop_reset_retry_valid": True})
faulthandler.cancel_dump_traceback_later()
result = {"source_sha256": source_pins, "rows": rows, "normal_size_cases": 14,
          "worker_pools": [2, 4],
          "scope": "Complete original MTHash and actual Up2k._hashlist_from_file, constructor-free single-thread Up2k config with original worker threads. Bounded <=2MiB+17 files. Not live server, high-level cancellation publication, all scheduler races, performance or process-lifetime thread cleanup qualification."}
(work / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
