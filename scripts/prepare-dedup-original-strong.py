"""Extract all pinned original-resolution strong JPEGs without renormalization."""
import hashlib,json,tarfile
from pathlib import Path
root=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays')
manifest=root/'prepared.json';archive=root/'copydays_strong.tar.gz'
output=Path('docs/research/dedup-original-strong-inputs.json');folder=root/'original-resolution-strong-all'
assert not output.exists() and not folder.exists()
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
archive_hash=h(archive);assert archive_hash=='7852c64aa4a37e8329ce08159fc947c16f09c66737db0a2479a90fc9ae40760c'
manifest_hash=h(manifest);m=json.loads(manifest.read_text());rows=m['images']['strong'];assert len(rows)==229
expected={r['source_member']:r for r in rows};assert len(expected)==229
folder.mkdir();records=[]
with tarfile.open(archive,'r:gz') as tar:
 for member in tar:
  if member.isdir():continue
  assert member.isfile() and member.name in expected
  row=expected.pop(member.name);assert row['source_archive_sha256']==archive_hash
  data=tar.extractfile(member).read();assert len(data)==member.size and hashlib.sha256(data).hexdigest()==row['source_sha256']
  dest=folder/row['filename'];assert dest.name==row['filename'] and not dest.exists();dest.write_bytes(data);dest.chmod(0o444)
  records.append({'name':row['filename'],'path':str(dest),'sha256':h(dest),'dimensions':row['source_size'],'group_id':row['group_id']})
assert not expected and len(records)==229 and h(archive)==archive_hash and h(manifest)==manifest_hash
folder.chmod(0o555)
output.write_text(json.dumps({'status':'verified_original_strong_archive_bytes','count':229,'records':sorted(records,key=lambda r:r['name']),'input_hashes':{str(archive):archive_hash,str(manifest):manifest_hash,str(Path(__file__)):h(__file__)},'scope':'All229 strong original JPEG bytes pinned to official archive and prepared publisher-origin manifest. No algorithm recall or pixel-equality proof.'},indent=2)+'\n')
print('229 original-resolution source JPEGs verified.')
