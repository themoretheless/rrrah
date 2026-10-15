#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research/dedup-500'
REPORT = DIRECTORY / 'imgsim365-native-controls-audit-20261015.json'
report = json.loads(REPORT.read_text())
MANIFEST = DIRECTORY / 'imgsim365-complete-source-20261015.json'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert sha(MANIFEST) == report['source_manifest_sha256']
manifest = json.loads(MANIFEST.read_text())
source = Path(manifest['source'])
evidence = DIRECTORY / 'evidence/Nr90--imgsim'
tree = json.loads((evidence / 'tree.json').read_text())
blobs = {row['path']: row for row in tree['tree'] if row['type'] == 'blob'}
assert len(blobs) == len(manifest['files']) == 15
assert set(blobs) == {row['path'] for row in manifest['files']}
for row in manifest['files']:
    content = (source / row['path']).read_bytes()
    assert len(content) == row['bytes'] == blobs[row['path']]['size']
    assert hashlib.sha1(b'blob ' + str(len(content)).encode() + b'\0' + content).hexdigest() == row['git_blob_sha1'] == blobs[row['path']]['sha']
    assert sha(source / row['path']) == row['sha256'] == sha(evidence / row['local'])
assert sha(source / 'rrrah_test.go') == report['test_sha256']
assert sha(DIRECTORY / 'imgsim365-native-controls-20261015.go') == report['test_sha256']
assert sha(source / 'controls.test') == report['executable_sha256']
assert sha(source / 'go.mod') == report['generated_module_sha256']
assert sha(source / 'go.sum') == report['generated_sum_sha256']
assert sha(DIRECTORY / 'imgsim365-native-controls-20261015.log') == report['successful_log_sha256']
assert sha(DIRECTORY / 'imgsim365-initial-wrong-directory-20261015.log') == report['initial_failure_log_sha256']
listed = subprocess.run([str(source / 'controls.test'), '-test.list', '.'], capture_output=True, text=True)
assert listed.returncode == 0
names = listed.stdout.splitlines()
assert len(names) == len(set(names)) == 10
assert set(names) == set(report['fresh_inventory'])
body = (DIRECTORY / 'imgsim365-native-controls-20261015.log').read_text()
assert set(re.findall(r'^--- PASS: (\w+)', body, re.M)) == set(names)
assert body.endswith('PASS\n')
original_names = set()
for row in manifest['files']:
    if row['path'].endswith('_test.go'):
        original_names.update(re.findall(r'func (Test\w+)\(', (source / row['path']).read_text()))
assert len(original_names) == 4 and original_names == set(report['original_test_functions'])
assert original_names.issubset(names)
dependency = report['compiled_dependency']
assert dependency['version'].endswith('-891127d8d1b5')
assert f"{dependency['module']} {dependency['version']} {dependency['checksum']}" in (source / 'go.sum').read_text().splitlines()
assert 'revision = "891127d8d1b52734debe1b3c3d7e747502b6c366"' in (source / 'Gopkg.lock').read_text()
OUTPUT = DIRECTORY / 'imgsim365-native-independent-audit-20261015.json'
assert not OUTPUT.exists()
OUTPUT.write_text(json.dumps(dict(status='verified_all15_source_fixture_blobs_and_complete10test_inventory', auditor_sha256=sha(__file__), runtime_report_sha256=sha(REPORT), source_manifest_sha256=sha(MANIFEST), executable_sha256=report['executable_sha256'], passed=10, failed=0, original_test_functions=4, authored_test_functions=6, independent_pixel_grids=512, independent_distance_pairs=512, complete_inventory=names, scope=report['scope'] + ' Independent current source/evidence/Gitblob pins, unchanged upstream test names, fresh binary inventory, successful/failed logs and original lock revision versus generated module checksum verified.'), indent=2) + '\n')
print('15source/fixture blobs,4original+6authored tests and locked revision independently verified')
