#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research/dedup-500'
REPORT = DIRECTORY / 'duplo19-native-controls-audit-20261015.json'
report = json.loads(REPORT.read_text())
MANIFEST = DIRECTORY / 'duplo19-complete-source-20261015.json'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()
assert sha(MANIFEST) == report['source_manifest_sha256']
manifest = json.loads(MANIFEST.read_text())
source = Path(manifest['source'])
evidence = DIRECTORY / 'evidence/rivo--duplo'
tree = json.loads((evidence / 'tree.json').read_text())
blobs = {row['path']: row for row in tree['tree'] if row['type'] == 'blob'}
assert len(blobs) == len(manifest['files']) == 12
assert set(blobs) == {row['path'] for row in manifest['files']}
original_names = {'duplo': set(), 'haar': set()}
for row in manifest['files']:
    content = (source / row['path']).read_bytes()
    assert len(content) == row['bytes'] == blobs[row['path']]['size']
    assert hashlib.sha1(b'blob ' + str(len(content)).encode() + b'\0' + content).hexdigest() == row['git_blob_sha1'] == blobs[row['path']]['sha']
    assert sha(source / row['path']) == row['sha256'] == sha(evidence / row['local'])
    if row['path'].endswith('_test.go'):
        package = 'haar' if row['path'].startswith('haar/') else 'duplo'
        original_names[package].update(re.findall(r'func (Test\w+|Example)\(', content.decode()))
assert sum(map(len, original_names.values())) == 13
authored = DIRECTORY / 'duplo19-native-controls-v2-20261015.go'
assert sha(authored) == report['authored_test_sha256'] == sha(source / 'rrrah_test.go')
author_names = set(re.findall(r'func (Test\w+)\(', authored.read_text()))
assert len(author_names) == 7
initial = (DIRECTORY / 'duplo19-native-controls-20261015.go').read_text()
assert initial.replace('references != 3*(ImageScale*ImageScale-1) || references != 49149', 'references != 3*(ImageScale*ImageScale-1)') == authored.read_text()
assert sha(source / 'go.mod') == report['generated_module_sha256']
assert sha(source / 'go.sum') == report['generated_sum_sha256']
for row in report['records']:
    assert sha(row['executable']) == row['executable_sha256']
    listed = subprocess.run([row['executable'], '-test.list', '.'], capture_output=True, text=True)
    assert listed.returncode == 0
    names = set(listed.stdout.splitlines())
    assert names == set(row['fresh_inventory'])
    expected = original_names[row['target']] | (author_names if row['target'] == 'duplo' else set())
    assert names == expected
    log = DIRECTORY / f"duplo19-{row['target']}-retained-runtime-20261015.log"
    assert sha(log) == row['log_sha256']
    assert set(re.findall(r'^--- PASS: (\w+)', log.read_text(), re.M)) == names
    assert log.read_text().endswith('PASS\n') and len(names) == row['passed']
assert sum(row['passed'] for row in report['records']) == 20
assert report['module_verification'] == 'all modules verified'
dependency = report['selected_dependency']
assert f"{dependency['module']} {dependency['version']} {dependency['checksum']}" in (source / 'go.sum').read_text().splitlines()
OUTPUT = DIRECTORY / 'duplo19-native-independent-audit-20261015.json'
assert not OUTPUT.exists()
OUTPUT.write_text(json.dumps(dict(status='verified12_source_blobs_and_complete20_native_case_inventory', auditor_sha256=sha(__file__), runtime_report_sha256=sha(REPORT), source_manifest_sha256=sha(MANIFEST), original_tests=12, original_examples=1, authored_tests=7, passed=20, failed=0, scope=report['scope'] + ' Independent source/evidence/Gitblob pins, both fresh retained binary inventories, all original/authored case names and logs, generated dependency checksum and authored-only vet repair checked.'), indent=2) + '\n')
print('12source blobs and20native cases independently verified')
