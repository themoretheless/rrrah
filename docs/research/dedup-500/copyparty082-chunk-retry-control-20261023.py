#!/usr/bin/env python3
"""Bounded corrupt-chunk then correct retry through original virtual HTTP pipeline."""
from pathlib import Path
import base64
import faulthandler
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
faulthandler.dump_traceback_later(30, exit=True)
from tests import util as tu
tu.get_ramdisk = lambda: tempfile.mkdtemp(prefix="owned-", dir=work)
from tests import test_dedup as original
from copyparty.httpcli import HttpCli

rows = []
for bad in [b"two", b"ones"]:
    tc = original.TestDedup("test_a")
    tc.setUp()
    tc.conn = None
    tc.fstab = None
    try:
        tc.args = tu.Cfg(v=[".::A"], a=[], e2d=True, safe_dedup=50,
                         bf_log="", bak_flips=False)
        root = Path(tc.reset())
        tc.cinit()
        fi = tc.files[0]
        name, hs = tc.do_post_hs("d", "f1", fi, True)
        def send(data):
            headers = ["POST /d/ HTTP/1.1", "Connection: close",
                       "Content-Type: application/octet-stream", "Content-Length: " + str(len(data)),
                       "X-Up2k-Hash: " + fi[1], "X-Up2k-Wark: " + hs["wark"], "", ""]
            request = "\r\n".join(headers).encode() + data
            HttpCli(tc.conn.setbuf(request)).run()
            head, body = tc.conn.s._reply.split(b"\r\n\r\n", 1)
            return head.decode().splitlines()[0], body.decode()
        bad_status, bad_body = send(bad)
        assert bad_status.startswith("HTTP/1.1 400") and "corrupted" in bad_body
        engine = tc.conn.hsrv.hub.up2k
        ptop = tc.asrv.vfs.all_vols[""].realpath
        job = engine.registry[ptop][hs["wark"]]
        assert job["need"] == [fi[1]] and not job["busy"]
        temporary = Path(job["ptop"]) / job["prel"] / job["tnam"]
        temporary_bytes = temporary.read_bytes()
        _, next_hs_body = tc.handshake("d", name, fi)
        next_hs = json.loads(next_hs_body)
        assert next_hs["hash"] == [fi[1]]
        good_status, good_body = send(b"one")
        assert good_status.startswith("HTTP/1.1 200") and good_body == "thank"
        final = root / "d" / name
        actual = final.read_bytes()
        _, returned = tc.curl("d/" + name)
        assert returned.encode() == actual
        actual_hash = base64.urlsafe_b64encode(hashlib.sha512(actual).digest()[:33]).decode()
        if len(bad) == 3:
            assert actual == b"one" and actual_hash == fi[1]
        rows.append({"bad_payload": bad.decode(), "bad_status": bad_status,
                     "bad_chunk_stays_needed_and_unlocked": True,
                     "temporary_after_bad_chunk": temporary_bytes.decode(),
                     "retry_handshake_requests_chunk": True, "retry_status": good_status,
                     "final_file": actual.decode(), "final_size": len(actual),
                     "declared_size": 3, "matches_requested_bytes": actual == b"one",
                     "matches_declared_chunk_hash": actual_hash == fi[1]})
    finally:
        tc.tearDown()
faulthandler.cancel_dump_traceback_later()
result = {"rows": rows,
          "scope": "Original HttpCli/up2k/SQLite/file handling with original virtual HTTP fixtures, owned tempfile allocator and safededup50. Debug corrupt-byte backup disabled. Two bounded input/retry sequences only; not real network server or all upload/race/platform behavior."}
(work / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result, indent=2))
