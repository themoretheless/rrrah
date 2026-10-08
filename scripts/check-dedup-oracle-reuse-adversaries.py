#!/usr/bin/env python3
"""Validate reuse provenance with correlated corruptions, without native reruns."""
import argparse
import copy
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("collection")
p.add_argument("reused_oracle")
p.add_argument("output")
a = p.parse_args()
collection = json.loads(Path(a.collection).read_text())
replayed = json.loads(Path(a.reused_oracle).read_text())
seed_path = replayed["results"][0]["reused_from"]
seed = json.loads(Path(seed_path).read_text())
assert len(seed["paths"]) == len(replayed["paths"]) == 12
verifier = Path(__file__).with_name("verify-dedup-six-multifile-parity.py")
results = []
with tempfile.TemporaryDirectory(prefix="rrrah-reuse-audit-") as folder:
    root = Path(folder)
    for case in ["baseline", "stale_seed_hash", "changed_seed_paths", "duplicate_seed_pair",
                 "different_probe", "correlated_counter_forgery", "wrong_reused_count",
                 "boolean_fresh_count", "boolean_fresh_memory"]:
        original = copy.deepcopy(seed)
        oracle = copy.deepcopy(replayed)
        native = copy.deepcopy(collection)
        source = root / (case + "-seed.json")
        oracle["input_hashes"].pop(seed_path)
        for row in oracle["results"]:
            row["reused_from"] = str(source)
        if case == "changed_seed_paths": original["paths"].reverse()
        if case == "duplicate_seed_pair": original["results"][1] = copy.deepcopy(original["results"][0])
        if case == "different_probe":
            keys = set(original["input_hashes"]) - set(original["paths"])
            assert len(keys) == 1
            old = keys.pop()
            replacement = str(Path(old).with_name("photo-probe-six-regions"))
            original["input_hashes"].pop(old)
            original["input_hashes"][replacement] = hashlib.sha256(Path(replacement).read_bytes()).hexdigest()
        source.write_text(json.dumps(original))
        oracle["input_hashes"][str(source)] = hashlib.sha256(source.read_bytes()).hexdigest()
        if case == "stale_seed_hash": source.write_text(source.read_text() + " ")
        if case == "correlated_counter_forgery":
            # Keep raw/parsed collection and oracle fields mutually consistent.
            # Only the pinned independent original exposes this forgery.
            oracle["results"][0]["evidence"]["spatial_gradient"]["inliers"] += 1
            oracle["results"][0]["stdout"] = json.dumps(oracle["results"][0]["evidence"])
            native["native"]["edges"][0]["spatial_gradient"]["inliers"] += 1
            native["stdout"] = json.dumps(native["native"])
        if case == "wrong_reused_count": oracle["reused_pairs"] -= 1
        if case.startswith("boolean_fresh"):
            oracle = copy.deepcopy(seed)
            if case == "boolean_fresh_count": oracle["reused_pairs"] = False
            else:
                oracle["results"][0]["evidence"]["managed_used"] = False
                oracle["results"][0]["stdout"] = json.dumps(oracle["results"][0]["evidence"])
        oracle_file = root / (case + "-oracle.json")
        collection_file = root / (case + "-collection.json")
        output = root / (case + "-audit.json")
        oracle_file.write_text(json.dumps(oracle))
        collection_file.write_text(json.dumps(native))
        run = subprocess.run(["python3", str(verifier), str(collection_file), str(oracle_file),
                              str(output), "--expected-files", "12", "--memory-limit-bytes", "134217728"],
                             capture_output=True, text=True)
        baseline = case == "baseline"
        assert (run.returncode == 0) == baseline, (case, run.stderr)
        assert output.exists() == baseline
        results.append(dict(case=case, returncode=run.returncode, audit_created=output.exists()))
Path(a.output).write_text(json.dumps(dict(
    status="verified_reuse_auditor_rejections", cases=results,
    input_hashes={f: hashlib.sha256(Path(f).read_bytes()).hexdigest()
                  for f in [a.collection, a.reused_oracle, seed_path, str(verifier), __file__]},
    scope="One valid replay plus8corruptions including correlated raw/parsed forgery; "
          "auditor behavior only, no new native pair confirmations."
), indent=2) + "\n")
