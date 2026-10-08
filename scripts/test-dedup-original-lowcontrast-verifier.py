"""Actual native miss is valid evidence; malformed success claims must refuse."""
import copy,hashlib,json,subprocess,sys,tempfile
from pathlib import Path
report=Path('docs/research/dedup-original-lowcontrast-200301.json');verifier=Path('scripts/verify-dedup-original-lowcontrast.py');base=json.loads(report.read_text())
mutations=[('wrong_query',lambda r:r.update(query='200101.jpg')),('wrong_recipe',lambda r:r.update(detector_minimum_corner_score=0.0001)),('bad_pin',lambda r:r['input_hashes'].update({str(report):'0'*64})),('boolean_credit',lambda r:r['evidence'].update(managed_used=False)),('unreleased_credit',lambda r:r['evidence'].update(managed_used=1)),('over_budget',lambda r:r['evidence'].update(managed_peak=512*1024*1024+1)),('nonfinite_point',lambda r:r['evidence']['correspondences'][0][0].__setitem__(0,float('inf'))),('inlier_without_model',lambda r:r['evidence'].update(inliers=[0])),('whole_without_model',lambda r:r['evidence'].update(whole_candidate=True)),('support_without_model',lambda r:r['evidence'].update(region_support_count=1))]
with tempfile.TemporaryDirectory(prefix='dedup-lowcontrast-parser-') as folder:
 folder=Path(folder)
 def run(value,name):
  source=folder/(name+'.json');out=folder/(name+'-audit.json');source.write_text(json.dumps(value));v=subprocess.run([sys.executable,str(verifier),str(source),str(out)],capture_output=True,text=True);return v,out
 v,out=run(base,'native_control');assert v.returncode==0,v.stderr
 rejected=[]
 for name,mutate in mutations:
  r=copy.deepcopy(base);mutate(r);r['stdout']=json.dumps(r['evidence']);v,out=run(r,name);assert v.returncode!=0 and not out.exists(),name;rejected.append(name)
out=Path('docs/research/dedup-original-lowcontrast-verifier-adversaries.json');assert not out.exists();out.write_text(json.dumps({'status':'verified_native_control_and_corrupted_report_refusal','rejected_cases':rejected,'input_hashes':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [report,verifier,Path(__file__)]},'scope':'Actual completed native miss accepted as evidence; ten corrupt reports rejected. Parser robustness does not establish copy recall or negative precision.'},indent=2)+'\n');print('Actual native control and ten refusals verified')
