#!/usr/bin/env python3
"""Standalone exact top-K distinct competitor prototype; no production adoption."""
from pathlib import Path
import ast
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parents[1]
FOLDER = Path('/private/tmp/rrrah-gradient-topk-experiment-20261014')
SOURCE = Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-cache-root-before-20261013/gradient.rs')
TEMPLATE = ROOT / 'scripts/experiment-dedup-gradient-distance-bounds-trace-v2-20261013.py'
assert not FOLDER.exists()
FOLDER.mkdir()
source = SOURCE.read_text()
a = source.index('fn distances_four(')
b = source.index('\nfn match_gradients_selected(', a)
c = source.index('\n/// Describe a rotated', b)
distance = source[a:b].replace('distances_four', 'baseline_distances')
matcher = source[b:c]
baseline = matcher.replace('match_gradients_selected', 'baseline').replace('distances_four', 'baseline_distances')
candidate = baseline.replace('fn baseline(', 'fn candidate(')
needle = '    let mut reverse = allocate(right.len())?;'
assert candidate.count(needle) == 1
candidate = candidate.replace(needle, needle + '''
    const K: usize = 16;
    let mut forward_top = vec![[(f64::INFINITY, usize::MAX); K]; left.len()];
    let mut reverse_top = vec![[(f64::INFINITY, usize::MAX); K]; right.len()];
    let mut forward_resolved = vec![false; left.len()];
    let mut reverse_resolved = vec![false; right.len()];
    let insert = |top: &mut [(f64,usize); K], distance: f64, index: usize| {
        if distance >= top[K-1].0 { return; }
        let mut slot = K-1;
        while slot > 0 && distance < top[slot-1].0 {
            top[slot] = top[slot-1];
            slot -= 1;
        }
        top[slot] = (distance,index);
    };
''')
for which, row, index in [('forward','i','j'), ('reverse','j','i')]:
    needle = f'update(&mut {which}[{row}], distance, {index});'
    assert candidate.count(needle) == 1
    candidate = candidate.replace(needle, needle + f'\n                if distinct_radius_squared.is_some() {{ insert(&mut {which}_top[{row}], distance, {index}); }}')
a = candidate.index('        for best in forward.iter_mut().chain(reverse.iter_mut()) {')
b = candidate.index('        for (i, a)', a)
resolve = ''
for which, other in [('forward','right'),('reverse','left')]:
    resolve += f'''
        for row in 0..{which}.len() {{
            let best = &mut {which}[row];
            best.1 = f64::INFINITY;
            let nearest = {other}[best.2].position;
            for &(distance,index) in &{which}_top[row] {{
                if index == usize::MAX {{ break; }}
                let p = {other}[index].position;
                if (p[0]-nearest[0]).powi(2)+(p[1]-nearest[1]).powi(2)>radius {{
                    best.1 = distance;
                    {which}_resolved[row] = true;
                    break;
                }}
            }}
        }}
'''
candidate = candidate[:a] + resolve + candidate[b:]
a = candidate.index('    if let Some(radius) = distinct_radius_squared {')
tail = candidate[a:]
needle = '                let distances = baseline_distances(&a.descriptor, chunk);'
assert tail.count(needle) == 1
tail = tail.replace(needle, '''                if forward_resolved[i] && (block*4..block*4+chunk.len()).all(|j|reverse_resolved[j]) { continue; }
''' + needle)
for which, row, feature in [('forward','i','b'),('reverse','j','a')]:
    needle = f'if ({feature}.position[0] - nearest[0]).powi(2) + ({feature}.position[1] - nearest[1]).powi(2) > radius {{'
    assert tail.count(needle) == 1
    tail = tail.replace(needle, f'if !{which}_resolved[{row}] && (({feature}.position[0] - nearest[0]).powi(2) + ({feature}.position[1] - nearest[1]).powi(2) > radius) {{')
candidate = candidate[:a] + tail

def traced(text, old, new):
    text = text.replace('fn ' + old + '(', 'fn ' + new + '(')
    text = text.replace('Result<Vec<crate::geometry::Correspondence>, LocalError>', 'Result<(Vec<crate::geometry::Correspondence>,Vec<(u64,u64,usize)>,Vec<(u64,u64,usize)>),LocalError>')
    text = text.replace('return Ok(Vec::new());', 'return Ok((Vec::new(),Vec::new(),Vec::new()));')
    return text.replace('Ok(output)', 'Ok((output,forward.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect(),reverse.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect()))')

literals = {n.targets[0].id: ast.literal_eval(n.value) for n in ast.parse(TEMPLATE.read_text()).body if isinstance(n,ast.Assign) and len(n.targets)==1 and isinstance(n.targets[0],ast.Name) and n.targets[0].id in ['header','main']}
# Correctness-only first gate; timings require a separately pinned CPU experiment.
main = literals['main'].replace(' let args:Vec<_>', ' return;\n let args:Vec<_>')
src = FOLDER / 'experiment.rs'
src.write_text(literals['header'] + distance + baseline + candidate + traced(baseline,'baseline','baseline_trace') + traced(candidate,'candidate','candidate_trace') + main)
exe = FOLDER / 'experiment'
command = ['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)]
build = subprocess.run(command,capture_output=True,text=True)
(FOLDER/'build.log').write_text(build.stdout+build.stderr)
assert build.returncode==0, build.stderr
run = subprocess.run([str(exe)],capture_output=True,text=True)
(FOLDER/'results.jsonl').write_text(run.stdout)
(FOLDER/'stderr.log').write_text(run.stderr)
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
report = ROOT / 'docs/research/dedup-gradient-topk-experiment-20261014.json'
assert not report.exists()
report.write_text(json.dumps(dict(status='synthetic_correctness_passed' if run.returncode==0 else 'failed',returncode=run.returncode,command=command,pins={str(p):sha(p) for p in [SOURCE,TEMPLATE,Path(__file__),src,exe,FOLDER/'build.log',FOLDER/'results.jsonl',FOLDER/'stderr.log']},scope='Standalone sorted exact top16 competitor retention with original second-pass fallback and cancellation checkpoints. Original pre-cache matcher as reference. Exact nearest table bits and correspondence/error/cancellation parity on164 synthetic cases. No managed allocation admission, performance proof, natural corpus, production adoption or whole-operation qualification.'),indent=2)+'\n')
print(report,run.returncode,run.stdout,run.stderr)
