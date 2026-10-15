"""Whole upstream store/progress/event bodies with explicit service bindings and real Pillow."""
from pathlib import Path
from types import SimpleNamespace as NS,ModuleType
from typing import List
from PIL import Image,TiffImagePlugin
import ast,hashlib,json,logging,os,re,sys,tempfile
R=Path(__file__).resolve().parents[1];S=R/'.dedup-work/image37-20261016/source';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();ns={'os':os,'logging':logging,'TiffImagePlugin':TiffImagePlugin};exec((S/'py_image_dedup/persistence/metadata_key.py').read_text(),ns)
# Bind the original dynamic util.image import to its whole unchanged implementation.
img=ModuleType('py_image_dedup.util.image');exec((S/'py_image_dedup/util/image.py').read_text(),img.__dict__);util=ModuleType('py_image_dedup.util');util.image=img;pkg=ModuleType('py_image_dedup');pkg.util=util;sys.modules['py_image_dedup']=pkg;sys.modules['py_image_dedup.util']=util
source=S/'py_image_dedup/persistence/__init__.py';tree=ast.parse(source.read_text());cl=next(x for x in tree.body if isinstance(x,ast.ClassDef));exec(compile(ast.Module(body=[cl],type_ignores=[]),str(source),'exec'),ns);Base=ns['ImageSignatureStore']
class Store(Base):
 def __init__(self):super().__init__(False);self.saved=None;self.writes=[]
 def get(self,p):return self.saved
 def _add(self,p,data):self.writes.append(dict(data));self.saved={'path':p,'metadata':dict(data)}
 def remove(self,p):self.saved=None
obs=[]
def record(name,**d):obs.append({'control':name,**d})
with tempfile.TemporaryDirectory(prefix='image37-metadata-owned-') as tmp:
 p=Path(tmp)/'image.bmp';Image.new('RGB',(12,12),'red').save(p);o=Store();o.add(str(p));assert len(o.writes)==1;first=p.read_bytes();st=p.stat();Image.new('RGB',(12,12),'blue').save(p);assert p.read_bytes()!=first and p.stat().st_size==st.st_size;os.utime(p,ns=(st.st_atime_ns,st.st_mtime_ns));assert p.stat().st_mtime_ns==st.st_mtime_ns;o.add(str(p));assert len(o.writes)==1;record('same_size_restored_mtime_skips_changed_actual_image',writes=1,pixels_changed=True)
 os.utime(p,ns=(st.st_atime_ns,st.st_mtime_ns+2_000_000_000));o.add(str(p));assert len(o.writes)==2;record('mtime_change_refreshes_metadata',writes=2)
 o.saved['metadata']['py-image-dedup_datamodel-version']=4;o.add(str(p));assert len(o.writes)==3;record('model_version_change_refreshes',writes=3)
 invalid=Path(tmp)/'invalid.png';invalid.write_bytes(b'not an image');assert img.get_pixel_count(str(invalid))==0 and img.get_exif_data(str(invalid))=={};record('unreadable_image_metadata_returns_zero_and_empty',pixel_count=0)
 source=S/'py_image_dedup/library/progress_manager.py';t=ast.parse(source.read_text());cl=next(x for x in t.body if isinstance(x,ast.ClassDef));n={'LOGGER':logging.getLogger('image37-control'),'tqdm':object};exec(compile(ast.Module(body=[cl],type_ignores=[]),str(source),'exec'),n);progress=n['ProgressManager']();progress.start('empty',0,'files',False)
 try:progress.inc()
 except ZeroDivisionError:record('zero_total_progress_increment_raises',exception='ZeroDivisionError')
 else:raise AssertionError('expected zero total refusal')
 progress.clear();progress.start('two',2,'files',False);progress.inc();progress.inc();assert progress._n==2;progress.clear();assert progress._n is None and progress._task is None;record('nonzero_progress_and_reset',passed=True)
 source=S/'py_image_dedup/library/file_watch.py';t=ast.parse(source.read_text());cl=next(x for x in t.body if isinstance(x,ast.ClassDef));config=NS(SOURCE_DIRECTORIES=NS(value=[Path(tmp)/'photos']),FILE_EXTENSION_FILTER=NS(value=['.png']));n={'FileSystemEventHandler':object,'DeduplicatorConfig':lambda:config,'re':re,'Path':Path};exec(compile(ast.Module(body=[cl],type_ignores=[]),str(source),'exec'),n);handler=n['EventHandler'](None)
 assert handler._event_matches_filter(NS(is_directory=False,src_path=str(Path(tmp)/'photos-other'/'image.png')));record('watch_regex_accepts_sibling_prefix',accepted=True,adapter='event object only; no watcher')
 assert handler._event_matches_filter(NS(is_directory=False,src_path=str(Path(tmp)/'photos'/'imageXpng')));record('watch_regex_extension_dot_is_wildcard',accepted=True)
 assert not handler._event_matches_filter(NS(is_directory=True,src_path=str(Path(tmp)/'photos')));record('directory_event_filtered',accepted=False)
 pins={str(p.relative_to(S)):h(p) for p in [S/'py_image_dedup/persistence/__init__.py',S/'py_image_dedup/persistence/metadata_key.py',S/'py_image_dedup/util/image.py',S/'py_image_dedup/library/progress_manager.py',S/'py_image_dedup/library/file_watch.py']}
 print(json.dumps({'status':'passed_selected_metadata_progress_event_controls','controls':obs,'source_pins':pins,'pillow_version':Image.__version__,'scope':'Whole unchanged selected classes and image module; real Pillow/stdlib file metadata. Store methods get/_add/remove are explicit in-memory adapters, progress is noninteractive, event base/event objects explicit adapters. No real Elasticsearch/image-match signature/daemon/watchdog/config/CLI/platform/performance qualification.'},sort_keys=True))
