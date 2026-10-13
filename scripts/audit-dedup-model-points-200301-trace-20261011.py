#!/usr/bin/env python3
import hashlib,json,math,struct
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-model-points-200301-trace-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_model_selected_points_trace';assert r['plan']['group_id']==200300 and r['plan']['direction']=='reverse';assert len(r['trace'])==25
 pins=dict(r['pins']);pins[str(p)]=sha(p);pins[str(Path(__file__))]=sha(__file__);pins[r['log']]=r['log_sha256'];assert all(sha(k)==v for k,v in pins.items());assert Path(r['log']).read_text()==r['stdout']+r['stderr']
 lines=[x for x in r['stderr'].splitlines() if x.startswith('RRRAH_MODEL_POINTS\t')];assert len(lines)==25
 counts=[]
 for line,t in zip(lines,r['trace']):
  values=[json.loads(v) for v in line.split('\t')[1:]];assert values==[t[k] for k in ['raw_proposals','source_dimensions','target_dimensions','left_rectangle','right_rectangle','coordinate_bits']]
  assert t['raw_proposals']>=len(t['coordinate_bits'])
  for bits in t['coordinate_bits']:
   assert len(bits)==4;v=[struct.unpack('>d',struct.pack('>Q',b))[0] for b in bits];assert all(math.isfinite(x) for x in v)
   for xy,d in [(v[:2],t['source_dimensions']),(v[2:],t['target_dimensions'])]:assert all(0<=x<n for x,n in zip(xy,d))
  counts.append(len(t['coordinate_bits']))
 out=D/'dedup-model-points-200301-trace-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_exact_bit_trace_25_windows','pins':pins,'selected_counts':counts,'total_points':sum(counts),'scope':'Instrumented frozen target-only model pipeline at640, before geometry. Exact mapped correspondences only; not joint-fit success, precision or production adoption.'},indent=2)+'\n');print(counts)
if __name__=='__main__':main()
