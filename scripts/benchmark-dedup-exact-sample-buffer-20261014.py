#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
F = Path('/private/tmp/rrrah-exact-sample-benchmark-20261014')
F.mkdir(exist_ok=False)
source = (ROOT / 'crates/rrrah-dedup/src/exact.rs').read_bytes()
fixed = b'allocate_read_buffer(if sampled { SAMPLE } else { BLOCK })?'
assert source.count(fixed) == 1
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
command = json.loads((ROOT / 'docs/research/dedup-exact-sample-buffer-20261014/command.json').read_text())
deps = []
for i, arg in enumerate(command):
    if arg in ['--extern', '-L']:
        deps += [arg, command[i + 1]]
driver = '''#[path="exact.rs"] mod exact;
fn main() {
    let root=std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let started=std::time::Instant::now();
    let report=exact::scan(&[root],&exact::Options::default(),||false);
    let elapsed=started.elapsed().as_nanos();
    assert!(report.complete());
    assert_eq!(report.files_observed,1200);
    assert_eq!(report.groups.len(),100);
    for group in &report.groups {assert_eq!(group.paths.len(),3);}
    println!("{}",elapsed);
    println!("{:?}",report);
}
'''
for name in ['baseline', 'fixed']:
    directory = F / name
    directory.mkdir()
    (directory / 'exact.rs').write_bytes(source if name == 'fixed' else source.replace(fixed, b'allocate_read_buffer(BLOCK)?'))
    (directory / 'main.rs').write_text(driver)
    build = subprocess.run(['rustc', '+stable', '--edition', '2024', '-O', '-A', 'dead_code', str(directory / 'main.rs'), *deps, '-o', str(directory / 'probe')], capture_output=True, text=True)
    (directory / 'build.log').write_text(build.stdout + build.stderr)
    assert build.returncode == 0, build.stderr
corpus = F / 'corpus'
corpus.mkdir()
for index in range(1200):
    identity = index // 3 if index < 300 else index
    data = (hashlib.sha256(str(identity).encode()).digest() * 128)
    (corpus / f'{index:04}.bin').write_bytes(data)
rows = []
reference = None
for pair in range(10):
    order = ['baseline', 'fixed'] if pair % 2 == 0 else ['fixed', 'baseline']
    for name in order:
        wall = time.perf_counter_ns()
        run = subprocess.run([str(F / name / 'probe'), str(corpus)], capture_output=True, text=True)
        wall = time.perf_counter_ns() - wall
        assert run.returncode == 0, run.stderr
        elapsed, report = run.stdout.split('\n', 1)
        if reference is None:
            reference = report
        assert report == reference
        rows.append(dict(pair=pair, version=name, elapsed_ns=int(elapsed), process_wall_ns=wall, report_sha256=hashlib.sha256(report.encode()).hexdigest()))
ratios = []
for pair in range(1, 10):
    values = {row['version']: row['elapsed_ns'] for row in rows if row['pair'] == pair}
    ratios.append(values['fixed'] / values['baseline'])
assert source == (ROOT / 'crates/rrrah-dedup/src/exact.rs').read_bytes()
OUT = ROOT / 'docs/research/dedup-exact-sample-buffer-benchmark-20261014.json'
assert not OUT.exists()
OUT.write_text(json.dumps(dict(status='completed_identical_reports', corpus='1200 synthetic 4096-byte files: 100 three-copy groups and 900 unique contents; warm local filesystem', rows=rows, warmup_pairs=1, measured_alternating_pairs=9, fixed_over_baseline_ratios=ratios, median_ratio=statistics.median(ratios), source_sha256=hashlib.sha256(source).hexdigest(), pins={str(F / name / file):sha(F / name / file) for name in ['baseline','fixed'] for file in ['exact.rs','main.rs','probe','build.log']}, limits='One synthetic small-file corpus, shared host, warm filesystem and direct original exact module. Baseline differs by one buffer-size expression only. No million-file, cold IO, large-file, platform or whole-application throughput guarantee.'), indent=2) + '\n')
print(json.dumps(dict(median_ratio=statistics.median(ratios), ratios=ratios)))
