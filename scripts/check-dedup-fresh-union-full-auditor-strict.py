"""Synthetic audit-parser controls, not source-resolution recall evidence."""
import copy,hashlib,json,subprocess,tempfile
from pathlib import Path
verifier=Path('scripts/verify-dedup-fresh-union-original-full-strict.py')
base=json.loads(Path('docs/research/dedup-fresh-union-original-full-checkpoint-0.json').read_text())
m=json.loads(Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/prepared.json').read_text());pair=m['positive_pairs'][0]
e={'status':'ok','correspondences':[],'matrix':None,'inliers':[],'regions':[],'region_support_count':0,'managed_used':0,'managed_peak':0,'retained_before_drop':0}
row={'original':pair['left']['filename'],'query':pair['right']['filename'],'group_id':pair['left']['group_id'],'returncode':0,'stdout':json.dumps(e),'stderr':'','evidence':e}
base['results']=[row];base['status']='running_pairs';cases={}
def mutate(name,fn):
 r=copy.deepcopy(base);fn(r);cases[name]=r

def evidence(r,key,value):
 r['results'][0]['evidence'][key]=value;r['results'][0]['stdout']=json.dumps(r['results'][0]['evidence'])
mutate('bool_usage',lambda r:evidence(r,'managed_used',False))
mutate('excess_memory',lambda r:evidence(r,'managed_peak',512*1024*1024+1))
mutate('unsupported_regions',lambda r:evidence(r,'region_support_count',1))
mutate('wrong_query',lambda r:r['results'][0].update(query='200301.jpg'))
mutate('wrong_origin',lambda r:r['results'][0].update(group_id=0))
mutate('stdout_mismatch',lambda r:r['results'][0].update(stdout='{}'))
mutate('out_of_bounds',lambda r:evidence(r,'correspondences',[[[1e9,1],[1,1]]]))
mutate('duplicate_points',lambda r:evidence(r,'correspondences',[[[1,1],[1,1]],[[1,1],[1,1]]]))
mutate('nonfinite',lambda r:evidence(r,'correspondences',[[[float('nan'),1],[1,1]]]))
mutate('truncated_terminal',lambda r:r.update(status='complete'))
error=copy.deepcopy(base);error['results'][0].pop('evidence');error['results'][0].update(returncode=101,stdout='',stderr='Synthetic native refusal; parser fixture only.')
with tempfile.TemporaryDirectory(prefix='rrrah-full-audit-controls-') as folder:
 root=Path(folder)
 def run(r,name):
  report=root/(name+'.json');output=root/(name+'-audit.json');report.write_text(json.dumps(r)+'\n')
  v=subprocess.run(['python3',str(verifier),str(report),str(output),'--checkpoint'],capture_output=True,text=True)
  return v,output
 v,out=run(base,'valid_miss');assert v.returncode==0,v.stderr;assert json.loads(out.read_text())['counts']=={'local_supported':0,'miss':1,'native_error':0}
 v,out=run(error,'valid_error');assert v.returncode==0,v.stderr;assert json.loads(out.read_text())['counts']=={'local_supported':0,'miss':0,'native_error':1}
 for name,r in cases.items():
  v,out=run(r,name);assert v.returncode!=0 and not out.exists(),name
output=Path('docs/research/dedup-fresh-union-full-auditor-controls.json');assert not output.exists()
output.write_text(json.dumps({'status':'reconstructed_parser_controls_passed','rejected_corrupt_reports':len(cases),'cases':list(cases),'native_error_retained_in_denominator':True,'input_hashes':{str(verifier):hashlib.sha256(verifier.read_bytes()).hexdigest(),str(Path(__file__)):hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scripts/dedup_jpeg_domain.py':hashlib.sha256(Path('scripts/dedup_jpeg_domain.py').read_bytes()).hexdigest()},'scope':'Synthetic first-row empty miss/error and10 corruptions. Auditor parser controls only; no native229-pair recall evidence.'},indent=2)+'\n')
print('10 corruptions rejected; native error counted separately from miss.')
