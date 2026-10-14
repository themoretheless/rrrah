#!/usr/bin/env python3
"""Count needed second-pass lanes on genuine banks; no timing claims."""
from pathlib import Path
import hashlib,json,subprocess,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
fixture_path=D/'dedup-natural-large-gradient-banks-v2-20261014.json';fixture=json.loads(fixture_path.read_text());assert fixture['status']=='complete_pending_audit'
features=[x['features']for x in fixture['extractions']];assert all(sha(x['features'])==x['features_sha256']for x in fixture['extractions'])
original=Path('/private/tmp/rrrah-gradient-masked-lanes-stress-20261014/stress.rs');original_audit=D/'dedup-gradient-masked-lanes-stress-20261014.json';original_report=json.loads(original_audit.read_text());assert sha(original)==original_report['pins'][str(original)]
s=original.read_text();a=s.index('fn candidate(');b=s.index('fn baseline_trace(',a);function=s[a:b]
needle='                    if needed == 0 { continue; }';assert function.count(needle)==1
function=function.replace(needle,'                    COUNTS.with(|c| {let mut v=c.get();v[needed.count_ones() as usize]+=1;c.set(v);});\n'+needle)
needle='                } else { baseline_distances(&a.descriptor,chunk) };';assert function.count(needle)==1
function=function.replace(needle,'                } else { COUNTS.with(|c|{let mut v=c.get();v[5]+=1;c.set(v);});baseline_distances(&a.descriptor,chunk) };')
s=s[:a]+function+s[b:];a=s.index('fn main()');s=s[:a]+r'''
thread_local!{static COUNTS:Cell<[u64;6]>=const{Cell::new([0;6])};}
fn main(){let args:Vec<_>=std::env::args().skip(1).collect();assert_eq!(args.len(),2);let left=read_dump(&args[0]);let right=read_dump(&args[1]);let policy=GradientMatchPolicy{max_comparisons:2*(left.len()*right.len()) as u64,max_squared_distance:0.5,squared_ratio:0.64};let calls=Cell::new(0usize);let expected=baseline(&left,&right,policy,Some(4.),||{calls.set(calls.get()+1);false}).unwrap();let expected_calls=calls.get();calls.set(0);let actual=candidate(&left,&right,policy,Some(4.),||{calls.set(calls.get()+1);false}).unwrap();assert_eq!(expected,actual);assert_eq!(expected_calls,calls.get());println!("{{\"counts\":{:?},\"left\":{},\"right\":{},\"matches\":{},\"callbacks\":{}}}",COUNTS.with(Cell::get),left.len(),right.len(),actual.len(),calls.get());}
'''
F=Path('/private/tmp/rrrah-masked-lane-counts-20261014');F.mkdir(exist_ok=False);src=F/'counts.rs';src.write_text(s);exe=F/'counts'
O=D/'dedup-masked-lane-counts-20261014.json';assert not O.exists()
state=dict(status='building',producer_pid=os.getpid(),pins={str(p):sha(p)for p in [Path(__file__),fixture_path,original_audit,original,src]},feature_pins={x['features']:x['features_sha256']for x in fixture['extractions']},results=[],scope='Instrumented standalone existing global runner-up versus selective lanes, shared current unroll kernel and public matcher distance/ratio/radius. Genuine2048natural feature banks, forward/reverse/self. Counts second-pass masks0..4and unresolved-forward full blocks; exact correspondence/callback counts. Counts explain work distribution, not CPU/managed/native/default/corpus quality proof.')
def save():
    tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(state,indent=2)+'\n');tmp.replace(O)
save()
with (F/'build.log').open('x')as log:
    p=subprocess.Popen(['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)],stdout=log,stderr=subprocess.STDOUT);state['child_pid']=p.pid;save();code=p.wait()
assert code==0,(F/'build.log').read_text();state['pins'][str(exe)]=sha(exe)
for label,inputs in [('forward',features),('reverse',features[::-1]),('self',[features[0],features[0]])]:
    out=F/(label+'.json');err=F/(label+'.stderr')
    with out.open('x')as stdout,err.open('x')as stderr:
        p=subprocess.Popen([str(exe),*inputs],stdout=stdout,stderr=stderr);state.update(status='running',active_case=label,child_pid=p.pid);save();code=p.wait()
    assert code==0 and err.read_bytes()==b'';value=json.loads(out.read_text());assert sum(value['counts'])==value['left']*((value['right']+3)//4)
    counts=value['counts'];before=4*sum(counts[1:]);after=counts[1]+2*counts[2]+4*sum(counts[3:]);assert after<=before
    state['results'].append(dict(label=label,inputs=inputs,value=value,second_pass_distance_pairs_before=before,second_pass_distance_pairs_after=after,stdout=str(out),stdout_sha256=sha(out),stderr=str(err),stderr_sha256=sha(err)));save()
assert all(sha(p)==h for p,h in state['pins'].items());state['status']='complete_pending_audit';save();print([(r['label'],r['value']['counts'])for r in state['results']])
