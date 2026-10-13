#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,subprocess,resource,time,os,statistics
R=Path(__file__).resolve().parents[1];T=Path('/Users/themoretheless/.codex/tmp');D=R/'docs/research';F=T/'rrrah-managed-version-benchmark-20261014';F.mkdir(exist_ok=False)
executables={'baseline':T/'rrrah-managed-baseline-benchmark-20261014/target/release/managed-baseline-benchmark','cache_disabled':T/'rrrah-managed-cache-benchmark-20261014/target/release/managed-cache-off-benchmark'};sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();inputs=T/'rrrah-gradient-natural-bound-benchmark-20261013';cases=[('self',inputs/'features-01.bin',inputs/'features-01.bin'),('different',inputs/'features-02.bin',inputs/'features-03.bin')]
pins={str(p):sha(p) for p in [Path(__file__),*executables.values(),*[p for c in cases for p in c[1:]],*[T/h/r for h in ['rrrah-managed-baseline-benchmark-20261014','rrrah-managed-cache-off-benchmark-20261014'] for r in ['Cargo.toml','Cargo.lock','src/main.rs']]]}
O=D/'dedup-managed-version-benchmark-20261014.json';assert not O.exists();d=dict(status='running',pid=os.getpid(),pins=pins,results=[],scope='Same1024natural-feature banks and native distance0.5/ratio0.64/radius2 policy. Baseline frozen source versus cache-v3 with zero optional headroom in both paths. Four alternating fresh-process pairs per case, each process performs20same uncached managed calls. Child user+systemCPU measured independently of wall contention. Correspondence counts compared across versions, exact bits only internally per executable; source versions include formatting drift.')
def save():
 tmp=O.with_suffix('.tmp');tmp.write_text(json.dumps(d,indent=2)+'\n');tmp.replace(O)
save()
for name,left,right in cases:
 expected=None
 for pair in range(4):
  values={}
  for label in (['baseline','cache_disabled'] if pair%2==0 else ['cache_disabled','baseline']):
   before=resource.getrusage(resource.RUSAGE_CHILDREN);start=time.monotonic_ns();r=subprocess.run([str(executables[label]),str(left),str(right)],capture_output=True,text=True);wall=time.monotonic_ns()-start;after=resource.getrusage(resource.RUSAGE_CHILDREN);cpu=(after.ru_utime-before.ru_utime)+(after.ru_stime-before.ru_stime)
   row=dict(case=name,pair=pair,version=label,wall_ns=wall,cpu_seconds=cpu,returncode=r.returncode,stdout=r.stdout,stderr=r.stderr);P=F/f'{name}-{pair}-{label}.json';P.write_text(json.dumps(row,indent=2)+'\n');d['results'].append(dict(report=str(P),sha256=sha(P),returncode=r.returncode));save();assert r.returncode==0
   samples=[json.loads(l) for l in r.stdout.splitlines()];assert len(samples)==9 and all(x['bits_identical'] and x['released'] and x['cached_peak']==x['uncached_peak'] for x in samples)
   matches=[x['matches'] for x in samples]
   if expected is None:expected=matches
   assert expected==matches;values[label]=cpu
  print(name,pair,values['cache_disabled']/values['baseline'],flush=True)
assert all(sha(p)==h for p,h in pins.items());d['status']='complete_pending_audit';save()
