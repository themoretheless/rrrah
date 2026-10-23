import ast,logging,sys,time,types
from pathlib import Path
root=Path('/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/sift497-20261023/viz/sift_viz')
pkg=types.ModuleType('sift_viz');pkg.__path__=[str(root)];sys.modules['sift_viz']=pkg
from sift_viz.models import HashSettings,CacheMeta,ProjectionResult,ClusterResult
module=ast.parse((root/'app.py').read_text());cls=next(x for x in module.body if isinstance(x,ast.ClassDef))
def extract(name):
 f=next(x for x in cls.body if isinstance(x,ast.FunctionDef) and x.name==name)
 for a in f.args.args:a.annotation=None
 f.returns=None;env={'Path':Path,'time':time,'fmt_ago':lambda s:'now','log':logging.getLogger('control')};exec(compile(ast.fix_missing_locations(ast.Module(body=[f],type_ignores=[])),str(root/'app.py'),'exec'),env);return env[name]
proj=ProjectionResult(files=['old.png'],points=[[0,0]],method='pca');cluster=ClusterResult(files=['old.png']);meta=CacheMeta(saved_at=time.time(),folder='folder',settings=HashSettings())
class Cache:
 def __init__(self):self.saved=None;self.copy_attempted=False
 def save(self,**kwargs):self.saved=kwargs
 def load(self,folder):return meta
 def load_results(self,folder):return proj,None,cluster
 def copy_hashes_to(self,*args):self.copy_attempted=True;return False
cache=Cache();noop=lambda *a,**k:None
s=types.SimpleNamespace(folder_var=types.SimpleNamespace(get=lambda:'folder'),_projection_2d=proj,_projection_3d=None,_cluster_data=cluster,_cache=cache,tmpdir='/unused-controlled-path',_settings_from_ui=lambda:HashSettings(algo='phash'),_restore_settings=noop,_update_tsne_visibility=noop,_update_frames_visibility=noop,_rebuild_cluster_params=noop,_on_cluster_done=noop,_set_status=noop)
extract('_save_cache')(s);assert cache.saved['settings'].algo=='phash' and cache.saved['proj_2d'].files==['old.png']
assert extract('_load_cache')(s,'folder') is True and cache.copy_attempted
print('original app methods: save labels existing results with current settings; cache load succeeds despite failed hash copy')
