from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / 'docs/research/dedup-500/blockhash400-complete-source-review-20261019.json'
review = json.loads(REPORT.read_text())
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
checkout = ROOT / '.dedup-work/blockhash400-20261019/source'
archive = ROOT / review['source_archive']
assert review['blob_count'] == len(review['manifest']) == 32
assert review['text_blobs'] == 15 and review['image_blobs'] == 17
for row in review['manifest']:
    original = checkout / row['path']
    data = original.read_bytes()
    assert len(data) == row['bytes'] and sha(original) == row['sha256']
    assert hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest() == row['git_blob']
    if row['review'] == 'fully_read_text':
        assert sha(archive / row['path']) == row['sha256']
    else:
        assert row['review'] == 'visually_reviewed_white_composited_contact_sheet'
assert sha(ROOT / review['visual_review']['contact_sheet']) == review['visual_review']['contact_sheet_sha256']
assert len(review['published_golden_analysis']['exact_vs_quick_hamming']) == 17
output = ROOT / 'docs/research/dedup-500/blockhash400-complete-audit-20261019.json'
assert not output.exists()
output.write_text(json.dumps({'status':'verified_full_pinned_source_and_visual_fixture_inventory',
    'whole_repo_complete':True,'whole_implementation_complete':True,
    'review':str(REPORT.relative_to(ROOT)),'review_sha256':sha(REPORT),
    'source_blobs':15,'visual_fixture_blobs':17,'auditor_sha256':sha(Path(__file__)),
    'native_upstream_tests_run':False,'native_runtime_qualified':False,
    'scope':'Complete original source and fixture reading, byte-pins verified; stored golden analysis only. No installed Haskell compiler and no native algorithm/runtime qualification.',
    'limits':review['limits']},indent=2)+'\n')
print(output)
