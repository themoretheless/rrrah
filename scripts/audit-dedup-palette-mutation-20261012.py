#!/usr/bin/env python3
import hashlib,json,re,shutil,zlib,io
from pathlib import Path
from PIL import Image
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 out=D/'dedup-palette-mutation-verification-20261012.json';copy=T/'rrrah-palette-mutation-qualified-20261012';assert not out.exists()and not copy.exists()
 parent=D/'dedup-exposure-recovery-search-snapshot-20261012.json';m=json.loads(parent.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==v for k,v in m['file_hashes'].items())
 h=T/'rrrah-palette-mutation-harness-20261012';test=R/'crates/rrrah-dedup/tests/palette_mutation.rs';assert sha(test)==sha(h/'tests/palette_mutation.rs');fixture=R/'crates/rrrah-dedup/tests/fixtures/palette-transparency/hidden-gradient.png';assert sha(fixture)==sha(h/'tests/fixtures/palette-transparency/hidden-gradient.png')
 original=fixture.read_bytes();changed=bytearray(original);offset=8;found=False
 while offset<len(original):
  length=int.from_bytes(original[offset:offset+4],'big');kind=original[offset+4:offset+8]
  assert zlib.crc32(original[offset+4:offset+8+length])==int.from_bytes(original[offset+8+length:offset+12+length],'big')
  if kind==b'tRNS':
   assert length==256 and original[offset+8:offset+8+length]==bytes(256);changed[offset+8+80]=255;changed[offset+8+length:offset+12+length]=zlib.crc32(changed[offset+4:offset+8+length]).to_bytes(4,'big');found=True
  offset+=length+12
 assert found and offset==len(original) and len(changed)==len(original)
 a=Image.open(io.BytesIO(original)).convert('RGBA');b=Image.open(io.BytesIO(changed)).convert('RGBA');assert sum(px[3]!=0 for px in a.getdata())==0 and sum(px[3]!=0 for px in b.getdata())==9
 specimen=T/'rrrah-palette-same-length-alpha-rewrite-20261012.png';assert not specimen.exists();specimen.write_bytes(changed)
 log=T/'rrrah-palette-mutation-tests-20261012.log';text=log.read_text();assert 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'in text and 'same_length_png_alpha_table_rewrite_invalidates_cache_and_collection_edges ... ok'in text
 binary=Path(re.search(r'Running tests/palette_mutation\.rs \(([^)]+)\)',text).group(1));shutil.copy2(binary,copy)
 paths=[parent,h/'Cargo.toml',h/'Cargo.lock',h/'tests/palette_mutation.rs',h/'tests/fixtures/palette-transparency/hidden-gradient.png',specimen,log,copy,Path(__file__)];out.write_text(json.dumps({'status':'verified_same_length_palette_alpha_mutation_cache_and_indexed_rescan','tests':1,'pins':{str(p):sha(p)for p in paths},'original_and_changed_bytes':len(original),'expected_before_equal':[[1,2],[1,3],[2,3]],'expected_after_equal':[[1,3]],'cache_sequence':['miss','hit','same_length_alpha_rewrite','miss','hit'],'independent_fixture_check':'Python zlib verifies every original chunk CRC, patches only tRNS entry80 and CRC without length change; Pillow confirms0→9 visible pixels. Native test independently constructs the rewrite, verifies cache miss/new fingerprint and indexed rescan removes changed-source edges.','scope':'Sequential repeated scans on unchanged184 production source, not during-scan mutation/metadata-indistinguishable race or allPNG/platform/RSS/full500 qualification. No fresh188 full gate or production fix claimed.'},indent=2)+'\n');print('Verified same-length alpha rewrite, cache invalidation and changed collection edges')
if __name__=='__main__':main()
