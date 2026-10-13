#!/usr/bin/env python3
"""Close the bounded root-run projected comparison regression gate."""
import hashlib
import json
from pathlib import Path
import re
import shutil

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / 'docs/research/dedup-projected-compare-audit-20261011.json'
LOG = ROOT / 'docs/research/dedup-projected-compare-gate-20261011.log'
MANIFEST = ROOT / 'docs/research/dedup-projected-compare-source-hashes-20261011.json'
ARCHIVE = Path('/Users/themoretheless/.codex/tmp/rrrah-projected-compare-qualified-20261011')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    log = LOG.read_text()
    counts = [int(x) for x in re.findall(r'test result: ok\. (\d+) passed;', log)]
    assert counts == [72, 7, 11, 9, 8, 6, 9, 10, 6, 8], counts
    sources = json.loads(MANIFEST.read_text())['file_hashes']
    artifacts = []
    for relative, expected in sources.items():
        source = ROOT / relative
        assert sha(source) == expected, relative
        target = ARCHIVE / 'sources' / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
        assert sha(target) == expected
    executables = re.findall(r'Running [^\n]*\((target/debug/deps/[^)]+)\)', log)
    assert len(executables) == 10, executables
    for relative in executables:
        source = ROOT / relative
        target = ARCHIVE / 'executables' / source.name
        target.parent.mkdir(parents=True, exist_ok=True)
        expected = sha(source)
        shutil.copy2(source, target)
        assert sha(target) == expected
        artifacts.append({'source': relative, 'archive': str(target), 'sha256': expected})
    for source in [LOG, MANIFEST]:
        target = ARCHIVE / source.name
        shutil.copy2(source, target)
        assert sha(source) == sha(target)
        artifacts.append({'source': str(source.relative_to(ROOT)), 'archive': str(target), 'sha256': sha(target)})
    report = {
        'status': 'complete_bounded_macos_projected_model_original_rgba_gate',
        'tests': sum(counts), 'suite_counts': counts, 'source_file_count': len(sources),
        'source_manifest': str(MANIFEST.relative_to(ROOT)), 'source_archive': str(ARCHIVE / 'sources'),
        'artifacts': artifacts,
        'execution_scope': 'Root run, not frozen execution; current dedup sources rechecked against hashes recorded during gate.',
        'implemented': ['Explicit provider-to-geometry-to-original-RGBA full-file API',
                        'Each source decoded once and original owners retained through confirmation',
                        'Both-direction coverage and agreement required; projected bytes are candidate-only',
                        'Source/dependency/weights observations and both generations remain live',
                        'Pixel work preflight before provider; managed original/input/proposal ownership'],
        'controls': ['HDR copy accepts; HDR collision with identical projected bytes rejects',
                     'Transparent black collision rejects through original alpha',
                     'Source/weights mutation in provider and final caller checkpoint rejects',
                     'First/middle/final caller cancellation and both request generations discard owners',
                     'One-short pixel work, invalid thresholds and tiny memory refuse'],
        'remaining': ['Native LoFTR projected wrapper and real HDR/alpha corpus',
                      'Automatic search portfolio integration', 'Total inference tensor/workspace RSS admission',
                      'Linux/Windows execution of this new API', 'Full negative-corpus precision and full 229 run'],
    }
    REPORT.write_text(json.dumps(report, indent=2) + '\n')
    print(report['status'], report['tests'])

if __name__ == '__main__':
    main()
