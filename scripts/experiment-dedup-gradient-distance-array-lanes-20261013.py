#!/usr/bin/env python3
from pathlib import Path
import hashlib,json,subprocess,ast
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');F=T/'rrrah-gradient-array-lanes-experiment-20261013';assert not F.exists();F.mkdir();P=R/'crates/rrrah-dedup/src/gradient.rs';s=P.read_text();a=s.index('fn distances_four(');b=s.index('\nfn match_gradients_selected(',a);c=s.index('\n/// Describe a rotated',b);dist=s[a:b];matcher=s[b:c];baseline=dist.replace('distances_four','baseline_distances')+matcher.replace('match_gradients_selected','baseline').replace('distances_four','baseline_distances');candidate_dist=dist.replace('distances_four','array_distances');a=candidate_dist.index('    let last = 3;');candidate_dist=candidate_dist[:a]+'''    let rows = [&b[0].descriptor.0, &b[1].descriptor.0, &b[2].descriptor.0, &b[3].descriptor.0];
    let mut sums = [0.; 4];
    for k in 0..128 {
        let x = a.0[k];
        let delta = [x - rows[0][k], x - rows[1][k], x - rows[2][k], x - rows[3][k]];
        for lane in 0..4 {
            sums[lane] += delta[lane] * delta[lane];
        }
    }
    sums
}
''';candidate_match=matcher.replace('match_gradients_selected','candidate').replace('distances_four','array_distances')
def traced(text,name):
 text=text.replace('fn match_gradients_selected(', 'fn '+name+'(').replace('fn candidate(', 'fn '+name+'(').replace('distances_four(', 'baseline_distances(')
 text=text.replace('Result<Vec<crate::geometry::Correspondence>, LocalError>', 'Result<(Vec<crate::geometry::Correspondence>, Vec<(u64,u64,usize)>, Vec<(u64,u64,usize)>), LocalError>')
 text=text.replace('return Ok(Vec::new());','return Ok((Vec::new(),Vec::new(),Vec::new()));')
 return text.replace('Ok(output)', 'Ok((output,forward.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect(),reverse.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect()))')
script=R/'scripts/experiment-dedup-gradient-distance-bounds-trace-v2-20261013.py';tree=ast.parse(script.read_text());literals={node.targets[0].id:ast.literal_eval(node.value) for node in tree.body if isinstance(node,ast.Assign) and len(node.targets)==1 and isinstance(node.targets[0],ast.Name) and node.targets[0].id in ['header','main']};src=F/'experiment.rs';src.write_text(literals['header']+baseline+candidate_dist+candidate_match+traced(matcher,'baseline_trace')+traced(candidate_match,'candidate_trace')+literals['main']);exe=F/'experiment';build=subprocess.run(['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)],capture_output=True,text=True);(F/'build.log').write_text(build.stdout+build.stderr);assert build.returncode==0,build.stderr;run=subprocess.run([str(exe)],capture_output=True,text=True);(F/'results.jsonl').write_text(run.stdout);(F/'stderr.log').write_text(run.stderr);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();O=R/'docs/research/dedup-gradient-array-lanes-experiment-20261013.json';assert not O.exists();O.write_text(json.dumps(dict(status='experiment_passed_pending_analysis' if run.returncode==0 else 'experiment_failed',returncode=run.returncode,pins={str(p):sha(p) for p in [P,Path(__file__),script,src,exe,F/'build.log',F/'results.jsonl',F/'stderr.log']},scope='Safe four-lane accumulator array layout, same per-descriptor addition order, no pruning/new allocations/unsafe/dependency. Exact baseline and candidate nearest-distance tables compared bitwise, callback counts/cancellation/error/correspondences matched on164synthetic fixtures. Standalone optimized host experiment; production source unchanged and natural/whole-operation speedup unproven.'),indent=2)+'\n');print(O,run.returncode)
