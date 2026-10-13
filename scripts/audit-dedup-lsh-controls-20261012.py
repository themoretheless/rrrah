#!/usr/bin/env python3
"""Pin a partial LSH source review and scoped original/counterexample execution."""
import hashlib
import json
import subprocess
import tarfile
import xml.etree.ElementTree as ET
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research/dedup-500'
T = Path('/Users/themoretheless/.codex/tmp')
S = T / 'rrrah-lsh-source-20261012'
C = T / 'rrrah-lsh-regenerated-20261012'

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    archive = T / 'rrrah-lsh-source-20261012.tar.gz'
    hashes = {}
    with tarfile.open(archive) as tar:
        for member in tar.getmembers():
            if member.isdir():
                continue
            assert member.isfile()
            name = str(Path(*Path(member.name).parts[1:]))
            assert '..' not in Path(name).parts and not Path(name).is_absolute()
            hashes[name] = hashlib.sha256(tar.extractfile(member).read()).hexdigest()
    assert len(hashes) == 13
    assert all(sha(S / n) == h for n, h in hashes.items())
    original = json.loads((D / 'reviews/027.json').read_text())
    for row in original['available_source_evidence'] + original['available_readme_license_evidence']:
        if not row['truncated']:
            assert hashes[row['path']] == row['sha256']
    deltas = [n for n, h in hashes.items() if sha(C / n) != h]
    assert sorted(deltas) == ['lsh/cMinhash.cpp', 'setup.py']
    assert (C / 'setup.py').read_text() == (S / 'setup.py').read_text().replace('USE_CYTHON = False', 'USE_CYTHON = True')
    pins = {str(S / n): h for n, h in hashes.items()}
    logs = ['original-build', 'regenerated-build', 'regenerated-build-v2', 'upstream-tests', 'controls', 'runtime-install']
    for name in logs:
        path = T / f'rrrah-lsh-{name}-20261012.log'
        pins[str(path)] = sha(path)
    assert "fatal error: 'longintrepr.h' file not found" in (T / 'rrrah-lsh-original-build-20261012.log').read_text()
    counts = {}
    for label, expected in [('upstream-tests', 108), ('controls', 6)]:
        path = T / f'rrrah-lsh-{label}-20261012.xml'
        suites = ET.parse(path).getroot().findall('testsuite')
        assert len(suites) == 1
        suite = suites[0]
        assert int(suite.attrib['tests']) == expected
        assert all(int(suite.attrib[k]) == 0 for k in ['failures', 'errors', 'skipped'])
        counts[label] = [case.attrib['name'] for case in suite.findall('testcase')]
        assert len(counts[label]) == expected
        pins[str(path)] = sha(path)
    binaries = list((C / 'lsh').glob('cMinhash*.so'))
    assert len(binaries) == 1
    for path in [C / 'rrrah_controls_test.py', C / 'setup.py', C / 'lsh/cMinhash.cpp', binaries[0], archive, Path(__file__)]:
        pins[str(path)] = sha(path)
    py = T / 'rrrah-lsh-runtime-20261012/bin/python'
    versions = subprocess.check_output([str(py), '-c', 'import sys,numpy,Cython,pytest; print(sys.version); print(numpy.__version__, Cython.__version__, pytest.__version__)'], text=True)
    out = D / 'lsh-partial-source-runtime-review-20261012.json'
    assert not out.exists()
    out.write_text(json.dumps({
        'status': 'partial_source_review_with_six_runtime_counterexamples',
        'repository': original['repository'], 'commit': original['commit'],
        'source_file_hashes': hashes, 'runtime_versions': versions,
        'complete_text_reads': 11,
        'notebook_review': 'All35 cell sources, Markdown, code and textual outputs inspected; embedded plots not rendered or rerun. Historical Reuters results are not new measurements.',
        'generated_cpp_scope': 'Original21808-line file header and both MinHash algorithm bodies inspected, including signed32 and capped64 minima, pointer increment/reset and unchecked caller lengths. Remaining generated Cython runtime not comprehensively reviewed; full tree source review remains incomplete.',
        'license': 'Root MIT read; embedded MurmurHash source/header explicitly public domain, unlike README MIT description. No code adopted.',
        'build_scope': 'Original pregenerated Cython0.24.1 C++ fails on Python3.12 missing longintrepr.h. First regeneration-named attempt accidentally ran unchanged setup in the original-build copy and also failed; preserved. v2 runs USE_CYTHON=True in a separate copy, regenerating only C++ from unchanged pyx using modern Cython. Not a successful original generated-C++ build.',
        'upstream_tests': counts['upstream-tests'], 'counterexample_tests': counts['controls'],
        'findings': [
            'NumPy fingerprint jaccard returns1 for disjoint equal-length arrays due to bitwise operations and length ratio.',
            'Known candidate with signature-set overlap1/3 survives min_jaccard0.9.',
            'clear empties bins but retains fingerprint entries; later removal raises KeyError.',
            'Replacing an id leaves stale old buckets; removal leaves nonexistent ids in candidates and filtered lookup raises KeyError.',
            'Caller fingerprint mutation changes stored signature without updating bins; removal raises KeyError.',
            'Cached fingerprint return is mutable/shared; caller mutation persists on future same-document calls.',
        ],
        'scope': '108 upstream tests and6 independent controls on macOS ARM64, using regenerated extension and unchanged Python/pyx/MurmurHash source. Text retrieval domain, not photo identity. No full13-file/generated-runtime/all500 audit, other-platform/runtime, hard memory or broad accuracy qualification. No implementation adopted.',
        'pins': pins,
    }, indent=2) + '\n')
    print('Verified108 upstream +6 controls; generated runtime review remains incomplete')

if __name__ == '__main__':
    main()
