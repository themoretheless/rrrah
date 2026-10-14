#!/usr/bin/env python3
"""Diagnostic exact guard expression plus original token code; explicit request adapter."""
from pathlib import Path
import hashlib,json,subprocess,resource,time,statistics,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();guard=R/'crates/rrrah-dedup/src/guarded_pair.rs';decode=R/'crates/rrrah-decode/src/lib.rs';gs=guard.read_text();ds=decode.read_text();a=gs.index('        let stop = latch.get()',gs.index('let cancelled = ||'));b=gs.index('\n    };',a);body=gs[a:b];assert body.endswith('        stop');a=ds.index('#[derive(Debug, Clone)]\npub struct GenerationToken');b=ds.index('\n#[derive(Debug, Clone, Copy, Default)]\npub struct AdaptTimings',a);token=ds[a:b]
F=Path('/private/tmp/rrrah-guard-latch-20261014');F.mkdir(exist_ok=False);K=D/'dedup-guard-latch-20261014';K.mkdir(exist_ok=False);O=D/'dedup-guard-latch-20261014.json';assert not O.exists();base='''use std::{cell::Cell,sync::{Arc,atomic::{AtomicU64,Ordering}},hint::black_box};
TOKEN
struct Request { cancellation: Option<GenerationToken> }
#[inline(always)] fn poll(latch:&Cell<bool>,cancel:&dyn Fn()->bool,left:&Request,right:&Request)->bool { BODY }
fn exercise(depth:usize,mode:usize,reps:usize,stop:usize,kind:usize)->(usize,usize){
 let a=Arc::new(AtomicU64::new(0));let b=Arc::new(AtomicU64::new(0));let c=Arc::new(AtomicU64::new(0));let ta=GenerationToken::new(a.clone(),0);let tb=GenerationToken::new(b.clone(),0);let tc=GenerationToken::new(c.clone(),0);
 let left=Request{cancellation:match mode{0=>None,1=>Some(ta.clone()),_=>Some(ta.combine(&tc))}};let right=Request{cancellation:match mode{0=>None,1=>Some(tb.clone()),_=>Some(tb.combine(&tc))}};
 let calls=Cell::new(0usize);let root=||{calls.set(calls.get()+1);if stop>0&&calls.get()==stop{match kind{0=>return true,1=>a.store(1,Ordering::Release),2=>b.store(1,Ordering::Release),_=>c.store(1,Ordering::Release)}}false};
 let l1=Cell::new(false);let one=||poll(&l1,black_box(&root),&left,&right);let l2=Cell::new(false);let two=||poll(&l2,black_box(&one),&left,&right);let l3=Cell::new(false);let three=||poll(&l3,black_box(&two),&left,&right);
 let target:&dyn Fn()->bool=if depth==1{black_box(&one)}else{black_box(&three)};let mut accepted=0;for _ in 0..reps{accepted+=usize::from(black_box(target()));}(calls.get(),accepted)
}
fn main(){let args:Vec<_>=std::env::args().collect();if args[1]=="controls"{let mut n=0;for depth in [1,3]{for mode in 0..3{for stop in 1..=17{for kind in 0..(if mode==0{1}else if mode==1{3}else{4}){let(c,t)=exercise(depth,mode,40,stop,kind);assert_eq!(c,stop);assert_eq!(t,41-stop);n+=1;}}}}println!("{{\\"controls\\":{n}}}");return;}let depth=args[1].parse().unwrap();let mode=args[2].parse().unwrap();let reps=args[3].parse().unwrap();let(c,t)=exercise(depth,mode,reps,0,0);assert_eq!(c,reps);assert_eq!(t,0);println!("{{\\"calls\\":{c},\\"cancelled\\":{t}}}");}
''';d=dict(status='building',producer_pid=os.getpid(),producer_sha256=sha(__file__),source_pins={str(p):sha(p)for p in [guard,decode]},guard_body_sha256=hashlib.sha256(body.encode()).hexdigest(),token_source_sha256=hashlib.sha256(token.encode()).hexdigest(),results=[],builds=[],scope='Diagnostic only: exact currentguard cancellation expression and originalGenerationToken implementation, controlledRequest adapter containing only cancellation field, dynFn one/three-layer chain, realAtomicU64 owners. Inline helper removes harness-onlycall; no actual DecodeRequest layout, IO/sourceguards, core/decoder/nativepipeline or wholeoperation/RSS qualification. Only false-latch stores changed; no usercallback or ownercheck omitted. Root not modified.')
def save():
 t=O.with_suffix('.tmp');t.write_text(json.dumps(d,indent=2)+'\n');t.replace(O)
save()
for v in ['baseline','candidate']:
 B=body if v=='baseline'else body.replace('latch.set(stop);','if stop { latch.set(true); }');assert B==body or B.replace('if stop { latch.set(true); }','latch.set(stop);')==body;src=K/(v+'.rs');src.write_text(base.replace('TOKEN',token).replace('BODY',B));exe=F/v;cmd=['rustc','+stable','--edition=2024','-C','opt-level=3',str(src),'-o',str(exe)]
 with (K/(v+'-build.log')).open('x')as f:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT)
 assert r.returncode==0;r=subprocess.run([str(exe),'controls'],capture_output=True,text=True);assert r.returncode==0;assert json.loads(r.stdout)['controls']==272;(K/(v+'-controls.json')).write_text(r.stdout);d['builds'].append(dict(version=v,command=cmd,source_sha256=sha(src),executable=str(exe),executable_sha256=sha(exe),controls=272));save()
d.update(status='benchmarking');save()
for depth,mode,reps in [(1,1,20000000),(3,0,10000000),(3,1,10000000),(3,2,10000000)]:
 label=f'depth{depth}-owners{mode}'
 for pair in range(9):
  for v in (['baseline','candidate']if pair%2==0 else['candidate','baseline']):
   command=[str(F/v),str(depth),str(mode),str(reps)];before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic();c=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);d.update(active_case=[label,pair,v],child_pid=c.pid);save();out,err=c.communicate();after=resource.getrusage(resource.RUSAGE_CHILDREN);wall=time.monotonic()-start;assert c.returncode==0 and err==''and json.loads(out)==dict(calls=reps,cancelled=0);cpu=after.ru_utime-before.ru_utime+after.ru_stime-before.ru_stime;raw=K/f'{label}-{pair}-{v}.json';raw.write_text(json.dumps(dict(command=command,returncode=c.returncode,stdout=out,stderr=err,cpu_seconds=cpu,wall_seconds=wall,before=[before.ru_utime,before.ru_stime],after=[after.ru_utime,after.ru_stime]),indent=2)+'\n');d['results'].append(dict(case=label,pair=pair,version=v,warmup=pair==0,raw=str(raw.relative_to(R)),raw_sha256=sha(raw),cpu_seconds=cpu));save()
summary=[]
for label in sorted({r['case']for r in d['results']}):
 ratios=[]
 for pair in range(1,9):
  rows={r['version']:r['cpu_seconds']for r in d['results']if r['case']==label and r['pair']==pair};ratios.append(rows['candidate']/rows['baseline'])
 summary.append(dict(case=label,median_cpu_ratio=statistics.median(ratios),ratios=ratios))
assert all(sha(p)==h for p,h in d['source_pins'].items());d.update(status='completed_pending_independent_audit',summary=summary);save();print(summary)
