"""Raw fit diagnostics, preserving the original acceptance coefficient limits."""
import hashlib,json,shutil,subprocess
from pathlib import Path
base=Path('docs/research');root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
reference=base/'dedup-folded-grid8-common-footprint-regions.json';prior=json.loads(reference.read_text());assert prior['status']=='complete_native_experiment' and len(prior['rows'])==6 and all(digest(k)==v for k,v in prior['input_hashes'].items())
exe=root/'affine-color-common-footprint-bounds-diagnostic';assert not exe.exists();shutil.copy2('target/debug/examples/affine_color_common_footprint_bounds_diagnostic',exe)
left=root/'original-resolution-negative-200201/201700.jpg';right=root/'original-resolution-strong-all/201702.jpg'
paths=[reference,exe,left,right,Path(__file__),Path('crates/rrrah-dedup/examples/affine_color_common_footprint_bounds_diagnostic.rs'),*[Path('crates/rrrah-dedup/src')/name for name in ['affine_color.rs','affine_region.rs','affine_region_file.rs']]]
pins={str(p):digest(p) for p in paths};output=base/'dedup-folded-grid8-color-bounds-diagnostic.json';assert not output.exists();report={'status':'running','required_cases':6,'input_hashes':pins,'rows':[],'scope':'Diagnostic raw color fits only, with original coefficient5/offset0.1 acceptance flags recomputed. Same six models/ROIs and sampling. No threshold promotion, bounded optimizer or real fold recovery.'}
def save():
 temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save()
for case in prior['rows']:
 input=Path(case['input']);assert digest(input)==case['input_sha256'];result=subprocess.run([str(exe),str(left),str(right),str(input)],capture_output=True,text=True);assert digest(input)==case['input_sha256'] and all(digest(k)==v for k,v in pins.items())
 row={'model_index':case['model_index'],'domains':case['domains'],'matrix':case['matrix'],'input':str(input),'input_sha256':case['input_sha256'],'original_status':case['evidence']['status'],'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr}
 if result.returncode==0:
  e=json.loads(result.stdout);assert e['managed_used']==0;row['evidence']=e;row['supported_with_original_limits']=e['status']=='ok' and len(e['directions'])==2 and all(v['original_bounds_admitted'] and v['samples']>=1000 and v['matched']>=.9*v['samples'] for v in e['directions'])
  if case['evidence']['status']=='ok':
   assert e['status']=='ok'
   for current,before in zip(e['directions'],case['evidence']['directions']):
    assert current['original_bounds_admitted'] and all(current[k]==before[k] for k in ['radius','training_samples','samples','matched','pixel_reads'])
 report['rows'].append(row);save();print(case['model_index'],row.get('evidence',{}).get('status'),row.get('supported_with_original_limits'),flush=True)
report['status']='complete_native_diagnostic';save()
