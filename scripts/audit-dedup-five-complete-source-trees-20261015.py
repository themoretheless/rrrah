"""Reconstruct Git tree objects and verify every archived blob against commit pins."""
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
D = ROOT / 'docs/research'
sha = lambda path: hashlib.sha256(Path(path).read_bytes()).hexdigest()


def git_object(kind, body):
    return hashlib.sha1(kind.encode() + b' ' + str(len(body)).encode() + b'\0' + body).hexdigest()


def reconstruct(entries):
    assert len({entry['path'] for entry in entries}) == len(entries)
    directories = [''] + [entry['path'] for entry in entries if entry['type'] == 'tree']
    built = {}
    for directory in sorted(directories, key=lambda p: p.count('/') + bool(p), reverse=True):
        children = [entry for entry in entries
                    if ('' if str(Path(entry['path']).parent) == '.' else str(Path(entry['path']).parent)) == directory]
        # Git compares tree names with a trailing slash; filenames as raw bytes.
        children.sort(key=lambda entry: Path(entry['path']).name.encode() + (b'/' if entry['type'] == 'tree' else b''))
        body = bytearray()
        for entry in children:
            assert entry['type'] in ('blob', 'tree')
            assert entry['mode'] in ('100644', '100755', '120000', '040000')
            if entry['type'] == 'tree':
                assert built[entry['path']] == entry['sha'], entry['path']
            body += entry['mode'].lstrip('0').encode() + b' ' + Path(entry['path']).name.encode() + b'\0' + bytes.fromhex(entry['sha'])
        built[directory] = git_object('tree', body)
    return built


assert git_object('tree', b'') == '4b825dc642cb6eb9a060e54bf8d69288fbee4904'
commit_pins = D / 'dedup-five-complete-source-commit-pins-20261015.json'
pins = json.loads(commit_pins.read_text())
assert {row['index'] for row in pins} == {164, 173, 191, 202, 329}
rows = []
for row in pins:
    registry_path = D / 'dedup-500/reviews' / f"{row['index']:03}.json"
    registry = json.loads(registry_path.read_text())
    assert registry['repository'] == row['repository']
    assert registry['commit'] == row['commit'] == row['observed_commit']
    base = D / 'dedup-500/evidence' / row['repository'].replace('/', '--')
    tree_path = base / 'tree.json'
    tree = json.loads(tree_path.read_text())
    assert tree['truncated'] is False and tree['sha'] == row['tree']
    built = reconstruct(tree['tree'])
    assert built[''] == row['tree']
    full = base / 'full-source'
    blobs = [entry for entry in tree['tree'] if entry['type'] == 'blob']
    assert {str(path.relative_to(full)) for path in full.rglob('*') if path.is_file()} == {entry['path'] for entry in blobs}
    report_path = ROOT / registry['complete_source_review']
    review = json.loads(report_path.read_text())
    assert review['commit'] == row['commit'] and review['repository'] == row['repository']
    review_blobs = {entry['path']: entry for entry in review['blobs']}
    assert len(review_blobs) == len(review['blobs']) == len(blobs)
    assert set(review_blobs) == {entry['path'] for entry in blobs}
    for entry in blobs:
        path = full / entry['path']
        body = path.read_bytes()
        assert len(body) == entry['size'] == review_blobs[entry['path']]['bytes']
        assert git_object('blob', body) == entry['sha'] == review_blobs[entry['path']]['git_blob']
        assert sha(path) == review_blobs[entry['path']]['sha256']
    # A missing tracked blob must change the reconstructed root or fail a child pin.
    omission_rejected = False
    try:
        incomplete = reconstruct([entry for entry in tree['tree'] if entry is not blobs[-1]])
        omission_rejected = incomplete[''] != row['tree']
    except AssertionError:
        omission_rejected = True
    assert omission_rejected, 'missing tracked blob was not detected'
    rows.append(dict(index=row['index'], repository=row['repository'], commit=row['commit'],
                     root_tree=row['tree'], trees_verified=len(built), blobs_verified=len(blobs),
                     registry_sha256=sha(registry_path), tree_manifest_sha256=sha(tree_path),
                     review_sha256=sha(report_path)))
output = D / 'dedup-five-complete-source-tree-audit-20261015.json'
assert not output.exists()
output.write_text(json.dumps(dict(
    status='verified_complete_commit_tree_and_archive_inventories', rows=rows,
    blobs_verified=sum(row['blobs_verified'] for row in rows),
    trees_verified=sum(row['trees_verified'] for row in rows),
    commit_pins_sha256=sha(commit_pins), auditor_sha256=sha(__file__),
    limits=['Fresh GitHub commit-to-tree observations plus independently reconstructed Git objects and complete archive inventories.',
            'Archive identity and completeness, not automated proof of human source understanding or new runtime/backend/platform/quality qualification.',
            'Symlink blobs checked as literal bytes and mode; targets not followed. No changes to upstream evidence.'],
), indent=2) + '\n')
print(output, 'blobs', sum(row['blobs_verified'] for row in rows))
