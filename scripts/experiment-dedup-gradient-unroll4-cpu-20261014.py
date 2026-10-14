#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import resource
import statistics
import subprocess
import time

R = Path(__file__).resolve().parents[1]
previous = R / 'docs/research/dedup-gradient-unroll4-experiment-20261014.json'
plan = json.loads(previous.read_text())
for path, pin in plan['pins'].items():
    assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == pin
original = Path('/private/tmp/rrrah-gradient-unroll4-20261014/experiment.rs').read_text()
start = original.index('fn main() {')
main = r'''
fn main() {
    let changed=std::env::args().nth(1).unwrap()=="candidate";
    let mut seed=173;let left:Vec<_>=(0..256).map(|_|feature(&mut seed)).collect();
    let right:Vec<_>=(0..1024).map(|_|feature(&mut seed)).collect();
    let mut sum=0u64;
    for _ in 0..8 {
        let (_,value)=run(&left,&right,if changed {candidate}else{distances_four});
        sum=sum.wrapping_add(value);
    }
    println!("{}",sum);
}
'''
F = Path('/private/tmp/rrrah-gradient-unroll4-cpu-20261014')
F.mkdir(exist_ok=False)
source = F / 'experiment.rs'
source.write_text(original[:start] + main)
binary = F / 'experiment'
build = subprocess.run(['rustc', '+stable', '--edition', '2024', '-C', 'opt-level=3', str(source), '-o', str(binary)], capture_output=True, text=True)
(F / 'build.log').write_text(build.stdout + build.stderr)
assert build.returncode == 0, build.stderr
rows = []
for pair in range(10):
    order = ['baseline', 'candidate'] if pair % 2 == 0 else ['candidate', 'baseline']
    for version in order:
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        start = time.perf_counter()
        run = subprocess.run([str(binary), version], capture_output=True, text=True)
        wall = time.perf_counter() - start
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        assert run.returncode == 0, run.stderr
        cpu = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
        assert cpu > 0
        rows.append(dict(pair=pair,version=version,cpu_seconds=cpu,wall_seconds=wall,checksum=run.stdout.strip()))
assert len({row['checksum'] for row in rows}) == 1
ratios = []
for pair in range(1,10):
    values = {row['version']:row['cpu_seconds'] for row in rows if row['pair'] == pair}
    ratios.append(values['candidate']/values['baseline'])
OUT = R / 'docs/research/dedup-gradient-unroll4-cpu-20261014.json'
assert not OUT.exists()
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
OUT.write_text(json.dumps(dict(status='cpu_measurement_complete_no_adoption', rows=rows, median_cpu_ratio=statistics.median(ratios), cpu_ratios=ratios, measured_pairs=9, warmup_pairs=1, repetitions_per_process=8, prior_bitwise_controls=76800, prior_experiment=str(previous), prior_experiment_sha256=sha(previous), pins={str(p):sha(p) for p in [source,binary,F/'build.log',Path(__file__)]}, scope='Fresh single-thread processes, synthetic normalized descriptors, exact checksums. CPU from per-child getrusage deltas, includes process initialization and teardown. No natural-bank, matcher/cancellation or whole-search speedup claim.'),indent=2)+'\n')
print(json.dumps(dict(median_cpu_ratio=statistics.median(ratios),ratios=ratios)))
