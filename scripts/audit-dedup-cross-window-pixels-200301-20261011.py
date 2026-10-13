#!/usr/bin/env python3
import hashlib,json,re,struct,math,importlib.util
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-cross-window-pixels-200301-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete_bounded_native_pixel_control' and r['returncode']==0;pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items())
 trace=D/'dedup-model-points-200301-trace-20261011.json';q=json.loads(trace.read_text());ta=D/'dedup-model-points-200301-trace-audit-20261011.json';a=json.loads(ta.read_text());assert all(sha(k)==v for k,v in a['pins'].items());pins.update(a['pins'])
 inp=json.loads(Path(r['command'][1]).read_text());assert inp['source']==q['plan']['left'] and inp['target']==q['plan']['right'] and inp['source_tolerance']==q['plan']['source_tolerance'];assert len(inp['points'])==len(inp['provenance'])==10
 for point,(w,i) in zip(inp['points'],inp['provenance']):assert point['source_bits']+point['target_bits']==q['trace'][w]['coordinate_bits'][i]
 consensus=D/'dedup-joint-consensus-200301-20261011.json';c=json.loads(consensus.read_text());best=json.loads(c['results'][6]['stdout'])['best'];bits=lambda v:struct.unpack('>Q',struct.pack('>d',v))[0];assert inp['transform_bits']==[[bits(v) for v in row] for row in best['transform']]
 module=R/'scripts/audit-dedup-joint-consensus-200301-20261011.py';spec=importlib.util.spec_from_file_location('audit',module);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);h=best['transform'];inv=m.inverse(h);pts=[]
 for point in inp['points']:
  a,b=[[struct.unpack('>d',struct.pack('>Q',v))[0] for v in point[k+'_bits']] for k in ['source','target']];assert math.dist(m.apply(h,a),b)<=2+1e-8 and math.dist(m.apply(inv,b),a)<=inp['source_tolerance']+1e-8
  assert all(math.dist(a,x)>2 and math.dist(b,y)>2 for x,y in pts);pts.append((a,b))
 v=json.loads(r['stdout']);assert v==r['result'] and v['managed_used']==0 and v['points']==10;positive=v['results'][0];assert positive['window_radius']==2 and positive['supported'];s=positive['evidence'];counts=[tuple(map(int,x)) for x in re.findall(r'sites: (\d+), valid_sites: (\d+), informative_pairs: (\d+), agreeing_pairs: (\d+)',s)];assert len(counts)==2 and all(info>=1000 and valid/sites>=.3 and agree/info>=.9 for sites,valid,info,agree in counts)
 assert v['results'][1]['window_radius']==6 and not v['results'][1]['supported'] and 'InsufficientGeometry' in v['results'][1]['evidence']
 pins.update({str(x):sha(x) for x in [p,trace,ta,consensus,module,Path(__file__)]});out=D/'dedup-cross-window-pixels-200301-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_cross_window_geometric_and_original_pixel_control','pins':pins,'provenance':inp['provenance'],'pixel_counts':counts,'supported_at_radius2':True,'scope':'Geometric selection before pixels across two windows. Strict10 witnesses/1000 informative/.9 agreement/.3 coverage retained; public original-pixel check with exact H. Diagnostic only, no integrated model-union path or independent semantic precision/full corpus claim.'},indent=2)+'\n');print(counts)
if __name__=='__main__':main()
