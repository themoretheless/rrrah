#!/usr/bin/env python3
from pathlib import Path
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parents[1]
P = ROOT / 'crates/rrrah-dedup/src/gradient.rs'
text = P.read_text()
start = text.index('fn distances_four(')
end = text.index('\nfn match_gradients_selected(', start)
original = text[start:end]
loop_start = original.index('    for k in 0..128 {')
loop_end = original.index('    [s0, s1, s2, s3]', loop_start)
body = original[loop_start:loop_end]
inner = body[body.index('\n') + 1:body.rfind('    }')]
unrolled = '    for base in (0..128).step_by(4) {\n'
for offset in range(4):
    unrolled += '        { let k = base + ' + str(offset) + ';\n' + inner + '        }\n'
unrolled += '    }\n'
candidate = original[:loop_start] + unrolled + original[loop_end:]
candidate = candidate.replace('fn distances_four(', 'fn candidate(')
F = Path('/private/tmp/rrrah-gradient-unroll4-20261014')
F.mkdir(exist_ok=False)
header = '''#[derive(Clone,Copy)] struct GradientDescriptor([f64;128]);
#[derive(Clone,Copy)] struct GradientFeature { position:[f64;2], descriptor:GradientDescriptor }
'''
driver = r'''
fn next(seed:&mut u64)->f64 { *seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1); ((*seed>>11) as f64)/((1u64<<53) as f64) }
fn feature(seed:&mut u64)->GradientFeature {
    let mut values=std::array::from_fn(|_| next(seed));
    let norm=values.iter().map(|v|v*v).sum::<f64>().sqrt();
    for value in &mut values { *value/=norm; }
    GradientFeature {position:[0.;2],descriptor:GradientDescriptor(values)}
}
fn run(left:&[GradientFeature],right:&[GradientFeature],f:fn(&GradientDescriptor,&[GradientFeature])->[f64;4])->(u128,u64) {
    let start=std::time::Instant::now();let mut checksum=0u64;
    for a in left {for chunk in right.chunks(4) {for value in f(std::hint::black_box(&a.descriptor),std::hint::black_box(chunk)) {checksum=checksum.wrapping_add(value.to_bits());}}}
    (start.elapsed().as_nanos(),checksum)
}
fn main() {
    let mut seed=173;let left:Vec<_>=(0..256).map(|_|feature(&mut seed)).collect();
    let right:Vec<_>=(0..1024).map(|_|feature(&mut seed)).collect();let mut checked=0;
    for a in &left {for length in 0..=4 {for offset in (0..1020).step_by(17) {
        let chunk=&right[offset..offset+length];
        assert_eq!(distances_four(&a.descriptor,chunk).map(f64::to_bits),candidate(&a.descriptor,chunk).map(f64::to_bits));checked+=1;
    }}}
    println!("checked {}",checked);
    for pair in 0..10 {
        let order=if pair%2==0 {[false,true]} else {[true,false]};
        for changed in order {let (n,sum)=run(&left,&right,if changed {candidate}else{distances_four});println!("{} {} {} {}",pair,changed,n,sum);}
    }
}
'''
source = F / 'experiment.rs'
source.write_text('#![allow(dead_code)]\n' + header + '#[inline(never)]\n' + original + '#[inline(never)]\n' + candidate + driver)
binary = F / 'experiment'
build = subprocess.run(['rustc', '+stable', '--edition', '2024', '-C', 'opt-level=3', str(source), '-o', str(binary)], capture_output=True, text=True)
(F / 'build.log').write_text(build.stdout + build.stderr)
assert build.returncode == 0, build.stderr
run = subprocess.run([str(binary)], capture_output=True, text=True)
(F / 'results.txt').write_text(run.stdout)
(F / 'stderr.log').write_text(run.stderr)
assert run.returncode == 0, run.stderr
assert P.read_text() == text
lines = run.stdout.splitlines()
assert lines[0].startswith('checked ')
rows = [line.split() for line in lines[1:]]
assert len(rows) == 20 and len({r[3] for r in rows}) == 1
ratios = []
for pair in range(1,10):
    values = {row[1]:int(row[2]) for row in rows if row[0] == str(pair)}
    ratios.append(values['true']/values['false'])
import statistics
OUT = ROOT / 'docs/research/dedup-gradient-unroll4-experiment-20261014.json'
assert not OUT.exists()
OUT.write_text(json.dumps(dict(status='synthetic_kernel_experiment_complete_no_adoption', bitwise_controls=int(lines[0].split()[1]), alternating_measured_pairs=9, ratios=ratios, median_ratio=statistics.median(ratios), source_sha256=hashlib.sha256(P.read_bytes()).hexdigest(), pins={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [source,binary,F/'build.log',F/'results.txt',F/'stderr.log']}, limits='Kernel only, normalized synthetic descriptors, actual descriptor width and position fields. Partial blocks0..4 compared bitwise. Both functions noinline. No nearest-selection/cancellation/corpus/managed matcher/whole-search qualification. Production unchanged.'),indent=2)+'\n')
print(json.dumps(dict(controls=int(lines[0].split()[1]),median_ratio=statistics.median(ratios),ratios=ratios)))
