#!/usr/bin/env python3
"""Run a stage-RSS-only diagnostic on the fixed worst frozen negative pair."""
import hashlib
import json
import os
import re
import shutil
import subprocess
import time
from pathlib import Path

R = Path(__file__).resolve().parents[1]
D = R / 'docs/research'
T = Path('/Users/themoretheless/.codex/tmp')

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

def main():
    mp = D / 'dedup-disc21-heldout-negative-source-20261012.json'
    m = json.loads(mp.read_text())
    base = Path(m['snapshot'])
    source = T / 'rrrah-loftr-negative-rss-source-20261012'
    harness = T / 'rrrah-loftr-negative-rss-harness-20261012'
    report = D / 'dedup-loftr-negative-rss-profile-20261012.json'
    assert not any(p.exists() for p in [source, harness, report])
    for n, h in m['file_hashes'].items():
        assert sha(base / n) == h
        q = source / n
        q.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(base / n, q)
    path = source / 'vendor/loftr/src/loftr_model.rs'
    code = path.read_text()
    replacements = [
        ('        self.var_store.load(path).map_err(LoftrError::from)',
         '        rrrah_rss_trace("weights.before", &[]);\n'
         '        let result = self.var_store.load(path).map_err(LoftrError::from);\n'
         '        rrrah_rss_trace("weights.after", &[]);\n        result'),
        ('        let batch_size = image0.size()[0];',
         '        rrrah_rss_trace("input", &[image0.size(), image1.size()]);\n        let batch_size = image0.size()[0];'),
        ('        let coarse_features = self.coarse_features(&backbone_features, batch_size, cancel)?;',
         '        rrrah_rss_trace("backbone", &[backbone_features.fine0.size(), backbone_features.fine1.size()]);\n'
         '        let coarse_features = self.coarse_features(&backbone_features, batch_size, cancel)?;\n'
         '        rrrah_rss_trace("coarse.features", &[coarse_features.transformed0.size(), coarse_features.transformed1.size()]);'),
        ('        // Debug statistics own only scalars;',
         '        rrrah_rss_trace("coarse.matches", &[coarse.conf_matrix.size(), coarse.mconf.size()]);\n        // Debug statistics own only scalars;'),
        ('        drop(coarse.conf_matrix);',
         '        drop(coarse.conf_matrix);\n        rrrah_rss_trace("coarse.released", &[]);'),
        ('        let hw0_f = coarse_features.hw0_f;',
         '        rrrah_rss_trace("fine.windows", &[feat_f0_unfold.size(), feat_f1_unfold.size()]);\n        let hw0_f = coarse_features.hw0_f;'),
        ('        drop(coarse_features);',
         '        drop(coarse_features);\n        rrrah_rss_trace("features.released", &[]);'),
        ('        let fine = FineMatching::forward(',
         '        rrrah_rss_trace("fine.transform", &[feat_f0_unfold.size(), feat_f1_unfold.size()]);\n        let fine = FineMatching::forward('),
        ('        Ok(ForwardOutput {',
         '        rrrah_rss_trace("fine.matches", &[fine.mkpts0_f.size(), fine.mkpts1_f.size()]);\n        Ok(ForwardOutput {'),
    ]
    for old, new in replacements:
        assert code.count(old) == 1, old
        code = code.replace(old, new)
    helper = '''
// Diagnostic-only: external ps reads current resident size; no tensor arithmetic.
fn rrrah_rss_trace(stage: &str, shapes: &[Vec<i64>]) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CALL: AtomicUsize = AtomicUsize::new(0);
    if stage == "input" { CALL.fetch_add(1, Ordering::Relaxed); }
    let result = std::process::Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output().expect("RSS diagnostic ps");
    assert!(result.status.success(), "RSS diagnostic ps failed");
    let kib: u64 = String::from_utf8(result.stdout).expect("RSS UTF8")
        .trim().parse().expect("RSS integer");
    eprintln!("RRRAH_RSS call={} stage={} rss_bytes={} shapes={:?}",
        CALL.load(Ordering::Relaxed), stage, kib * 1024, shapes);
}
'''
    path.write_text(code + helper)
    old = T / 'rrrah-disc21-heldout-negative-harness-20261012'
    harness.mkdir()
    (harness / 'Cargo.toml').write_text((old / 'Cargo.toml').read_text().replace(str(base), str(source)))
    shutil.copy2(old / 'Cargo.lock', harness / 'Cargo.lock')
    probe = harness / 'src/bin/disc21_heldout_negative_collection.rs'
    probe.parent.mkdir(parents=True)
    shutil.copy2(old / 'src/bin/disc21_heldout_negative_collection.rs', probe)
    hashes = {n: sha(source / n) for n in m['file_hashes']}
    assert [n for n in hashes if hashes[n] != m['file_hashes'][n]] == ['vendor/loftr/src/loftr_model.rs']
    runtime_report = D / 'dedup-disc21-heldout-negatives-runtime-20261012.json'
    original = json.loads(runtime_report.read_text())
    original_row = original['results'][7]
    assert original_row['query'] == 'Q00992' and original_row['reference'] == 'R043984'
    pins = {str(p): sha(p) for p in [mp, runtime_report, Path(__file__), probe, harness / 'Cargo.toml', harness / 'Cargo.lock']}
    pins.update({str(source / n): h for n, h in hashes.items()})
    result = {'status': 'building', 'producer_pid': os.getpid(), 'pins': pins,
              'source': str(source), 'production_delta': ['vendor/loftr/src/loftr_model.rs'],
              'instrumentation_replacements': replacements, 'instrumentation_helper': helper,
              'original_pair_index': 7,
              'scope': 'Diagnostic-only stage RSS on the original worst frozen negative pair; unchanged model arithmetic, caller policy, all68 primary/recovery bidirectional attempts and historical dependency composition. ps samples are phase-boundary current RSS, not internal-kernel peaks or payload attribution. No root adoption, timing benchmark, hard memory cap or complete current-stack/corpus qualification.'}
    def save():
        tmp = report.with_suffix('.tmp')
        tmp.write_text(json.dumps(result, indent=2) + '\n')
        tmp.replace(report)
    save()
    env = dict(os.environ, LIBTORCH_USE_PYTORCH='1')
    env['PATH'] = str(T / 'rrrah-loftr-native-runtime-20261010/bin') + ':' + env['PATH']
    cmd = ['cargo', 'build', '--manifest-path', str(harness / 'Cargo.toml'), '--bin',
           'disc21_heldout_negative_collection', '--locked', '--offline', '--target-dir',
           str(T / 'rrrah-dedup-exact-portfolio-20261011')]
    log = T / 'rrrah-loftr-negative-rss-build-20261012.log'
    with log.open('w') as stream:
        child = subprocess.Popen(cmd, cwd='/tmp', env=env, stdout=stream, stderr=subprocess.STDOUT)
        result.update(build_command=cmd, build_pid=child.pid)
        save()
        rc = child.wait()
    result.update(build_returncode=rc, build_log=str(log))
    pins[str(log)] = sha(log)
    save()
    if rc:
        result['status'] = 'build_failed'; save(); return
    binary = T / 'rrrah-loftr-negative-rss-executable-20261012'
    assert not binary.exists()
    shutil.copy2(T / 'rrrah-dedup-exact-portfolio-20261011/debug/disc21_heldout_negative_collection', binary)
    pins[str(binary)] = sha(binary)
    cmd = original_row['command'].copy()
    cmd[4] = str(binary)
    stdout = T / 'rrrah-loftr-negative-rss-20261012.stdout'
    stderr = T / 'rrrah-loftr-negative-rss-20261012.stderr'
    with stdout.open('w') as out, stderr.open('w') as err:
        start = time.monotonic()
        child = subprocess.Popen(cmd, cwd='/tmp', stdout=out, stderr=err)
        result.update(status='running', command=cmd, child_pid=child.pid,
                      stdout=str(stdout), stderr=str(stderr))
        save()
        rc = child.wait()
    result.update(returncode=rc, elapsed_seconds=time.monotonic()-start)
    pins.update({str(p):sha(p) for p in [stdout, stderr]})
    if rc:
        result['status'] = 'runtime_failed'; save(); return
    evidence = json.loads(stdout.read_text())
    assert evidence == original_row['evidence'], 'instrumented evidence differs; preserve report'
    trace = re.findall(r'RRRAH_RSS call=(\d+) stage=(\S+) rss_bytes=(\d+) shapes=(.*)', stderr.read_text())
    assert len(trace) > 0
    result.update(status='complete_identical_evidence_stage_rss_diagnostic',
                  evidence=evidence, rss_peak_bytes=int(re.search(r'(\d+)\s+maximum resident set size', stderr.read_text())[1]),
                  trace=[dict(call=int(c), stage=s, rss_bytes=int(r), shapes=json.loads(shape)) for c,s,r,shape in trace])
    assert all(sha(p)==h for p,h in pins.items())
    save()
    print('Completed unchanged-evidence stage RSS diagnostic', flush=True)

if __name__ == '__main__':
    main()
