"""Independently validate stored MD5 chunks against available original fixtures; no ECC claim."""
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
BASE = ROOT / ".dedup-work/pyfilefixity044-20261023"
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
pins = {x["path"]: x["sha"] for x in entries if x.get("type") == "blob"}
def verified(rel):
    data = (BASE / rel).read_bytes()
    assert hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest() == pins[rel]
    return data
records, skipped = [], []
for name in ["test_header_ecc_test_algo.db", "test_header_ecc_test_dir.db", "test_header_ecc_test_one_file.db", "test_header_ecc_test_one_file_tamper.db", "test_repair_ecc_check.db"]:
    raw = verified("pyFileFixity/tests/results/" + name)
    for entry in raw.split(b"\xfe\xff" * 5)[1:]:
        fields = entry.split(b"\xfa\xff\xfa\xff\xfa", 4)
        rel = "pyFileFixity/tests/files/" + fields[0].decode("latin1")
        if not (BASE / rel).is_file():
            skipped.append({"database": name, "fixture": rel})
            continue
        original = verified(rel)
        assert len(original) == int(fields[1])
        header = original[:1024]
        chunks = [header[i:i+159] for i in range(0, len(header), 159)]
        track = fields[4]
        assert len(track) == len(chunks) * 128
        for i, chunk in enumerate(chunks):
            stored = track[i*128:i*128+32]
            assert hashlib.md5(chunk).hexdigest().encode("ascii") == stored
            changed = bytes([chunk[0] ^ 1]) + chunk[1:]
            assert hashlib.md5(changed).hexdigest().encode("ascii") != stored
        records.append({"database": name, "fixture": rel, "verified_md5_chunks": len(chunks), "mutated_chunks_detected": len(chunks)})
print(json.dumps({"scope": "Independent hashlib MD5 versus original pinned database chunks, header defaults1024/message159/hash32/ecc96 derived from reviewed source; no ECC decoding or original integration suite", "records": records, "total_verified_md5_chunks": sum(x["verified_md5_chunks"] for x in records), "skipped_missing_local_fixtures": skipped}, indent=2))
