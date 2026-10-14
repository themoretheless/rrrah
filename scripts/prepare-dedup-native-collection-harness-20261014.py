#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'docs/research/dedup-consensus-native-collection-v5-source-20261014.json'
manifest = json.loads(MANIFEST.read_text())
source = Path(manifest['source'])
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
assert all(sha(source / name) == pin for name, pin in manifest['source_sha256'].items())
old = Path('/Users/themoretheless/.codex/tmp/rrrah-consensus-focused-harness-20261013')
harness = Path('/private/tmp/rrrah-native-collection-harness-20261014')
harness.mkdir(exist_ok=False)
(harness / 'tests').mkdir()
toml = (old / 'Cargo.toml').read_text().replace('/Users/themoretheless/.codex/tmp/rrrah-consensus-native-source-20261013', str(source))
toml = toml.replace('default = ["decode"]', 'default = ["decode", "loftr"]').replace('decode = []', 'decode = []\nloftr = []')
(harness / 'Cargo.toml').write_text(toml)
(harness / 'Cargo.lock').write_bytes((old / 'Cargo.lock').read_bytes())
for name in ['portfolio_collection', 'search_portfolio']:
    (harness / 'tests' / (name + '.rs')).write_bytes((source / 'crates/rrrah-dedup/tests' / (name + '.rs')).read_bytes())
subprocess.run(['cp', '-cR', str(source / 'crates/rrrah-dedup/tests/fixtures'), str(harness / 'tests/fixtures')], check=True)
target = Path('/private/tmp/rrrah-native-collection-target-20261014')
assert not target.exists()
subprocess.run(['cp', '-cR', '/Users/themoretheless/.codex/tmp/rrrah-dedup-exact-portfolio-20261011', str(target)], check=True)
assert all(sha(source / name) == pin for name, pin in manifest['source_sha256'].items())
out = ROOT / 'docs/research/dedup-native-collection-harness-20261014.json'
assert not out.exists()
out.write_text(json.dumps(dict(status='prepared_not_executed', source_manifest=str(MANIFEST), source_manifest_sha256=sha(MANIFEST), harness=str(harness), target=str(target), harness_sha256={str(p.relative_to(harness)): sha(p) for p in harness.rglob('*') if p.is_file()}, scope='Exact two frozen integration test sources, LoFTR dependency enabled, standalone optimized Cargo harness. Existing build cache cloned for reuse only; source/runtime checks remain required. Not whole workspace qualification.'), indent=2) + '\n')
print(out)
