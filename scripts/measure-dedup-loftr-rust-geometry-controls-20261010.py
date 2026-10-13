"""Wrong-source pixel controls with retained mosaic proposals; not model precision."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[1]
D=ROOT/'docs/research'
T=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
W=T/'native-loftr-inputs-20261010'
OUT=D/'dedup-loftr-rust-geometry-controls-20261010.json'
assert not OUT.exists()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
prior=D/'dedup-loftr-rust-geometry-20261010.json'
r=json.loads(prior.read_text())
assert r['status']=='complete_rust_geometry_diagnostic'
binary=T/'supplied-anchor-files-probe-qualified-20261010'
target=T/'original-resolution-strong-all/209401.jpg'
sources=[T/'original-resolution-negative-200201'/name for name in ('201400.jpg','209000.jpg')]
files=[prior,binary,target,Path(__file__),*sources]
for i in range(4):
    for suffix in ('model.txt','points.txt'):
        p=W/f'{i}-rust-geometry-qualified.{suffix}'
        assert sha(p)==r['pins'][str(p)]
        files.append(p)
pins={str(p):sha(p) for p in files}
report=dict(status='running',pins=pins,results=[],scope=__doc__)
def save():
    temp=OUT.with_suffix('.json.tmp');temp.write_text(json.dumps(report,indent=2)+'\n');temp.replace(OUT)
save()
try:
    for source in sources:
        for i in range(4):
            p=subprocess.run([str(binary),str(source),str(target),str(W/f'{i}-rust-geometry-qualified.model.txt'),str(W/f'{i}-rust-geometry-qualified.points.txt'),'5.157305482252365'],capture_output=True,text=True)
            row=dict(source=str(source),variant=i,returncode=p.returncode,stdout=p.stdout,stderr=p.stderr)
            report['results'].append(row);save()
            assert p.returncode==0
            row['evidence']=json.loads(p.stdout)
            assert row['evidence']['status']=='ok' and row['evidence']['supported'] is False
            assert 'memory_used=0 ' in p.stderr
            save()
    assert all(sha(p)==h for p,h in pins.items())
    report['status']='verified_eight_wrong_source_pixel_rejections'
except Exception as error:
    report.update(status='failed_wrong_source_pixel_controls',error=str(error));raise
finally:
    save()
print(report['status'])
