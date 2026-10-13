#!/usr/bin/env python3
import hashlib,json,struct
from pathlib import Path
D=Path(__file__).resolve().parents[1]/'docs/research'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 p=D/'dedup-disc21-overlap-regions-native-v2-20261011.json';r=json.loads(p.read_text());assert r['status']=='complete' and r['returncode']==r['build_returncode']==0
 pins=dict(r['pins']);assert all(sha(k)==v for k,v in pins.items());assert sha(r['build_log'])==r['build_log_sha256'];e=json.loads(r['stdout']);assert e['managed_used']==0;e=e['evidence'];assert e['bank_count']==9 and e['original_dimensions']==[[576,1024],[1024,760]];expected=[[x,y,288,512] for y in [0,256,512] for x in [0,144,288]];assert e['source_rectangles']==expected and e['target_rectangle']==[0,0,1024,760];assert [b['source_rectangle'] for b in e['banks']]==expected
 for b in e['banks']:
  for q in b['points']:
   for key in ['source','target']:
    assert len(q[key+'_bits'])==2
    exact=[struct.unpack('<d',struct.pack('<Q',v))[0] for v in q[key+'_bits']];assert all(abs(a-b)<1e-9 for a,b in zip(exact,q[key]))
 for a in e['pixel_attempts']:
  assert a['bank'] in range(9) and a['window_radius'] in [2,6] and type(a['supported']) is bool
  if a['supported']:assert 'supported: true' in a['pixels'] and 'source_window_bounds:' in a['pixels'] and 'target_window_bounds:' in a['pixels']
 assert e['supported']==any(a['supported'] for a in e['pixel_attempts'])
 original=D/'dedup-disc21-bank-diagnostic-native-20261011.json';old=json.loads(original.read_text());assert all(sha(k)==v for k,v in old['pins'].items());pins.update(old['pins']);verify=D/'dedup-anchor-window-bounds-verification-20261011.json';v=json.loads(verify.read_text());assert v['status']=='verified_157_tests_anchor_window_bounds' and all(sha(k)==h for k,h in v['pins'].items());pins.update({str(x):sha(x) for x in [p,original,verify,Path(__file__),Path(r['build_log'])]})
 localization=[]
 for a in e['pixel_attempts']:
  if a['supported']:localization.append({'bank':a['bank'],'window_radius':a['window_radius'],'witnesses':len(a['witnesses']),'inside_manual_tile':sum(199<=q['source'][0]<373 and 694<=q['source'][1]<867 for q in a['witnesses'])})
 out=D/'dedup-disc21-overlap-regions-native-v2-audit-20261011.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_fixed_automatic_overlap_experiment','pins':pins,'supported':e['supported'],'pixel_attempts':e['pixel_attempts'],'supported_localization_controls':localization,'bank_point_counts':[len(b['points']) for b in e['banks']],'scope':'Automatic fixed9-window half-image overlap experiment, unchanged pixel thresholds and full outer guards. Manual tile used only offline for coarse localization, not official localization annotations. No cohort/general precision or whole-library completion claim.'},indent=2)+'\n');print('support',e['supported'],'localization',localization)
if __name__=='__main__':main()
