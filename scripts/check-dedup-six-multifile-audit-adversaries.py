#!/usr/bin/env python3
"""Exercise the parity auditor with synthetic reports; never claim native success."""
import argparse
import copy
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("oracle")
parser.add_argument("output")
args = parser.parse_args()
oracle = json.loads(Path(args.oracle).read_text())
assert oracle["status"] == "complete" and len(oracle["results"]) == 15
edges = []
for row in oracle["results"]:
    e = row["evidence"]
    binary = None if e["regional_geometry"] is None else {
        "geometry": e["regional_geometry"], "regions": e["regions"],
        "region_support_count": e["region_support_count"]}
    edges.append(dict(left=row["left"], right=row["right"],
                      legacy_candidate=e["candidate"],
                      legacy_searches=e["five_accepted_searches"],
                      spatial_gradient=e["spatial_gradient"],
                      binary_regions=binary,
                      gradient_regions=[*e["gradient_regions"], e["spatial_gradient_regions"]]))
native = dict(status="ok", files=6, managed_used=0, managed_peak=0,
              proposed_pairs=15, edges=edges,indexed_features=100,descriptor_hits=100,
              max_gradient_retrieval_comparisons=405000000)
cases = {}
def altered(name, mutate):
    value = copy.deepcopy(native)
    mutate(value)
    cases[name] = value
altered("missing_pair", lambda v: v["edges"].pop())
altered("duplicate_pair", lambda v: v["edges"].__setitem__(1, copy.deepcopy(v["edges"][0])))
altered("integer_candidate", lambda v: v["edges"][0].__setitem__("legacy_candidate", int(v["edges"][0]["legacy_candidate"])))
altered("boolean_file_id", lambda v: v["edges"][0].__setitem__("left", True))
altered("boolean_peak", lambda v: v.__setitem__("managed_peak", False))
altered("excess_memory", lambda v: v.__setitem__("managed_peak", 64 * 1024 * 1024 + 1))
altered("changed_spatial_decision", lambda v: v["edges"][0]["spatial_gradient"].__setitem__("candidate", not v["edges"][0]["spatial_gradient"]["candidate"]))
altered("changed_region_count", lambda v: v["edges"][0]["gradient_regions"][0].__setitem__("region_support_count", -1))
altered("integer_hit_count", lambda v: v.__setitem__("descriptor_hits", True))
altered("excess_features", lambda v: v.__setitem__("indexed_features", 39001))
altered("excess_hits", lambda v: v.__setitem__("descriptor_hits", 810000001))
altered("missing_region_lane", lambda v: v["edges"][0]["gradient_regions"].pop())
verifier = Path(__file__).with_name("verify-dedup-six-multifile-parity.py")
results = []
with tempfile.TemporaryDirectory(prefix="rrrah-six-audit-") as folder:
    root = Path(folder)
    for name, value in [("synthetic_baseline", native), *cases.items()]:
        report = dict(returncode=0, paths=oracle["paths"],
                      input_hashes=oracle["input_hashes"], native=value,
                      stdout=json.dumps(value))
        source = root / (name + ".json")
        output = root / (name + "-audit.json")
        source.write_text(json.dumps(report))
        process = subprocess.run(["python3", str(verifier), str(source), args.oracle, str(output)],
                                 capture_output=True, text=True)
        baseline = name == "synthetic_baseline"
        assert (process.returncode == 0) == baseline, (name, process.stderr)
        assert output.exists() == baseline, name
        results.append(dict(case=name, returncode=process.returncode,
                            audit_created=output.exists()))
Path(args.output).write_text(json.dumps(dict(
    status="verified_auditor_rejections", cases=results,
    input_hashes={p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
                  for p in [args.oracle, str(verifier), __file__]},
    scope="Synthetic baseline derived from15 pair records plus12 corruptions; "
          "auditor behavior only, no successful native six-source collection claim."
), indent=2) + "\n")
