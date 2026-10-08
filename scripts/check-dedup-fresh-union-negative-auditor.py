"""Reconstructed parser controls; never an actual 156-source qualification."""
import copy, hashlib, json, subprocess, tempfile
from pathlib import Path
report=Path('docs/research/dedup-fresh-union-original-negatives-200301.json')
verifier=Path('scripts/verify-dedup-fresh-union-original-negatives.py')
base=json.loads(report.read_text())
positive=json.loads(Path('docs/research/dedup-fresh-candidate-union-200301.json').read_text())
prepared=json.loads(Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/prepared.json').read_text())
original=next(x for x in prepared['images']['original'] if x['group_id']!=200300)
base.update(status='running_negatives',positive_control={'returncode':0,'stdout':positive['stdout'],'evidence':positive['evidence']})
e={'status':'ok','correspondences':[], 'matrix':None,'inliers':[], 'regions':[], 'region_support_count':0, 'retained_before_drop':0,'managed_used':0,'managed_peak':0}
base['results']=[{'original':original['filename'],'original_group':original['group_id'],'returncode':0,'stdout':json.dumps(e),'evidence':e}]
variants={}
def mutation(name,fn):
    r=copy.deepcopy(base);fn(r);variants[name]=r
def evidence_change(r,key,value):
    r['results'][0]['evidence'][key]=value
    r['results'][0]['stdout']=json.dumps(r['results'][0]['evidence'])
mutation('false_positive',lambda r:evidence_change(r,'region_support_count',1))
mutation('bool_counter',lambda r:evidence_change(r,'managed_used',False))
mutation('over_budget',lambda r:evidence_change(r,'managed_peak',512*1024*1024+1))
mutation('wrong_origin',lambda r:r['results'][0].update(original_group=200300))
mutation('wrong_order',lambda r:r['results'][0].update(original='200100.jpg'))
mutation('native_failure',lambda r:r['results'][0].update(returncode=1))
mutation('stdout_mismatch',lambda r:r['results'][0].update(stdout='{}'))
mutation('out_of_bounds',lambda r:evidence_change(r,'correspondences',[[[99999,0],[1,1]]]))
mutation('nonfinite',lambda r:evidence_change(r,'correspondences',[[[float('nan'),0],[1,1]]]))
mutation('duplicate_locations',lambda r:evidence_change(r,'correspondences',[[[1,1],[1,1]],[[1,1],[1,1]]]))
mutation('positive_parity',lambda r:r['positive_control']['evidence'].update(region_support_count=0))
mutation('too_few_terminal',lambda r:r.update(status='complete'))
with tempfile.TemporaryDirectory(prefix='rrrah-union-auditor-') as folder:
    root=Path(folder)
    def run(r,name,checkpoint=True):
        path=root/(name+'.json');out=root/(name+'-audit.json');path.write_text(json.dumps(r,indent=2)+'\n')
        args=['python3',str(verifier),str(path),str(out)]
        if checkpoint:args.append('--checkpoint')
        v=subprocess.run(args,capture_output=True,text=True)
        return v.returncode,out.exists(),v.stderr
    code,exists,stderr=run(base,'valid');assert code==0 and exists,stderr
    for name,r in variants.items():
        code,exists,_=run(r,name,checkpoint=name!='too_few_terminal');assert code!=0 and not exists,name
    output={'status':'reconstructed_parser_controls_passed','rejected_corrupt_reports':len(variants),'cases':list(variants),'input_hashes':{str(verifier):hashlib.sha256(verifier.read_bytes()).hexdigest(),str(Path(__file__)):hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},'scope':'Synthetic empty-evidence prefix plus qualified positive; verifier refusal controls only, not native negative proof.'}
    dest=Path('docs/research/dedup-fresh-union-negative-auditor-controls.json');assert not dest.exists();dest.write_text(json.dumps(output,indent=2)+'\n');print(output['status'],len(variants))
