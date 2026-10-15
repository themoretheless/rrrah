"""Selected unchanged upstream bodies with explicit service adapters, owned fixtures only."""
from pathlib import Path
from types import SimpleNamespace as NS
from typing import List
import ast,datetime,filecmp,hashlib,json,logging,os,re,shutil,tempfile
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/image37-20261016';S=W/'source';h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
class Metric:
 def inc(self):pass
 def time(self):return lambda f:f
class Progress:
 def __getattr__(self,k):return lambda *a,**kw:None
class Store:
 def __init__(self):self.removed=[]
 def remove(self,p):self.removed.append(str(p))
ns={'__builtins__':__builtins__,'Path':Path,'List':List,'datetime':datetime,'os':os,'shutil':shutil,'filecmp':filecmp,'logging':logging,'echo':lambda *a,**kw:None,'FIND_DUPLICATES_TIME':Metric(),'DUPLICATE_ACTION_DELETE_COUNT':Metric(),'DUPLICATE_ACTION_MOVE_COUNT':Metric()}
exec((S/'py_image_dedup/persistence/metadata_key.py').read_text(),ns)
config=NS(PRIORITIZATION_RULES=NS(value=[{'name':'bigger-file-size'}]));ns['DeduplicatorConfig']=config
ns['file']=NS(get_file_name=lambda p:os.path.split(p)[1],get_containing_folder=lambda p:os.path.split(p)[0])
tree=ast.parse((S/'py_image_dedup/library/deduplicator.py').read_text());cls=next(x for x in tree.body if isinstance(x,ast.ClassDef));selected=['find_duplicates_of_file','_select_images_to_delete','_sort_by_quality_descending','_delete_files','_move_files','_truncate_middle'];cls.body=[x for x in cls.body if isinstance(x,(ast.FunctionDef,ast.AsyncFunctionDef)) and x.name in selected];assert len(cls.body)==len(selected);exec(compile(ast.Module(body=[cls],type_ignores=[]),'<unchanged upstream selected methods>','exec'),ns);C=ns['ImageMatchDeduplicator']
file_ns={};exec((S/'py_image_dedup/util/file.py').read_text(),file_ns)
observations=[]
def record(name,**data):observations.append({'control':name,**data})
def make():
 o=C.__new__(C);o._progress_manager=Progress();o._persistence=Store();o._processed_files={};o._config=NS(SEARCH_ACROSS_ROOT_DIRS=NS(value=False),MAX_FILE_MODIFICATION_TIME_DELTA=NS(value=None));return o
def candidate(p,size):return {'path':str(p),'dist':0,'score':64,'metadata':{'filesize':size,'file_modification_date':1,'pixelcount':1,'exif_data':{}}}
with tempfile.TemporaryDirectory(prefix='image37-owned-') as tmp:
 root=Path(tmp);allowed=root/'allowed';outside=root/'outside';allowed.mkdir();outside.mkdir();a=allowed/'a.png';b=allowed/'b.png';c=outside/'c.png';a.write_bytes(b'original');b.write_bytes(b'original');c.write_bytes(b'unique outside')
 o=make();queries=[];ca,cb,cc=candidate(a,20),candidate(b,10),candidate(c,1)
 def query(p):
  queries.append(p);return [ca,cb] if len(queries)==1 else [ca,cb,cc]
 o._persistence.find_similar=query;saved=[];o._save_duplicates_for_result=lambda keep,delete:saved.append((keep,delete));o.find_duplicates_of_file([allowed],allowed,a);assert len(queries)==2 and cc in saved[0][1];assert c.exists();record('second_query_escapes_allowed_root',outside_marked=True,actual_outside_deleted=False,adapter='explicit two-response storage; no real Elasticsearch/image matching')
 o=make();a.write_bytes(b'new unique content after matching');o._delete_files([a],False);assert not a.exists();record('post_match_rewrite_deleted',remaining=False)
 a.write_bytes(b'keeper');b.write_bytes(b'copy');a.unlink();o=make();o._delete_files([b],False);assert not a.exists() and not b.exists();record('last_copy_deleted_after_keeper_removed',remaining_copies=0)
 b.write_bytes(b'preserve');o=make();o._delete_files([b],True);assert b.read_bytes()==b'preserve' and not o._persistence.removed;record('dry_run_preserves_bytes_and_storage',preserved=True)
 o=make();o._delete_files([root/'missing'],False);assert len(o._persistence.removed)==1;record('missing_file_removes_storage_entry',removed_entries=1)
 src=root/'moving';src.write_bytes(b'abcd');target=root/'target';dst=Path(str(target),*src.parts[1:]);dst.parent.mkdir(parents=True);dst.write_bytes(b'abcd');filecmp.clear_cache();assert filecmp.cmp(src,dst,shallow=False);st=src.stat();src.write_bytes(b'wxyz');os.utime(src,ns=(st.st_atime_ns,st.st_mtime_ns));assert src.stat().st_mtime_ns==st.st_mtime_ns and src.read_bytes()!=dst.read_bytes();o=make();o._move_files([src],target,False);assert not src.exists() and dst.read_bytes()==b'abcd';filecmp.clear_cache();record('native_filecmp_cached_equality_deletes_changed_source',same_size_and_restored_mtime=True,source_removed=True,target_original=True)
 assert file_ns['file_has_extension'](Path('photo.PNG'),['.png']);record('case_insensitive_extension',accepted=True)
 try:file_ns['file_has_extension'](Path('photo.png'),None)
 except AttributeError:record('documented_none_extension_filter_raises',exception='AttributeError')
 else:raise AssertionError('expected none filter failure')
 nested=allowed/'nested';nested.mkdir();(nested/'excluded.png').write_bytes(b'fixture');count=file_ns['get_files_count'](allowed,True,['.png'],[re.compile(re.escape(str(nested)))]);assert count==2;record('counter_exclusions_use_cwd_basename_instead_of_walk_root',excluded_file_counted=True)
 o=make()
 try:o._select_images_to_delete([])
 except IndexError:record('empty_second_search_not_guarded',exception='IndexError')
 else:raise AssertionError('expected empty selection failure')
pins={str(p.relative_to(S)):h(p) for p in S.rglob('*') if p.is_file()};print(json.dumps({'status':'passed_selected_native_controls','controls':observations,'selected_methods':selected,'source_pins':pins,'scope':'Selected unchanged AST method bodies; no original suite/full package/image signature/Elasticsearch/daemon/dependency/platform/quality/performance qualification. All file actions on owned temporary fixtures; explicit storage/config/progress/metric bindings. Remaining text and all12images still require review.'},sort_keys=True))
