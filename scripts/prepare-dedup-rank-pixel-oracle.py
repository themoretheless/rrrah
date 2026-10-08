"""Dump exact native normalized pixels; independent rank math consumes them."""
import hashlib,json,shutil,subprocess
from pathlib import Path
base=Path('docs/research');root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays');digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
reference=base/'dedup-filtered-rank-real.json';r=json.loads(reference.read_text());assert r['status']=='complete_native_filtered_rank_diagnostic'
sources=sorted({row[key] for row in r['rows'] for key in ['source','query']});assert len(sources)==8
for path in sources:assert digest(path)==r['input_hashes'][path]
exe=root/'rank-pixel-dump';assert not exe.exists();shutil.copy2('target/debug/examples/rank_pixel_dump',exe)
directory=root/'rank-normalized-pixel-oracle';directory.mkdir()
paths=[reference,exe,Path(__file__),Path('crates/rrrah-dedup/examples/rank_pixel_dump.rs'),Path('crates/rrrah-dedup/src/decode.rs'),Path('crates/rrrah-dedup/src/linear.rs'),*[Path(p) for p in sources]]
pins={str(p):digest(p) for p in paths};output=base/'dedup-rank-normalized-pixels.json';assert not output.exists();report={'status':'running','input_hashes':pins,'required_images':8,'rows':[],'scope':'Exact pixels from the same native selected-frame decode/view API and decode caps as the rank probe. Decoder is shared, not an independent normalization oracle; public Copydays fixtures only.'}
def save():
 temporary=output.with_suffix('.tmp');temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(output)
save()
magic=b'RRRAH-RANK-RGBA32-V1\n'
for path in sources:
 destination=directory/(Path(path).stem+'.rgba32');assert not destination.exists()
 result=subprocess.run([str(exe),path,str(destination)],capture_output=True,text=True);assert all(digest(k)==v for k,v in pins.items())
 row={'source':path,'source_sha256':digest(path),'pixels':str(destination),'returncode':result.returncode,'stdout':result.stdout,'stderr':result.stderr};report['rows'].append(row);save();assert result.returncode==0
 e=json.loads(result.stdout);assert e['managed_used']==0 and e['width']*e['height']<=6400000
 with destination.open('rb') as file:
  assert file.read(len(magic))==magic;assert int.from_bytes(file.read(4),'little')==e['width'];assert int.from_bytes(file.read(4),'little')==e['height']
 assert destination.stat().st_size==len(magic)+8+e['width']*e['height']*16
 row.update(evidence=e,pixels_sha256=digest(destination));save();print(Path(path).name,e['width'],e['height'],flush=True)
report['status']='complete_native_normalized_pixel_dump';save()
