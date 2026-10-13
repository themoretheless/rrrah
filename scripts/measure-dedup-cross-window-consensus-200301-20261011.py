#!/usr/bin/env python3
import hashlib,json,math,struct,importlib.util
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 audit=D/'dedup-joint-consensus-200301-audit-20261011.json';a=json.loads(audit.read_text());assert all(sha(p)==h for p,h in a['pins'].items());source=D/'dedup-joint-consensus-200301-20261011.json';r=json.loads(source.read_text());module=R/'scripts/audit-dedup-joint-consensus-200301-20261011.py';spec=importlib.util.spec_from_file_location('audit',module);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);points=[];tol=None
 for window,p in enumerate(r['plan']):
  q=json.loads(Path(p).read_text());tol=q['source_tolerance']
  for index,v in enumerate(q['points']):points.append({'window':window,'index':index,'source':[struct.unpack('>d',struct.pack('>Q',b))[0] for b in v['source_bits']],'target':[struct.unpack('>d',struct.pack('>Q',b))[0] for b in v['target_bits']]})
 results=[]
 for window,row in enumerate(r['results']):
  best=json.loads(row['stdout'])['best']
  if not best:continue
  h=best['transform'];inv=m.inverse(h);accepted=[]
  for p in points:
   if math.dist(m.apply(h,p['source']),p['target'])>2 or math.dist(m.apply(inv,p['target']),p['source'])>tol:continue
   if any(math.dist(p['source'],q['source'])<=2 or math.dist(p['target'],q['target'])<=2 for q in accepted):continue
   accepted.append(p)
  results.append({'model_window':window,'joint_nonaliased_points':len(accepted),'provenance':[[p['window'],p['index']] for p in accepted]})
 out=D/'dedup-cross-window-consensus-200301-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'complete_bounded_cross_window_consensus','pins':{str(p):sha(p) for p in [audit,source,module,Path(__file__)]},'results':results,'scope':'Rescore existing sampled models on all669 captured coordinates with strict original residuals and greedy2px endpoint alias rejection. No new fitting/pixels/production support; ordering not exhaustive.'},indent=2)+'\n');print(sorted(results,key=lambda r:r['joint_nonaliased_points'],reverse=True)[:3])
if __name__=='__main__':main()
