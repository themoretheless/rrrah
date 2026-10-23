import importlib.util,json,sys,tempfile,types
from pathlib import Path
root=Path('/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/sift497-20261023/viz/sift_viz')
pkg=types.ModuleType('sift_viz');pkg.__path__=[str(root)];sys.modules['sift_viz']=pkg
from sift_viz.models import HashSettings,ProjectionResult,ClusterResult,ViewerState
from sift_viz.cache import CacheManager
with tempfile.TemporaryDirectory(prefix='sift-cache-review-') as td:
 base=Path(td)
 class LocalCache(CacheManager):
  def cache_dir(self,folder):return base/'cache'
 manager=LocalCache();source=base/'source.json';source.write_text('old-hashes')
 proj=ProjectionResult(files=['a'],points=[[0,0]],method='pca');clusters=ClusterResult(files=['a'])
 manager.save('folder',HashSettings(),proj,None,clusters,source)
 source.unlink();manager.save('folder',HashSettings(algo='phash'),proj,None,clusters,source)
 assert manager.load('folder').settings.algo=='phash'
 assert (base/'cache/hashes.json').read_text()=='old-hashes'
 (base/'cache/hashes.json').write_text('{broken')
 assert manager.load('folder') is not None and manager.load_results('folder') is not None
 dest=base/'copied';assert manager.copy_hashes_to('folder',dest) and dest.read_text()=='{broken'
 wrong=ProjectionResult(files=['a','b'],points=[[0]],method='pca');assert len(wrong.files)!=len(wrong.points)
 bad=ClusterResult(files=['a'],groups=[{'members':[99]}]);assert bad.groups[0].members==[99]
 print('stale hash retained under changed settings; corrupt hash accepted/copied; projection cardinality and group indices unvalidated')
