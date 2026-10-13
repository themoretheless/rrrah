#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,subprocess,ast
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');F=T/'rrrah-gradient-runner-up-v2-experiment-20261014';assert not F.exists();F.mkdir();P=T/'rrrah-gradient-cache-root-before-20261013/gradient.rs';s=P.read_text();a=s.index('fn distances_four(');b=s.index('\nfn match_gradients_selected(',a);c=s.index('\n/// Describe a rotated',b);dist=s[a:b];matcher=s[b:c];baseline=dist.replace('distances_four','baseline_distances')+matcher.replace('match_gradients_selected','baseline').replace('distances_four','baseline_distances');candidate_dist=''
candidate_match=matcher.replace('match_gradients_selected','candidate').replace('distances_four','baseline_distances')
setup="""let mut reverse = allocate(right.len())?;
 let mut forward_runner=vec![usize::MAX;left.len()];let mut reverse_runner=vec![usize::MAX;right.len()];
 let mut forward_resolved=vec![false;left.len()];let mut reverse_resolved=vec![false;right.len()];
"""
candidate_match=candidate_match.replace('let mut reverse = allocate(right.len())?;',setup)
a=candidate_match.index('    let update =');b=candidate_match.index('    for (i, a)',a)
candidate_match=candidate_match[:a]+"""    let update=|best:&mut(f64,f64,usize),runner:&mut usize,distance:f64,index:usize|{
 if distance<best.0{*runner=best.2;*best=(distance,best.0,index);}else if distance<best.1{best.1=distance;*runner=index;}
 };
"""+candidate_match[b:]
candidate_match=candidate_match.replace('update(&mut forward[i], distance, j);','update(&mut forward[i], &mut forward_runner[i],distance, j);').replace('update(&mut reverse[j], distance, i);','update(&mut reverse[j], &mut reverse_runner[j],distance, i);')
a=candidate_match.index('        for best in forward.iter_mut().chain(reverse.iter_mut()) {');b=candidate_match.index('        for (i, a)',a)
candidate_match=candidate_match[:a]+"""        for i in 0..forward.len(){let runner=forward_runner[i];let best=&mut forward[i];if runner!=usize::MAX{let a=right[best.2].position;let b=right[runner].position;forward_resolved[i]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !forward_resolved[i]{best.1=f64::INFINITY;}}
        for j in 0..reverse.len(){let runner=reverse_runner[j];let best=&mut reverse[j];if runner!=usize::MAX{let a=left[best.2].position;let b=left[runner].position;reverse_resolved[j]=(a[0]-b[0]).powi(2)+(a[1]-b[1]).powi(2)>radius;}if !reverse_resolved[j]{best.1=f64::INFINITY;}}
"""+candidate_match[b:]
a=candidate_match.index('    if let Some(radius) = distinct_radius_squared {')
t=candidate_match[a:]
t=t.replace('let distances = baseline_distances(&a.descriptor, chunk);','if forward_resolved[i] && (block*4..block*4+chunk.len()).all(|j|reverse_resolved[j]){continue;}\n                let distances = baseline_distances(&a.descriptor, chunk);')
t=t.replace('if (b.position[0] - nearest[0])','if !forward_resolved[i] && (b.position[0] - nearest[0])')
# Parenthesize full geometric sum before resolved boolean conjunction.
t=t.replace('if !forward_resolved[i] && (b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius {','if !forward_resolved[i] && ((b.position[0] - nearest[0]).powi(2) + (b.position[1] - nearest[1]).powi(2) > radius) {')
t=t.replace('if (a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius {','if !reverse_resolved[j] && ((a.position[0] - nearest[0]).powi(2) + (a.position[1] - nearest[1]).powi(2) > radius) {')
candidate_match=candidate_match[:a]+t

# If no runner proves a distinct competitor, use the unchanged second pass.
a=candidate_match.index('        for (i, a)',candidate_match.index('    if let Some(radius) = distinct_radius_squared {'))
b=candidate_match.index('    let mut output',a)
optimized=candidate_match[a:b]
assert optimized.endswith('    }\n')
optimized=optimized[:-6]
x=matcher.index('        for (i, a)',matcher.index('    if let Some(radius) = distinct_radius_squared {'))
y=matcher.index('    let mut output',x)
original=matcher[x:y].replace('distances_four','baseline_distances')
assert original.endswith('    }\n')
original=original[:-6]
candidate_match=candidate_match[:a]+'        if forward_resolved.iter().chain(&reverse_resolved).any(|&v|v) {\n'+optimized+'        } else {\n'+original+'        }\n    }\n'+candidate_match[b:]

def traced(text,name):
 text=text.replace('fn match_gradients_selected(', 'fn '+name+'(').replace('fn candidate(', 'fn '+name+'(').replace('distances_four(', 'baseline_distances(')
 text=text.replace('Result<Vec<crate::geometry::Correspondence>, LocalError>', 'Result<(Vec<crate::geometry::Correspondence>, Vec<(u64,u64,usize)>, Vec<(u64,u64,usize)>), LocalError>')
 text=text.replace('return Ok(Vec::new());','return Ok((Vec::new(),Vec::new(),Vec::new()));')
 return text.replace('Ok(output)', 'Ok((output,forward.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect(),reverse.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect()))')
script=R/'scripts/experiment-dedup-gradient-distance-bounds-trace-v2-20261013.py';tree=ast.parse(script.read_text());literals={node.targets[0].id:ast.literal_eval(node.value) for node in tree.body if isinstance(node,ast.Assign) and len(node.targets)==1 and isinstance(node.targets[0],ast.Name) and node.targets[0].id in ['header','main']};src=F/'experiment.rs';src.write_text(literals['header']+baseline+candidate_dist+candidate_match+traced(matcher,'baseline_trace')+traced(candidate_match,'candidate_trace')+literals['main']);exe=F/'experiment';build=subprocess.run(['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)],capture_output=True,text=True);(F/'build.log').write_text(build.stdout+build.stderr);assert build.returncode==0,build.stderr;run=subprocess.run([str(exe)],capture_output=True,text=True);(F/'results.jsonl').write_text(run.stdout);(F/'stderr.log').write_text(run.stderr);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();O=R/'docs/research/dedup-gradient-runner-up-v2-experiment-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='experiment_passed_pending_analysis' if run.returncode==0 else 'experiment_failed',returncode=run.returncode,pins={str(p):sha(p) for p in [P,Path(__file__),script,src,exe,F/'build.log',F/'results.jsonl',F/'stderr.log']},scope='Exact global runner-up outside exclusion radius proves distinct second distance; unresolved directions recompute. Standalone prototype adds O(n+m) runner indices and flags, preserves checkpoints and ordered f64. No production adoption or managed admission. Synthetic proof only until natural controls.'),indent=2)+'\n');print(O,run.returncode)
