"""Automatic-grid all-other-original negative gate under normalized geometry."""
import json,hashlib,subprocess
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');base=Path('docs/research');h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();reference=base/'dedup-native-screen-affine-color-negative-gate.json';ref=json.loads(reference.read_text());assert ref['status']=='complete' and len(ref['rows'])==156 and all(h(k)==v for k,v in ref['input_hashes'].items());exe=root/'affine-color-grid-image-probe-screen-meanfix';right=root/'original-resolution-strong-all/200501.jpg';positive=base/'dedup-native-screen-affine-color-grid-meanfix.json';p=json.loads(positive.read_text());assert all(h(k)==v for k,v in p['input_hashes'].items());pins={str(v):h(v) for v in [reference,positive,exe,right,Path(__file__)]};out=base/'dedup-native-screen-affine-color-grid-negatives-meanfix.json';assert not out.exists();report={'status':'running','required_cases':156,'input_hashes':pins,'rows':[],'scope':'Automatic grid under normalized supplied geometry against all other originals; all refusals/errors retained. No feature retrieval, burst or all-query precision proof.'}
def save():
 tmp=out.with_suffix('.tmp');tmp.write_text(json.dumps(report,indent=2)+'\n');tmp.replace(out)
save()
for case in ref['rows']:
 left=Path(case['source']);assert h(left)==case['source_sha256'];old=Path(case['input']);assert h(old)==case['input_sha256'];input=root/('screen-color-grid-meanfix-negative-'+left.stem+'.txt');assert not input.exists();input.write_text(' '.join(old.read_text().split()[:9])+'\n')
 v=subprocess.run([str(exe),str(left),str(right),str(input)],capture_output=True,text=True);assert h(left)==case['source_sha256'] and all(h(k)==v for k,v in pins.items());row={'source':str(left),'source_sha256':h(left),'input':str(input),'input_sha256':h(input),'returncode':v.returncode,'stdout':v.stdout,'stderr':v.stderr}
 if v.returncode==0:
  e=json.loads(v.stdout);assert e['managed_used']==0;row['evidence']=e
  if e['status']=='ok':row['supports']=sum(all('matched' in d and d['matched']>=.9*d['samples'] for d in region['directions']) for region in e['regions'])
 report['rows'].append(row);save();print(left.name,row.get('supports'),row.get('evidence',{}).get('status'),flush=True)
report['status']='complete';save()
