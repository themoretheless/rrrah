from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / 'docs/research/dedup-500'
SOURCE = DIRECTORY / 'fizz457-complete-source-review-20261019.json'
RUNTIME = DIRECTORY / 'fizz457-selected-runtime-20261019.json'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
review = json.loads(SOURCE.read_text())
runtime = json.loads(RUNTIME.read_text())
assert review['blob_count'] == len(review['manifest']) == 19
assert review['tree_count'] == 3 and review['submodules'] == review['binary_assets'] == 0
assert review['whole_repo_complete'] and review['whole_implementation_complete']
archive = ROOT / review['source_archive']
assert {str(p.relative_to(archive)) for p in archive.rglob('*') if p.is_file()} == {x['path'] for x in review['manifest']}
for row in review['manifest']:
    p = archive / row['path']
    data = p.read_bytes()
    assert len(data) == row['bytes'] and sha(p) == row['sha256']
    assert hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest() == row['git_blob']
    assert row['fully_read']
for path, value in runtime['source_pins'].items():
    assert sha(archive / path) == value
assert sha(ROOT / 'scripts/qualify-fizz457-20261019.py') == runtime['producer_sha256']
assert len(runtime['findings']) == 7
output = DIRECTORY / 'fizz457-complete-audit-20261019.json'
assert not output.exists()
output.write_text(json.dumps({
    'status':'verified_complete_source_and_selected_actual_runtime',
    'whole_repo_complete':True,'whole_implementation_complete':True,
    'complete_source_review':str(SOURCE.relative_to(ROOT)),'complete_source_review_sha256':sha(SOURCE),
    'selected_runtime':str(RUNTIME.relative_to(ROOT)),'selected_runtime_sha256':sha(RUNTIME),
    'fully_read_blobs':19,'auditor_sha256':sha(Path(__file__)),
    'scope':'Complete pinned source read, all Gitblob hashes and archived bytes verified; actual original selected scanner/analyzer/ZIP controls. Runtime tests do not cover complete GUI, video decode, format/platform or release behavior.',
    'limits':review['limits']+runtime['limits']},indent=2)+'\n')
print(output)
