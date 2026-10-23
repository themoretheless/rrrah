"""Independent reconstruction of available adaptive tracks; no decoding claim."""
import contextlib
import hashlib
import io
import json
import runpy
from pathlib import Path
with contextlib.redirect_stdout(io.StringIO()):
    control = runpy.run_path(str(Path(__file__).with_name("pyfilefixity044-independent-parity-control-20261023.py")))
verified, parity, BASE = (control[k] for k in ("verified", "parity", "BASE"))
results, skipped = [], []
for name in ["test_repair_ecc_sa_check.db", "test_structural_adaptive_ecc_test_algo.db", "test_structural_adaptive_ecc_test_dir.db", "test_structural_adaptive_ecc_test_one_file.db", "test_structural_adaptive_ecc_test_one_file_tamper.db"]:
    raw = verified("pyFileFixity/tests/results/" + name)
    for entry in raw.split(b"\xfe\xff" * 5)[1:]:
        fields = entry.split(b"\xfa\xff\xfa\xff\xfa", 4)
        rel = "pyFileFixity/tests/files/" + fields[0].decode("latin1")
        if not (BASE / rel).is_file():
            skipped.append({"database": name, "fixture": rel})
            continue
        payload = verified(rel)
        assert len(payload) == int(fields[1])
        position = offset = blocks = 0
        ks = []
        while position < len(payload):
            rate = .3 if position < 1024 else .2 + float(position-1024) * (.1-.2) / (len(payload)-1024)
            k = int(round(255.0 / (1 + 2*rate), 0))
            message = payload[position:position+k]
            width = 32 + 255 - k
            stored = fields[4][offset:offset+width]
            expected = hashlib.md5(message).hexdigest().encode("ascii") + parity(message, k)
            assert stored == expected, (name, rel, position, k)
            changed = bytes([message[0] ^ 1]) + message[1:]
            assert parity(changed, k) != stored[32:]
            position += len(message)
            offset += width
            blocks += 1
            ks.append(k)
        assert offset == len(fields[4])
        results.append({"database": name, "fixture": rel, "bytes_covered": position, "track_bytes_covered": offset, "verified_blocks": blocks, "message_sizes": ks})
print(json.dumps({"scope": "Independent exact MD5+ECC reconstruction of adaptive tracks for available original fixtures, source-defined defaults and explicit complete track consumption; no imported decoder or repair", "records": results, "verified_blocks": sum(x["verified_blocks"] for x in results), "single_byte_parity_negative_controls": sum(x["verified_blocks"] for x in results), "skipped_missing_local_fixtures": skipped, "result": "PASS"}, indent=2))
