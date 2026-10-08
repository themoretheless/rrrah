"""Real proof report controls: all deliberate evidence corruptions must fail."""
import copy,hashlib,json,subprocess,tempfile
from pathlib import Path
source=Path('docs/research/dedup-native-screen-affine-color-search-proof.json')
verifier=Path('scripts/verify-dedup-native-screen-affine-color-search-proof.py')
helper=Path('scripts/dedup_distinct_points.py')
output=Path('docs/research/dedup-native-search-proof-auditor-controls.json');assert not output.exists()
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
original=json.loads(source.read_text());assert original['status']=='terminal'
pins={str(p):digest(p) for p in [source,verifier,helper,Path(__file__)]}
def duplicate_point(e):
    e['detail']['correspondences'][-1]=copy.deepcopy(e['detail']['correspondences'][0])
def missing_inlier(e):
    e['detail']['inlier_ids'].pop();e['inliers']-=1
def false_residual(e):
    point=e['detail']['correspondences'][e['detail']['inlier_ids'][0]]
    point['target'][0]+=10 if point['target'][0]<450 else -10
def shifted_domain(e):
    e['detail']['regions'][0]['domains'][0][0]+=1
def missing_cell(e):
    e['detail']['regions'].pop();e['regions']-=1
def radius(e):
    e['detail']['regions'][0]['source_radius']+=1
def matched(e):
    for region in e['detail']['regions']:
        for direction in region['directions']:
            if 'matched' in direction:
                direction['matched']=direction['samples']+1
                return
    raise AssertionError('real proof has no valid color directions')
def supports(e):e['supports']+=1
cases=[('valid',None),('duplicate_point',duplicate_point),('missing_inlier',missing_inlier),('false_residual',false_residual),('shifted_domain',shifted_domain),('missing_cell',missing_cell),('radius',radius),('matched_count',matched),('false_support',supports)]
results=[]
with tempfile.TemporaryDirectory(prefix='rrrah-search-proof-controls-') as folder:
    for name,mutation in cases:
        report=copy.deepcopy(original)
        if mutation:
            mutation(report['evidence']);report['stdout']=json.dumps(report['evidence'])
        path=Path(folder)/(name+'.json');audit=Path(folder)/(name+'-audit.json')
        path.write_text(json.dumps(report))
        result=subprocess.run(['python3',str(verifier),str(path),str(audit)],capture_output=True,text=True)
        assert (result.returncode==0)==(mutation is None),(name,result.stderr[-1500:])
        assert audit.exists()==(mutation is None)
        results.append({'case':name,'returncode':result.returncode})
        print(name,result.returncode,flush=True)
assert all(digest(k)==v for k,v in pins.items())
output.write_text(json.dumps({'status':'passed','input_hashes':pins,'controls':results,'scope':'One real valid report and eight malformed-report controls. No independent pixel resampling, upstream algorithm correctness or full case coverage claim.'},indent=2)+'\n')
