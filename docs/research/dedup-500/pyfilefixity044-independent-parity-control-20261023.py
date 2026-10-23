"""Independent bitwise GF encoder versus pinned ECC bytes; no repair/decoding claim."""
import functools
import hashlib
import json
from pathlib import Path
ROOT = Path(__file__).resolve().parents[3]
BASE = ROOT / ".dedup-work/pyfilefixity044-20261023"
entries = json.loads((ROOT / "docs/research/dedup-500/evidence/lrq3000--pyFileFixity/tree.json").read_text())["tree"]
pins = {x["path"]: x["sha"] for x in entries if x.get("type") == "blob"}
def verified(rel):
    raw = (BASE / rel).read_bytes()
    assert hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == pins[rel]
    return raw
def mul(a, b):
    result = 0
    while b:
        if b & 1: result ^= a
        b >>= 1
        a <<= 1
        if a & 256: a ^= 0x11b
    return result
@functools.lru_cache(None)
def generator(nsym):
    poly = [1]
    root = 3
    for _ in range(nsym):
        expanded = [0] * (len(poly) + 1)
        for i, coefficient in enumerate(poly):
            expanded[i] ^= coefficient
            expanded[i+1] ^= mul(coefficient, root)
        poly = expanded
        root = mul(root, 3)
    return poly
@functools.lru_cache(None)
def parity(message, k):
    nsym = 255 - k
    assert len(message) <= k
    padded = bytes(k - len(message)) + message
    work = list(padded) + [0] * nsym
    gen = generator(nsym)
    for i in range(k):
        coefficient = work[i]
        if coefficient:
            for j in range(1, len(gen)):
                work[i+j] ^= mul(coefficient, gen[j])
    return bytes(work[k:])
metadata = header = 0
for rel in sorted(pins):
    if not rel.startswith("pyFileFixity/tests/results/") or not rel.endswith(".db"): continue
    raw = verified(rel)
    for entry in raw.split(b"\xfe\xff" * 5)[1:]:
        fields = entry.split(b"\xfa\xff\xfa\xff\xfa", 4)
        assert len(fields) == 5
        for message, stored in [(fields[0], fields[2]), (fields[1], fields[3])]:
            assert parity(message, 128) == stored, (rel, fields[0], "metadata")
            changed = bytes([message[0] ^ 1]) + message[1:]
            assert parity(changed, 128) != stored
            metadata += 1
        fixture = "pyFileFixity/tests/files/" + fields[0].decode("latin1")
        is_header = Path(rel).name.startswith("test_header_ecc_") or Path(rel).name == "test_repair_ecc_check.db"
        if not is_header or not (BASE / fixture).is_file(): continue
        payload = verified(fixture)[:1024]
        blocks = [payload[i:i+159] for i in range(0, len(payload), 159)]
        assert len(fields[4]) == len(blocks) * 128
        for i, message in enumerate(blocks):
            stored = fields[4][i*128+32:(i+1)*128]
            assert parity(message, 159) == stored
            assert parity(bytes([message[0] ^ 1]) + message[1:], 159) != stored
            header += 1
print(json.dumps({"scope": "Authored independent bitwise GF(256) polynomial encoder, prim0x11b/generator3/fcr1/left-padding from reviewed source; no imported ECC package or decoding", "metadata_parity_comparisons": metadata, "available_header_parity_comparisons": header, "single_byte_mutation_negative_controls": metadata + header, "distinct_cached_message_k_pairs_including_mutations": parity.cache_info().currsize, "result": "PASS", "remaining": "Unavailable source payloads and adaptive data tracks not verified in this control; no recovery decoding or all-cases proof"}, indent=2))
