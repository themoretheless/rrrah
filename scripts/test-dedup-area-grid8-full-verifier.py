"""Audit-parser controls from prior native positive, not full negative qualification."""
import copy,hashlib,json,subprocess,sys,tempfile
from pathlib import Path
verifier=Path('scripts/verify-dedup-area-grid8-original-negatives.py')
report=Path('docs/research/dedup-area-grid8-original-negatives-200101.json')
positive=Path('docs/research/dedup-original-area-grid8-blur7-200101.json')
base=json.loads(report.read_text());control=json.loads(positive.read_text())
base.update(status='running_negatives',results=[],positive_control={k:control[k] for k in ['returncode','stdout','stderr','evidence']})
mutations=[
 ('wrong_query_group',lambda r:r.update(query_group=-1)),
 ('bad_source_pin',lambda r:r['input_hashes'].update({str(positive):'0'*64})),
 ('invalid_inlier',lambda r:r['positive_control']['evidence']['inliers'].__setitem__(0,999999)),
 ('boolean_used',lambda r:r['positive_control']['evidence'].update(managed_used=False)),
 ('unreleased_credit',lambda r:r['positive_control']['evidence'].update(managed_used=1)),
 ('over_budget',lambda r:r['positive_control']['evidence'].update(managed_peak=512*1024*1024+1)),
 ('bad_region_counter',lambda r:r['positive_control']['evidence'].update(region_support_count=999)),
 ('domain_outside_jpeg',lambda r:r['positive_control']['evidence']['regions'][0]['domains'][0].__setitem__(0,999999)),
 ('impossible_pixels',lambda r:r['positive_control']['evidence']['regions'][0]['pixels']['counts'][0].__setitem__(0,999999999)),
]
with tempfile.TemporaryDirectory(prefix='dedup-full-audit-') as directory:
 directory=Path(directory)
 def run(value,name):
  source=directory/(name+'.json');target=directory/(name+'-audit.json');source.write_text(json.dumps(value,allow_nan=False))
  result=subprocess.run([sys.executable,str(verifier),str(source),str(target),'--checkpoint'],capture_output=True,text=True)
  return result,target
 result,target=run(base,'parser_control');assert result.returncode==0,result.stderr;assert target.exists()
 results=[]
 for name,mutate in mutations:
  r=copy.deepcopy(base);mutate(r);r['positive_control']['stdout']=json.dumps(r['positive_control']['evidence'],allow_nan=False)
  result,target=run(r,name);assert result.returncode!=0 and not target.exists(),name;results.append(name)
 r=copy.deepcopy(base);r['positive_control']['evidence']['managed_used']=1
 result,target=run(r,'raw_disagreement');assert result.returncode!=0 and not target.exists();results.append('raw_disagreement')
out=Path('docs/research/dedup-area-grid8-full-verifier-adversaries.json');assert not out.exists()
out.write_text(json.dumps({'status':'verified_full_gate_parser_adversaries','rejected_cases':results,'input_hashes':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [verifier,positive,Path(__file__)]},'scope':'Parser control reconstructed from completed native positive and current pinned input inventory, zero negative rows. Ten corrupted records rejected; not completed full-gate or negative precision evidence.'},indent=2)+'\n')
print('parser control passed; ten corrupt reports rejected')
