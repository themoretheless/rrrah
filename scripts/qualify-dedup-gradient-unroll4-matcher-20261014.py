#!/usr/bin/env python3
from pathlib import Path
import ast
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
P=Path('/Users/themoretheless/.codex/tmp/rrrah-gradient-cache-root-before-20261013/gradient.rs')
text=P.read_text();start=text.index('fn distances_four(');middle=text.index('\nfn match_gradients_selected(',start);end=text.index('\n/// Describe a rotated',middle)
distance=text[start:middle];matcher=text[middle:end]
candidate_source=Path('/private/tmp/rrrah-gradient-unroll4-natural-cpu-20261014/experiment.rs').read_text()
start=candidate_source.index('fn candidate(');end=candidate_source.index('\nfn next(',start)
candidate_distance=candidate_source[start:end].replace('fn candidate(', 'fn candidate_distances(')
baseline=distance.replace('distances_four','baseline_distances')+matcher.replace('match_gradients_selected','baseline').replace('distances_four','baseline_distances')
changed=matcher.replace('match_gradients_selected','candidate').replace('distances_four','candidate_distances')

def traced(source,name,kernel):
    source=source.replace('fn match_gradients_selected(', 'fn '+name+'(').replace('distances_four(',kernel+'(')
    source=source.replace('Result<Vec<crate::geometry::Correspondence>, LocalError>', 'Result<(Vec<crate::geometry::Correspondence>, Vec<(u64,u64,usize)>, Vec<(u64,u64,usize)>), LocalError>')
    source=source.replace('return Ok(Vec::new());','return Ok((Vec::new(),Vec::new(),Vec::new()));')
    return source.replace('Ok(output)','Ok((output,forward.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect(),reverse.iter().map(|&(a,b,i)|(a.to_bits(),b.to_bits(),i)).collect()))')

script=R/'scripts/experiment-dedup-gradient-distance-bounds-trace-v2-20261013.py'
values={node.targets[0].id:ast.literal_eval(node.value) for node in ast.parse(script.read_text()).body if isinstance(node,ast.Assign) and len(node.targets)==1 and isinstance(node.targets[0],ast.Name) and node.targets[0].id in ['header','main']}
main=values['main'];prefix=main[:main.index(' let args:Vec<_>')]
loader=main[main.index('fn read_dump('):main.index('fn run_natural(')]
tail=r'''
 let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),16);
 for (index,paths) in args.chunks_exact(2).enumerate() {
    let a=read_dump(&paths[0]);let b=read_dump(&paths[1]);
    for radius in [None,Some(4.)] {
        let count=Cell::new(0);
        let expected=baseline(&a,&b,policy,radius,||{count.set(count.get()+1);false}).unwrap();let calls=count.get();count.set(0);
        let output=candidate(&a,&b,policy,radius,||{count.set(count.get()+1);false}).unwrap();assert_eq!(expected,output);assert_eq!(calls,count.get());
        assert_eq!(baseline_trace(&a,&b,policy,radius,||false),candidate_trace(&a,&b,policy,radius,||false));
        for stop in [1,calls/2+1,calls] {
            let c=Cell::new(0);let before=baseline(&a,&b,policy,radius,||{c.set(c.get()+1);c.get()==stop});c.set(0);
            let after=candidate(&a,&b,policy,radius,||{c.set(c.get()+1);c.get()==stop});assert_eq!(before,after);
        }
        println!("natural {} {} {} {}",index,radius.is_some(),output.len(),calls);
    }
 }
}
'''
F=Path('/private/tmp/rrrah-gradient-unroll4-matcher-20261014');F.mkdir(exist_ok=False)
source=F/'experiment.rs';source.write_text(values['header']+baseline+candidate_distance+changed+traced(matcher,'baseline_trace','baseline_distances')+traced(matcher,'candidate_trace','candidate_distances')+prefix+tail+loader)
binary=F/'experiment';build=subprocess.run(['rustc','+stable','--edition=2024','-C','opt-level=3',str(source),'-o',str(binary)],capture_output=True,text=True);(F/'build.log').write_text(build.stdout+build.stderr);assert build.returncode==0,build.stderr
N=R/'docs/research/dedup-gradient-unroll4-natural-cpu-20261014.json';natural=json.loads(N.read_text());assert natural['status']=='completed_pending_audit_no_adoption'
assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest()==h for p,h in natural['feature_pins'].items())
run=subprocess.run([str(binary),*[p for case in natural['cases'] for p in case['inputs']]],capture_output=True,text=True)
(F/'results.txt').write_text(run.stdout);(F/'stderr.log').write_text(run.stderr);assert run.returncode==0,run.stderr
lines=run.stdout.splitlines();assert json.loads(lines[0])['fixtures']==164 and len(lines)==17
assert all(line.startswith('natural ') for line in lines[1:])
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
O=R/'docs/research/dedup-gradient-unroll4-matcher-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='verified_standalone_uncached_matcher_candidate_no_adoption',synthetic_fixtures=164,natural_directions=8,matching_modes=2,pins={str(p):sha(p) for p in [P,script,source,binary,F/'build.log',F/'results.txt',F/'stderr.log',N,Path(__file__)]},scope='Original pre-cache matcher source with only four-dimension kernel unroll substituted. Exact correspondence vectors and both nearest tables, callback counts and three cancellation stops per scenario agree. Natural retained first1024features. Managed cache/runner/current full crate/native search not executed in this control.'),indent=2)+'\n');print(O)
