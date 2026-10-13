"""Supplement eight truncated archives with complete files from the same pinned commits."""
import concurrent.futures,hashlib,json,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];BASE=ROOT/'docs/research/dedup-500'
OUT=BASE/'full-truncated-source-20261011'
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert not OUT.exists();OUT.mkdir();items=[]
 for directory in sorted((BASE/'evidence').iterdir()):
  manifest_path=directory/'manifest.json'
  if not manifest_path.exists():continue
  manifest=json.loads(manifest_path.read_text());tree_path=directory/'tree.json';tree=json.loads(tree_path.read_text())
  entries={e['path']:e for e in tree['tree']}
  for file in manifest['files']:
   if file['truncated']:
    assert file['path'] in entries and entries[file['path']]['type']=='blob'
    assert sha(directory/file['local'])==file['sha256']
    items.append((directory,manifest_path,tree_path,manifest,file,entries[file['path']]))
 assert len(items)==8
 def fetch(item):
  directory,manifest_path,tree_path,manifest,file,entry=item
  url='https://raw.githubusercontent.com/'+manifest['repository']+'/'+manifest['commit']+'/'+urllib.parse.quote(file['path'],safe='/')
  request=urllib.request.Request(url,headers={'User-Agent':'rrrah-source-research'})
  with urllib.request.urlopen(request,timeout=45) as stream:data=stream.read()
  blob=hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest();assert blob==entry['sha'],(file['path'],blob,entry['sha'])
  if 'size' in entry:assert len(data)==entry['size']
  destination=OUT/(manifest['repository'].replace('/','--')+'--'+file['local']);destination.write_bytes(data)
  return {'repository':manifest['repository'],'commit':manifest['commit'],'path':file['path'],'url':url,'archive':str(destination),'sha256':sha(destination),'git_blob':blob,'bytes':len(data),'lines':len(data.splitlines()),'original_excerpt':str(directory/file['local']),'manifest':str(manifest_path),'manifest_sha256':sha(manifest_path),'tree':str(tree_path),'tree_sha256':sha(tree_path)}
 with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:rows=list(pool.map(fetch,items))
 report={'status':'complete_eight_full_pinned_files','scope':__doc__,'files':rows,'producer':str(Path(__file__).resolve()),'producer_sha256':sha(Path(__file__)),'limits':['Full-byte acquisition closes truncation only; source findings need separate review','Original 500 manifests and reviews remain unchanged','No upstream tests, execution, licenses or full-repository algorithm qualification implied']}
 (OUT/'manifest.json').write_text(json.dumps(report,indent=2)+'\n');print(report['status'],sum(r['bytes'] for r in rows))
if __name__=='__main__':main()
