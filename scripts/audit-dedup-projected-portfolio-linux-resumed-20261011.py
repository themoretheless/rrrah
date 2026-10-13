#!/usr/bin/env python3
"""Archive the completed frozen projected-portfolio Linux gate before target reuse."""
import gzip,hashlib,json,re,shutil
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];D=ROOT/'docs/research'
MANIFEST=D/'dedup-projected-portfolio-snapshot-20261011.json'
LOG=D/'dedup-projected-portfolio-linux-resumed-20261011.log'
OUT=D/'dedup-projected-portfolio-linux-resumed-audit-20261011.json'
ARCHIVE=Path('/Users/themoretheless/.codex/tmp/rrrah-projected-portfolio-linux-resumed-qualified-20261011')
BASE=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-linux-native')
HELPER=Path('/Users/themoretheless/.codex/tmp/rrrah-projected-portfolio-linux-20261011.sh')
EXPECTED={'lib':72,'local_index':7,'model_compare':11,'model_input':9,'model_input_file':10,
 'model_projected_compare':8,'model_tiles':13,'portfolio_collection':10,'proposals':6,'search_portfolio':13,
 'anchor_rank_file':3,'anchor_candidate_search':1,'anchor_fallback_search':1,'anchor_geometry_search':1,
 'anchor_reflected_geometry':1,'anchor_geometry_rank_fallback':1,'anchor_reflection_fallback':12,
 'anchor_fallback_collection_lifecycle':1,'anchor_reflection_collection_lifecycle':1,
 'native_anchor_recovery':2,'model_projected_tiles':8,'projected_collection':8,'native_pixel_domain':3,'gradient':9,'gradient_candidate_smoothing':3,'rank_region':7,'local_rank_cached':3,'mesh_rank':2,'local_rank':5,'gradient_interpolated':4,'projected_portfolio':12}
def sha(p):
 h=hashlib.sha256()
 with Path(p).open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def main():
 assert not OUT.exists() and not ARCHIVE.exists()
 text=LOG.read_text();assert 'test result: FAILED' not in text
 runs=re.findall(r'Running (?:unittests src/lib\.rs|tests/([^\s]+)\.rs) \(([^)]+)\)(.*?)(?=\n\s*Running |\Z)',text,re.S)
 observed={}
 for name,_,body in runs:
  name=name or 'lib';counts=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',body)
  assert len(counts)==1 and name not in observed,(name,counts);observed[name]=int(counts[0])
 assert observed==EXPECTED,observed
 m=json.loads(MANIFEST.read_text());snapshot=Path(m['snapshot'])
 def verify():assert all(sha(snapshot/p)==h for p,h in m['file_hashes'].items())
 verify();ARCHIVE.mkdir();artifacts=[]
 for name,binary,_ in runs:
  source=BASE/Path(binary).relative_to('/validation');digest=sha(source)
  with source.open('rb') as f:assert f.read(4)==b'\x7fELF'
  target=ARCHIVE/(source.name+'.gz')
  with source.open('rb') as f,target.open('wb') as out:
   with gzip.GzipFile(filename='',mode='wb',fileobj=out,compresslevel=1,mtime=0) as z:shutil.copyfileobj(f,z,1024*1024)
  h=hashlib.sha256()
  with gzip.open(target,'rb') as f:
   for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
  assert h.hexdigest()==digest==sha(source)
  artifacts.append({'suite':name or 'lib','source':str(source),'archive':str(target),
   'executable_sha256':digest,'archive_sha256':sha(target),'executable_bytes':source.stat().st_size})
 pins={}
 for p in [MANIFEST,LOG,HELPER,Path(__file__)]:
  pins[str(p)]=sha(p);shutil.copy2(p,ARCHIVE/p.name);assert sha(ARCHIVE/p.name)==pins[str(p)]
 verify()
 report={'status':'complete_bounded_frozen_projected_portfolio_linux_gate','tests':sum(observed.values()),
  'programs':len(artifacts),'suite_counts':observed,'ignored_tests':sum(int(n) for n in re.findall(r'test result: ok\. \d+ passed; 0 failed; (\d+) ignored;',text)),'frozen_files':len(m['file_hashes']),
  'artifacts':artifacts,'pins':pins,
  'scope':'Linux aarch64 frozen decode-feature generic/native-anchor tests; artifacts archived before Linux target reuse',
  'limits':['Bounded fixtures do not establish broad collection recall or precision',
            'No Torch, Windows, broad negative/burst or hard total RSS qualification',
            'Full229 uses an earlier independent frozen source']}
 OUT.write_text(json.dumps(report,indent=2)+'\n');print(report['status'],report['tests'])
if __name__=='__main__':main()
