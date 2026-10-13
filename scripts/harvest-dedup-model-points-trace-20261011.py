#!/usr/bin/env python3
import hashlib,json,re,struct
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-model-points-trace-harvest-20261011.json';assert not out.exists();rp=D/'dedup-model-points-trace-20261011.json';r=json.loads(rp.read_text());assert all(sha(p)==h for p,h in r['pins'].items());mp=D/'dedup-model-points-trace-snapshot-20261011.json';m=json.loads(mp.read_text());assert all(sha(Path(m['snapshot'])/f)==h for f,h in m['file_hashes'].items());log=T/'rrrah-model-points-trace-qualified-20261011/201300.log';text=log.read_text();assert '\nError: InsufficientAnchorGeometry(ModelTileEvidence' in text;assert 'len: 25, model_digest:' in text;traces=[]
 for line in text.splitlines():
  if line.startswith('RRRAH_MODEL_POINTS\t'):
   fields=line.split('\t');assert len(fields)==7;raw,source,target,left,right,points=[json.loads(v) for v in fields[1:]];assert 0<=len(points)<=raw<=5000
   for p in points:
    assert len(p)==4;values=[struct.unpack('>d',struct.pack('>Q',v))[0] for v in p];assert all(0<=values[i]<[source[0],source[1],target[0],target[1]][i] for i in range(4))
   traces.append({'raw_proposals':raw,'source_dimensions':source,'target_dimensions':target,'left_rectangle':left,'right_rectangle':right,'coordinate_bits':points})
 assert len(traces)==25;rects=lambda d:[[0,0,*d],[0,0,d[0]//2,d[1]//2],[d[0]//2,0,d[0]-d[0]//2,d[1]//2],[0,d[1]//2,d[0]//2,d[1]-d[1]//2],[d[0]//2,d[1]//2,d[0]-d[0]//2,d[1]-d[1]//2]];assert {(tuple(v['left_rectangle']),tuple(v['right_rectangle'])) for v in traces}=={(tuple(l),tuple(r)) for l in rects(traces[0]['source_dimensions']) for r in rects(traces[0]['target_dimensions'])};insufficient=text.count('evidence: InsufficientAnchorGeometry');assert insufficient==3
 pins={str(p):sha(p) for p in [rp,mp,log,Path(__file__)]};out.write_text(json.dumps({'status':'verified_25_actual_model_point_sets_typed_inconclusive','pins':pins,'plan':r['plan'],'trace':traces,'insufficient_geometry_attempts':insufficient,'typed_result':'InsufficientAnchorGeometry','scope':'Harvest of terminal model-only reverse201300 diagnostic with expected typed inconclusive error, not a model crash.25 exact selected/mapped point sets from pinned instrumented source, full Cartesian windows and point domains checked. Original producer asserted success incorrectly; no rerun or managed-release/pixel/model-joint fix claim.'},indent=2)+'\n');print('Verified25 point sets;',sum(len(t['coordinate_bits']) for t in traces),'points;3 insufficient attempts')
if __name__=='__main__':main()
