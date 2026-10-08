#!/usr/bin/env python3
"""Check one six-source retrieval against all fifteen independent pair confirmations (pinned reused observations allowed)."""
import argparse
import hashlib
import itertools
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("collection")
parser.add_argument("oracle")
parser.add_argument("output")
parser.add_argument("--expected-files",type=int,default=6)
parser.add_argument("--memory-limit-bytes",type=int,default=64*1024*1024)
args = parser.parse_args()

def exact_json(left, right):
    # Python considers True == 1; native evidence must retain JSON types.
    return json.dumps(left, sort_keys=True, separators=(",", ":")) == json.dumps(
        right, sort_keys=True, separators=(",", ":"))

n=args.expected_files
assert n>=2 and args.memory_limit_bytes>=0
pair_count=n*(n-1)//2
collection = json.loads(Path(args.collection).read_text())
oracle = json.loads(Path(args.oracle).read_text())
assert oracle["status"] == "complete"
assert collection["returncode"] == 0
assert collection["paths"] == oracle["paths"] and len(oracle["paths"]) == n
for report in (collection, oracle):
    assert all(path in report["input_hashes"] for path in report["paths"])
    for path, digest in report["input_hashes"].items():
        assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest, path
native = json.loads(collection["stdout"])
assert exact_json(native, collection["native"]) and native["status"] == "ok"
assert native["files"] == n and native["managed_used"] == 0
assert 0 <= native["managed_peak"] <= args.memory_limit_bytes
assert type(native.get("managed_limit",64*1024*1024)) is int
assert native.get("managed_limit",64*1024*1024)==args.memory_limit_bytes
work=5*(1500*n)**2
assert native["max_gradient_retrieval_comparisons"]==work
assert 0<=native["indexed_features"]<=6500*n
assert 0<=native["descriptor_hits"]<=2*work
for row in oracle["results"]:
    assert type(row["left"]) is int and type(row["right"]) is int
    assert type(row["returncode"]) is int
required = set(itertools.combinations(range(1, n+1), 2))
for edge in native["edges"]:
    assert type(edge["left"]) is int and type(edge["right"]) is int
for field in ("files", "managed_used", "managed_peak", "proposed_pairs", "indexed_features", "descriptor_hits", "max_gradient_retrieval_comparisons"):
    assert type(native[field]) is int
edges = {(e["left"], e["right"]): e for e in native["edges"]}
assert len(edges) == len(native["edges"]) == native["proposed_pairs"] == pair_count
assert set(edges) == required
rows = {(r["left"], r["right"]): r for r in oracle["results"]}
assert len(rows) == len(oracle["results"]) == pair_count and set(rows) == required
reuse_sources={}
active_sources=set()

def program_pins(report):
    references={r["reused_from"] for r in report["results"] if "reused_from" in r}
    keys=set(report["input_hashes"])-set(report["paths"])-references
    assert len(keys)==1, "One pinned probe executable required"
    return {p:report["input_hashes"][p] for p in keys}

def load_reuse_source(path, depth=0):
    key=str(Path(path).resolve())
    assert depth<64 and key not in active_sources
    if key in reuse_sources:return reuse_sources[key]
    active_sources.add(key)
    seed=json.loads(Path(path).read_text())
    assert seed["status"]=="complete" and len(seed["paths"])>=2
    assert len(set(seed["paths"]))==len(seed["paths"])
    assert all(p in seed["input_hashes"] for p in seed["paths"])
    for p,h in seed["input_hashes"].items():
        assert hashlib.sha256(Path(p).read_bytes()).hexdigest()==h,p
    ids=set(itertools.combinations(range(1,len(seed["paths"])+1),2))
    rows={(r["left"],r["right"]):r for r in seed["results"]}
    assert len(rows)==len(seed["results"])==len(ids) and set(rows)==ids
    for row in seed["results"]:
        assert type(row["left"]) is int and type(row["right"]) is int
        assert type(row["returncode"]) is int and row["returncode"]==0
        assert exact_json(json.loads(row["stdout"]),row["evidence"])
        if "reused_from" in row:
            child=row["reused_from"]
            assert child in seed["input_hashes"]
            earlier,table=load_reuse_source(child,depth+1)
            assert earlier["paths"]==seed["paths"][:len(earlier["paths"])]
            assert program_pins(earlier)==program_pins(seed)
            previous=table[(row["left"],row["right"])]
            assert exact_json(row["evidence"],previous["evidence"])
            assert row["stdout"]==previous["stdout"]
    active_sources.remove(key)
    reuse_sources[key]=(seed,rows)
    return seed,rows

reused_count=0
for pair in sorted(required):
    row = rows[pair]
    if "reused_from" in row:
        source=row["reused_from"]
        assert source in oracle["input_hashes"]
        seed,table=load_reuse_source(source)
        assert seed["paths"]==oracle["paths"][:len(seed["paths"])]
        assert program_pins(seed)==program_pins(oracle)
        previous=table[pair]
        assert exact_json(row["evidence"],previous["evidence"])
        assert row["stdout"]==previous["stdout"] and row["returncode"]==previous["returncode"]
        reused_count+=1
    assert row["returncode"] == 0
    direct = json.loads(row["stdout"])
    assert exact_json(direct, row["evidence"]) and direct["status"] == "ok"
    assert direct["retrieved"] is True and type(direct["managed_used"]) is int and direct["managed_used"] == 0
    assert type(direct["managed_peak"]) is int and 0<=direct["managed_peak"]<=64*1024*1024
    edge = edges[pair]
    assert exact_json(edge["legacy_candidate"], direct["candidate"]), pair
    assert exact_json(edge["legacy_searches"], direct["five_accepted_searches"]), pair
    assert exact_json(edge["spatial_gradient"], direct["spatial_gradient"]), pair
    expected_binary = None
    if direct["regional_geometry"] is not None:
        expected_binary = dict(geometry=direct["regional_geometry"],
                               regions=direct["regions"],
                               region_support_count=direct["region_support_count"])
    assert exact_json(edge["binary_regions"], expected_binary), pair
    assert exact_json(edge["gradient_regions"], [*direct["gradient_regions"],
                                         direct["spatial_gradient_regions"]]), pair
assert type(oracle.get("reused_pairs",0)) is int
assert reused_count==oracle.get("reused_pairs",0)
Path(args.output).write_text(json.dumps({
    "status": "verified", "verified_pairs": pair_count,
    "managed_peak": native["managed_peak"],
    "reused_pair_observations": reused_count,
    "input_hashes": {p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
                     for p in (args.collection, args.oracle, __file__)},
    "scope": f"Fixed {n}-source collection equals all{pair_count} independent pair confirmations (pinned reused observations allowed). "
             "No broad corpus, scale, precision or universal RSS claim."
}, indent=2) + "\n")
