"""Fresh geometry gate on another observed unrelated color-only support."""
import hashlib,json,subprocess
from pathlib import Path
base=Path('docs/research');root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
qualified=base/'dedup-native-screen-affine-color-search-proof.json';q=json.loads(qualified.read_text());assert q['status']=='terminal' and q['returncode']==0
provenance={};resolved=[]
for name,expected in q['input_hashes'].items():
    path=Path(name)
    if digest(path)!=expected:
        preserved=base/'dedup-affine-radius16-source-snapshot'/path.name
        if preserved.is_file() and digest(preserved)==expected:
            path=preserved
        else:
            path=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-affine-region-linux-snapshot-20261007')/name
    assert digest(path)==expected,name
    provenance[name]={'qualified_sha256':expected,'preserved_path':str(path)}
    resolved.append(path)
exe=root/'affine-color-search-proof-probe-screen-release';left=root/'original-resolution-negative-200201/203100.jpg';right=root/'original-resolution-strong-all/200501.jpg'
live=json.loads((base/'dedup-native-screen-affine-color-grid-negatives-meanfix.json').read_text())
counter=next(v for v in live['rows'] if Path(v['source']).name=='203100.jpg');assert counter['returncode']==0 and counter['supports']==2 and digest(left)==counter['source_sha256']
snapshot=base/'dedup-affine-grid-203100-counterexample.json';assert not snapshot.exists();snapshot.write_text(json.dumps({'status':'observed_unrelated_color_region_support','source_row':counter,'scope':'Different publisher-origin input under supplied normalized geometry. Color-only support is not a copy decision.'},indent=2)+'\n')
prepared=root/'prepared.json';manifest=json.loads(prepared.read_text())
for path,split in [(left,'original'),(right,'strong')]:assert digest(path)==next(v['source_sha256'] for v in manifest['images'][split] if v['filename']==path.name)
pins={str(p):digest(p) for p in [qualified,snapshot,exe,left,right,prepared,Path(__file__),*resolved]}
output=base/'dedup-affine-grid-203100-native-search.json';assert not output.exists();report={'status':'running','source':str(left),'query':str(right),'input_hashes':pins,'qualified_source_provenance':provenance,'scope':'Fresh native candidate geometry without supplied model/ROI on one observed unrelated color-only support. Uses preserved radius16 binary/source revision, not current source-footprint API or all-query precision.'}
def save():
    temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save();result=subprocess.run([str(exe),str(left),str(right)],capture_output=True,text=True);assert all(digest(k)==v for k,v in pins.items());report.update(returncode=result.returncode,stdout=result.stdout,stderr=result.stderr)
if result.returncode==0:
    evidence=json.loads(result.stdout);assert evidence['managed_used']==0;report.update(status='terminal',evidence=evidence)
else:report['status']='native_failure'
save();print({k:v for k,v in report.get('evidence',{}).items() if k!='detail'});raise SystemExit(result.returncode)
