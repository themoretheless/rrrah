"""Fresh native original-file anchors for all four frozen learned proposal variants."""
import hashlib,json,shutil,subprocess,time
from pathlib import Path
D=Path('docs/research');T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
out=D/'dedup-209401-loftr-native-anchors.json';folder=T/'209401-loftr-native-anchors-20261010';binary=T/'supplied-anchor-files-probe-qualified-20261010'
assert not out.exists() and not folder.exists() and not binary.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
proposals=D/'dedup-209401-loftr-diagnostic.json';r=json.loads(proposals.read_text());assert r['status']=='complete_proposal_diagnostic' and len(r['rows'])==4
assert all(sha(p)==h for p,h in r['pins'].items())
manifest=D/'dedup-supplied-anchor-snapshot-20261010.json';m=json.loads(manifest.read_text());assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
shutil.copy2('target/debug/examples/anchor_rank_supplied_files_probe',binary);folder.mkdir()
inputs=[T/'original-resolution-negative-200201/209400.jpg',T/'original-resolution-strong-all/209401.jpg'];cases=[]
for i,row in enumerate(r['rows']):
    assert row['matrix'] is not None
    model=folder/f'{i}-model.txt';points=folder/f'{i}-points.txt'
    model.write_text(' '.join(repr(v) for line in row['matrix'] for v in line)+'\n')
    points.write_text(''.join(' '.join(repr(v) for xy in p for v in xy)+'\n' for p in row['distinct_points']))
    cases.append((model,points))
pins={str(p):sha(p) for p in inputs+[proposals,manifest,binary,Path(__file__),D/'dedup-supplied-anchor-build-20261010.log']+[p for case in cases for p in case]}
report=dict(status='running',pins=pins,source_manifest=str(manifest),source_tolerance=r['source_tolerance'],results=[],scope='All externally proposed original-coordinate points and models passed to existing Rust original-file confirmation with unchanged rank8/filter8/.005/1000/.3/.9/min10, target2/source-scaled geometry and 512 MiB. No automatic native learned extraction, unrelated precision or collection recall claim.')
def save():
    temp=out.with_suffix('.json.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(out)
save()
try:
    for i,(model,points) in enumerate(cases):
        started=time.monotonic();p=subprocess.run([str(binary),*map(str,inputs),str(model),str(points),str(r['source_tolerance'])],capture_output=True,text=True)
        row=dict(variant=i,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr,elapsed_seconds=time.monotonic()-started)
        if p.returncode==0:row['evidence']=json.loads(p.stdout)
        report['results'].append(row);save();print(i,row.get('evidence',{}),flush=True)
    assert all(sha(p)==h for p,h in pins.items())
    assert all(sha(Path(m['snapshot'])/p)==h for p,h in m['file_hashes'].items())
    report['status']='complete_native_supplied_anchor_diagnostic'
finally:
    save()
