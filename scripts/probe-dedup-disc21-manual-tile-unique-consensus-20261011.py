#!/usr/bin/env python3
"""Feasibility diagnostic with an explicit manual oracle, never production recall."""
import hashlib,json,re,struct,subprocess,time
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def bits(x):return struct.unpack('<Q',struct.pack('<d',x))[0]
def main():
 parent=D/'dedup-joint-consensus-200301-20261011.json';m=json.loads(parent.read_text());assert all(sha(k)==v for k,v in m['pins'].items());binary=next(Path(k) for k in m['pins'] if k.endswith('/rrrah-joint-consensus-200301-qualified-20261011/joint_consensus_diagnostic'));A=T/'rrrah-disc21-manual-tile-unique-consensus-20261011';out=D/'dedup-disc21-manual-tile-unique-consensus-20261011.json';assert not A.exists() and not out.exists();A.mkdir();pins={str(parent):sha(parent),str(binary):sha(binary),str(Path(__file__)):sha(__file__)};r={'status':'running','pins':pins,'results':[],'scope':'Manual tile oracle feasibility only. Existing pinned generic geometry diagnostic,1m trials/seed173/tolerances2,minimum4; no pixel confirmation or automatic retrieval qualification. Exact endpoint aliases removed in caller order before fitting; this order can discard other possible matches and is not an unbiased estimator.'}
 def save():out.write_text(json.dumps(r,indent=2)+'\n')
 save()
 for kind,name in [('raw','dark-proposal-locations'),('normalized','dark-normalized-proposals')]:
  p=D/f'dedup-disc21-{name}-20261011.json';data=json.loads(p.read_text());assert data['status']=='complete' and all(sha(k)==v for k,v in data['pins'].items());audit=D/f'dedup-disc21-{name}-audit-20261011.json';a=json.loads(audit.read_text());assert all(sha(k)==v for k,v in a['pins'].items());pins.update({str(p):sha(p),str(audit):sha(audit)});points=[]
  for line in data['stderr'].splitlines():
   if not line.startswith('BANK '):continue
   for q in re.findall(r'source: \[([^,]+), ([^\]]+)\], target: \[([^,]+), ([^\]]+)\]',line):
    x,y,u,v=map(float,q)
    if 199<=x<373 and 694<=y<867:points.append({'source':[x,y],'target':[u,v],'source_bits':[bits(x),bits(y)],'target_bits':[bits(u),bits(v)]})
  original_count=len(points);source_seen=set();target_seen=set();unique=[]
  for point in points:
   source_key=tuple(point['source_bits']);target_key=tuple(point['target_bits'])
   if source_key in source_seen or target_key in target_seen:continue
   unique.append(point);source_seen.add(source_key);target_seen.add(target_key)
  points=unique
  path=A/(kind+'.json');path.write_text(json.dumps({'points':points,'source_dimensions':[576,1024],'target_dimensions':[1024,760],'source_tolerance':2.0})+'\n');pins[str(path)]=sha(path);cmd=[str(binary),str(path),'1000000','173'];start=time.monotonic();p=subprocess.run(cmd,capture_output=True,text=True);log=A/(kind+'.log');log.write_text(p.stdout+p.stderr);pins[str(log)]=sha(log);assert p.returncode==0,p.stderr;e=json.loads(p.stdout);r['results'].append({'kind':kind,'before_exact_alias_elision':original_count,'command':cmd,'elapsed_seconds':time.monotonic()-start,'evidence':e});save();print(kind,'points',e['points'],'best_inliers',len(e['best']['joint_inliers']) if e['best'] else 0,flush=True)
 assert all(sha(k)==v for k,v in pins.items());r['status']='complete_manual_oracle_feasibility_diagnostic';save()
if __name__=='__main__':main()
