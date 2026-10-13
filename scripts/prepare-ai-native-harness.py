#!/usr/bin/env python3
"""Create source-linked, offline native AI test workspaces under ignored target/."""
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "target/ai-native-harness-20261013"


def versions(path):
    result = set()
    for package in path.read_text().split("[[package]]")[1:]:
        name = re.search(r'^name = "([^"]+)"', package, re.M)
        version = re.search(r'^version = "([^"]+)"', package, re.M)
        if name and version:
            result.add((name[1], version[1]))
    return result


def main():
    root_versions = versions(ROOT / "Cargo.lock")
    manifests = {}
    for name in ("hayro", "hayro-interpret"):
        source = ROOT / "vendor" / name
        output = BASE / name
        output.mkdir(parents=True, exist_ok=True)
        manifest = (source / "Cargo.toml").read_text()
        manifest = re.sub(r"(?ms)^\[\[example\]\]\n.*?(?=^\[|\Z)", "", manifest)
        manifest = manifest.replace('path = "src/lib.rs"', 'path = ' + json.dumps(str(source / "src/lib.rs"), ensure_ascii=False))
        manifest += '\n[workspace]\n[patch.crates-io]\nmoxcms={path=' + json.dumps(str(ROOT / "vendor/moxcms"), ensure_ascii=False) + '}\n'
        if name == "hayro":
            manifest += 'hayro-interpret={path=' + json.dumps(str(ROOT / "vendor/hayro-interpret"), ensure_ascii=False) + '}\n'
        manifest += "[profile.dev]\nopt-level=3\n"
        path = output / "Cargo.toml"
        path.write_text(manifest)
        shutil.copyfile(ROOT / "Cargo.lock", output / "Cargo.lock")
        # Prune the copied workspace lock offline, retaining its selected versions.
        subprocess.run(["cargo", "metadata", "--manifest-path", str(path), "--offline", "--format-version", "1"], cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
        extra = versions(output / "Cargo.lock") - root_versions
        if extra:
            raise RuntimeError("Test harness introduced versions absent from root lock: " + repr(sorted(extra)))
        manifests[name] = str(path)
    print(json.dumps({"manifests": manifests, "root_lock_versions_preserved": True}, indent=2))


if __name__ == "__main__":
    main()
