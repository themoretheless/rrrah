#!/usr/bin/env python3
import hashlib,json,math,re,runpy,struct
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def values(bits):return [struct.unpack('>d',struct.pack('>Q',b))[0] for b in bits]
def main():
 report=D/'dedup-anchor-scale-remapped-controls-20261011.json';raw=report.read_bytes();r=json.loads(raw);closed=[]
 for row in r['results']:
  if row['status']=='running':break
  closed.append(row)
 assert closed;out=D/f'dedup-anchor-scale-remapped-controls-prefix-{len(closed):04d}-audit-20261011.json';assert not out.exists();capture=Path(r['plan'][0]['input']).parent/f'prefix-{len(closed):04d}.json';assert not capture.exists();capture.write_bytes(raw);assert all(sha(p)==h for p,h in r['pins'].items());pins=dict(r['pins']);pins.update({str(capture):sha(capture),str(Path(__file__)):sha(__file__)});decisions=[]
 original=json.loads((Path('/Users/themoretheless/.codex/tmp/rrrah-model-joint-refit-qualified-20261011/201300-0.json')).read_text());helper=R/'scripts/audit-dedup-model-joint-refit-20261011.py';geo=runpy.run_path(str(helper));pins[str(helper)]=sha(helper);expected=[]
 for item in r['inventory']:
  if item['role']=='excluded_already_same_domain':assert item['dimensions']==[2560,1920];continue
  assert item['dimensions']!=[2560,1920];expected.extend((item['target'],radius) for radius in [8,12,16])
 assert [(p['target'],p['filter_radius']) for p in r['plan']]==expected
 for index,row in enumerate(closed):
  assert row['index']==index and row['returncode']==0;plan=r['plan'][index];assert all(row[k]==value for k,value in plan.items());inp=json.loads(Path(row['input']).read_text());scale=inp['diagnostic_target_scale'];assert scale==[inp['target_dimensions'][i]/original['target_dimensions'][i] for i in range(2)]
  for p,o in zip(inp['points'],original['points']):
   assert p['source_bits']==o['source_bits'];assert values(p['target_bits'])==[v*scale[i] for i,v in enumerate(values(o['target_bits']))]
  assert len(inp['points'])==len(original['points']);assert sha(row['log'])==row['log_sha256'];assert Path(row['log']).read_text()==row['stdout']+row['stderr'];pins[row['log']]=row['log_sha256'];v=json.loads(row['stdout']);assert v['managed_used']==0 and v['filter_radius']==row['filter_radius'];assert v['source_tolerance']==original['source_tolerance'] and v['target_tolerance']==2 and v['trials']==1000000 and v['seed']==173
  if v['best']:
   h=v['best']['transform'];inv=geo['invert'](h);score=0
   for i in v['best']['joint_inliers']:
    p=inp['points'][i];a=values(p['source_bits']);b=values(p['target_bits']);f=sum((x-y)**2 for x,y in zip(geo['project'](h,a),b));back=sum((x-y)**2 for x,y in zip(geo['project'](inv,b),a));assert f<=4 and back<=v['source_tolerance']**2;score+=f/4+back/v['source_tolerance']**2
   assert math.isclose(score,v['best']['normalized_bidirectional_error'],rel_tol=1e-7,abs_tol=1e-7)
  e=v['anchor_result'];supported=False;counts=[]
  for line in row['stderr'].splitlines():
   assert line.startswith('RRRAH_ANCHOR_WINDOW\t');counts.append(dict((k,int(n)) for k,n in re.findall(r'(sites|valid_sites|informative_pairs|agreeing_pairs|pixel_reads): (\d+)',line)))
  if e and 'supported' in e:
   n=e['anchors'][0];assert len(counts)==sum(e['anchors'])
   for ws,agg in zip([counts[:n],counts[n:]],e['directions']):assert all(sum(c[k] for c in ws)==val for k,val in agg.items())
   supported=all(x['informative_pairs']>=1000 and x['valid_sites']/x['sites']>=.3 and x['agreeing_pairs']/x['informative_pairs']>=.9 for x in e['directions']);assert supported==e['supported']
  else:assert not counts
  assert row['status']==('supported' if supported else 'not_supported');decisions.append({'target':row['target'],'filter_radius':row['filter_radius'],'supported':supported if e and 'supported' in e else None,'outcome':'pixel_evidence' if e and 'supported' in e else 'inconclusive_geometry_or_typed_error','anchor_result':e})
 assert all(sha(p)==h for p,h in pins.items());out.write_text(json.dumps({'status':'verified_closed_remapped_scale_control_prefix','pins':pins,'closed':len(closed),'planned':len(r['plan']),'producer_status_at_capture':r['status'],'results':decisions,'scope':'Exact normalized target mapping, source identity, joint residuals and all pixel window counts checked. Artificial coordinate control on original files, not model inference, annotated precision or corpus recovery.'},indent=2)+'\n');print(len(closed),'of',len(r['plan']),'verified')
if __name__=='__main__':main()
