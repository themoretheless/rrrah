#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json

ROOT = Path(__file__).resolve().parents[1]
REPORT_PATH = ROOT / 'docs/research/dedup-current-test-metadata-v1-20261015.json'
report = json.loads(REPORT_PATH.read_text())
assert len(report['failed_targets']) == 9 and not report['source_changes']
BACKUP = Path('/private/tmp/rrrah-strict-test-warning-cleanup-20261015')
BACKUP.mkdir(exist_ok=False)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def once(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new, 1)

updated = {}
for name in report['failed_targets']:
    path = ROOT / f'crates/rrrah-dedup/tests/{name}.rs'
    before = path.read_bytes()
    assert sha(before) == report['source_pins'][str(path.relative_to(ROOT))]
    text = before.decode()
    if name in ['anchor_collection_preflight', 'anchor_geometry_collection_preflight']:
        text = once(text, '    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rotation");\n    let a = rrrah_decode::DecodeRequest::new(root.join("base.png"));\n', '')
    if name in ['five_collection', 'five_gradient_region_lifecycle', 'five_region_admission', 'five_region_lifecycle']:
        text = once(text, '            LocalFileError, ProjectiveComplementaryFilePolicy, ProjectivePortfolioFilePolicy,', '            ProjectiveComplementaryFilePolicy, ProjectivePortfolioFilePolicy,')
    if name in ['five_gradient_region_lifecycle', 'five_region_admission']:
        text = once(text, '            ProjectiveFiveSearchCollectionPolicy, scan_projective_local_collection_five_searches,', '            ProjectiveFiveSearchCollectionPolicy,')
    if name == 'five_region_admission':
        text = once(text, '    use std::{cell::Cell, io::Write};\n', '')
        fixture = '''    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/photos-heldout");
    let dir = tempfile::tempdir().unwrap();
    let a_path = dir.path().join("a.png");
    let b_path = dir.path().join("b.png");
    std::fs::copy(source.join("2414-base.png"), &a_path).unwrap();
    std::fs::copy(&a_path, &b_path).unwrap();
    let a = DecodeRequest::new(&a_path);
    let b = DecodeRequest::new(&b_path);
'''
        text = once(text, fixture, '')
        text = once(text, '''    let mut files = vec![
        (1, a.clone()),
        (2, b.clone()),
        (3, DecodeRequest::new(source.join("5495-base.png"))),
    ];
''', '')
        unused = '    let budget = MemoryBudget::new(64 * 1024 * 1024);\n'
        assert text.count(unused) == 2
        text = text.replace(unused, '', 1)
    if name == 'gradient_collection':
        text = once(text, '''        local_scan::{
            LocalFileError, ProjectiveComplementaryFilePolicy, ProjectivePortfolioFilePolicy,
            ProjectivePyramidPhotometricFilePolicy,
        },''', '        local_scan::ProjectivePyramidPhotometricFilePolicy,')
        text = once(text, '''            ProjectiveRegistrationPolicy, ProjectiveRegistrationPortfolioPolicy,
            ProjectiveRegistrationTrustPolicy,
''', '')
    if name == 'projective_pruning':
        for capture in ['204100', '211900']:
            text = once(text, f'mod data_{capture} {{', '// Shared captures retain legacy outputs; this regression compares the full reference directly.\n#[expect(dead_code)]\n' + f'mod data_{capture} {{')
    if name == 'region_transform':
        text = once(text, '    local_scan::{LocalFilePolicy, compare_local_files_projective},', '    local_scan::LocalFilePolicy,')
        text = once(text, '''        local_scan::{
            LocalFileError, ProjectivePyramidPhotometricFilePolicy,
            compare_local_files_projective_pyramid_photometric,
        },''', '        local_scan::{LocalFileError, ProjectivePyramidPhotometricFilePolicy},')
        text = once(text, '    drop(unrelated_view);\n', '')
        text = once(text, '    drop(view);\n', '')
    updated[path] = (before, text)

for path, (before, after) in updated.items():
    (BACKUP / path.name).write_bytes(before)
    path.write_text(after)
print('Updated nine test sources; original files retained, test assertions preserved')
