"""Corrupted-report controls; reconstructed parser input is not a native six-file run."""
import copy,hashlib,json,subprocess,sys,tempfile
from pathlib import Path
verifier=Path('scripts/verify-dedup-original-area-six-collection.py');positive=Path('docs/research/dedup-original-area-grid8-blur7-200101.json')
base=json.loads(Path('docs/research/dedup-original-area-six-collection.json').read_text());e=json.loads(positive.read_text())['evidence'];pair={k:copy.deepcopy(e[k]) for k in ['whole_candidate','correspondences','matrix','inliers','regions','region_support_count']};pair.update(left=1,right=2)
base.update(status='running_fresh_pairs',collection={'status':'ok','analysed':[1,2,3,4,5,6],'insufficient_features':[],'indexed_features':100,'descriptor_hits':100,'proposed_pairs':1,'pixel_verification_pairs':1,'pairs':[pair],'managed_used':0,'managed_peak':e['managed_peak'],'retained_before_drop':e['retained_before_drop']},results=[{'left':1,'right':2,'same_origin':True,'retrieved':True,'evidence':e}])
mutations=[('bad_pin',lambda r:r['input_hashes'].update({str(positive):'0'*64})),('wrong_label',lambda r:r['results'][0].update(same_origin=False)),('missed_retrieval',lambda r:r['results'][0].update(retrieved=False)),('duplicate_pair',lambda r:r['collection']['pairs'].append(copy.deepcopy(r['collection']['pairs'][0]))),('bad_counter',lambda r:r['collection'].update(proposed_pairs=2)),('boolean_peak',lambda r:r['collection'].update(managed_peak=True)),('unreleased_credit',lambda r:r['collection'].update(managed_used=1)),('invalid_inlier',lambda r:r['results'][0]['evidence']['inliers'].__setitem__(0,999999)),('domain_outside',lambda r:r['collection']['pairs'][0]['regions'][0]['domains'][0].__setitem__(0,999999)),('parity_disagreement',lambda r:r['results'][0]['evidence']['correspondences'][0][0].__setitem__(0,999999))]
with tempfile.TemporaryDirectory(prefix='dedup-six-parser-') as folder:
 folder=Path(folder)
 def run(value,name):
  source=folder/(name+'.json');target=folder/(name+'-audit.json');source.write_text(json.dumps(value,allow_nan=False));result=subprocess.run([sys.executable,str(verifier),str(source),str(target),'--checkpoint'],capture_output=True,text=True);return result,target
 result,target=run(base,'control');assert result.returncode==0,result.stderr
 rejected=[]
 for name,mutate in mutations:
  value=copy.deepcopy(base);mutate(value);result,target=run(value,name);assert result.returncode!=0 and not target.exists(),name;rejected.append(name)
out=Path('docs/research/dedup-area-six-verifier-adversaries.json');assert not out.exists();out.write_text(json.dumps({'status':'verified_parser_controls','rejected_cases':rejected,'input_hashes':{str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [verifier,positive,Path(__file__)]},'scope':'Reconstructed one-pair parser control based on prior native positive. Not evidence of an actual six-source native collection result.'},indent=2)+'\n');print('control accepted; ten corrupted reports rejected')
