#!/usr/bin/env python3
"""Audit the public cancellation reproduction and executed regression suites."""
import hashlib
import json
import re
import shutil
import subprocess
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def main():
    red_path = D / 'dedup-visual-generation-red-v3-20261012.json'
    red = json.loads(red_path.read_text())
    assert red['status'] == 'reproduced_generation_cancellation_publishes_analysed_file'
    assert all(sha(p) == h for p, h in red['pins'].items())
    red_log = T / 'rrrah-visual-generation-red-v3-20261012.log'
    assert 'Ok(VisualReport { analysed: [1]' in red_log.read_text()
    source_path = D / 'dedup-visual-generation-green-source-20261012.json'
    source = json.loads(source_path.read_text())
    snapshot = Path(source['snapshot'])
    assert all(sha(snapshot / p) == h for p, h in source['file_hashes'].items())
    for p in ['crates/rrrah-dedup/src/scan.rs', 'crates/rrrah-dedup/tests/scan.rs']:
        assert sha(R / p) == source['file_hashes'][p]
    before = (T / 'rrrah-visual-generation-source-before-20261012.rs').read_text()
    current = (R / 'crates/rrrah-dedup/src/scan.rs').read_text()
    doc = ('/// Cancellation of any admitted request generation invalidates the batch,\n'
           '/// including during index search and final report publication.\n')
    block_start = current.index('    let mut tokens = Vec::new();')
    block_end = current.index('    let mut ordered_requests = Vec::new();', block_start)
    reconstructed = current[:block_start] + current[block_end:]
    assert reconstructed.replace(doc, '', 1) == before
    tests = (R / 'crates/rrrah-dedup/tests/scan.rs').read_text()
    name = 'visual_batch_final_publication_observes_request_generation_cancellation'
    start = tests.index('\n#[test]\nfn ' + name)
    end = tests.index('\n#[test]', start + 1)
    assert tests[:start] + tests[end:] == (T / 'rrrah-visual-generation-tests-before-20261012.rs').read_text()
    log = T / 'rrrah-visual-generation-green-20261012.log'
    text = log.read_text()
    assert 'test ' + name + ' ... ok' in text
    summaries = re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;', text)
    assert len(summaries) == 3
    assert 'test result: FAILED' not in text
    pins = {str(p): sha(p) for p in [red_path, source_path, log, Path(__file__)]}
    executables = []
    matches = re.findall(r'Running (unittests src/lib.rs|tests/exact_files.rs|tests/scan.rs) \(([^)]+)\)', text)
    assert len(matches) == 3
    for label, path in matches:
        suffix = 'lib' if label.startswith('unittests') else Path(label).stem
        dest = T / f'rrrah-visual-generation-green-{suffix}-executable-20261012'
        assert not dest.exists()
        shutil.copy2(path, dest)
        listed = subprocess.check_output([str(dest), '--list'], text=True)
        names = re.findall(r'^(.*): test$', listed, re.M)
        if suffix == 'scan':
            assert name in names
        pins[str(dest)] = sha(dest)
        executables.append({'suite': suffix, 'inventory': names, 'executable': str(dest)})
    drift = [p for p, h in source['file_hashes'].items() if sha(R / p) != h]
    out = D / 'dedup-visual-generation-verification-20261012.json'
    assert not out.exists()
    out.write_text(json.dumps({
        'status': 'verified_public_visual_generation_cancellation_fix_and_scoped_suites',
        'passed': sum(int(p) for p, _ in summaries),
        'ignored': sum(int(i) for _, i in summaries),
        'summaries_in_log_order': summaries, 'executables': executables, 'pins': pins,
        'root_dependency_drift_after_green_snapshot': drift,
        'unrelated_delta_between_red_and_green_snapshots': [p for p in source['deltas_from_initial_red'] if p.startswith('vendor/')],
        'scope': 'Actual macOS ARM64 decode-feature library, exact-file and visual scan tests. Source fix reconstructed exactly; saved executables/logs. Concurrent unrelated hayro source changed between snapshots, so no pure whole-dependency before/after attribution or full current workspace qualification. No physical race, Windows/Linux, all458/all500/fullformats/total RSS claim. Initial zero-cache setup failures preserved separately and excluded from defect evidence.',
    }, indent=2) + '\n')
    print('Verified', summaries, 'root drift', drift)

if __name__ == '__main__':
    main()
