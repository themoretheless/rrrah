"""Complete original module with declared Sublime UI adapter, real owned filesystem."""
from pathlib import Path
import sys,types,importlib.util,json,hashlib
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/sidebar55-20261016';S=W/'source';A=W/'native-controls';A.mkdir(exist_ok=False)
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
class Settings:
 def __init__(self,d):self.d=d
 def get(self,k,default=None):return self.d.get(k,default)
class View:
 def __init__(self,p):self.p=p;self.targets=[]
 def file_name(self):return self.p
 def settings(self):return Settings({})
 def retarget(self,p):self.targets.append(p);self.p=p
class Window:
 def __init__(self):self.messages=[];self.opened=[];self.commands=[];self.roots=[];self.v=[];self.project={'folders':[]}
 def status_message(self,x):self.messages.append(x)
 def open_file(self,x):self.opened.append(x)
 def run_command(self,*x):self.commands.append(x)
 def folders(self):return self.roots
 def active_view(self):return self.v[0] if self.v else None
 def views(self):return self.v
 def project_data(self):return self.project
class WindowCommand:
 def __init__(self,w):self.window=w
s=types.ModuleType('sublime');s.defaults={'difftool':'owned-diff','tab_context':True};s.clipboard='';s.dialogs=[];s.ws=[]
s.load_settings=lambda _:Settings(s.defaults);s.set_clipboard=lambda x:setattr(s,'clipboard',x);s.message_dialog=lambda x:s.dialogs.append(x);s.windows=lambda:s.ws;s.platform=lambda:'osx';s.version=lambda:'4158'
p=types.ModuleType('sublime_plugin');p.WindowCommand=WindowCommand;p.EventListener=object;sys.modules.update(sublime=s,sublime_plugin=p)
spec=importlib.util.spec_from_file_location('upstream_sidebar55',S/'SideBar.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
runs=[]
for n in range(2):
 b=A/f'run{n}';b.mkdir();w=Window();s.ws=[w];w.v=[View(str(b/'active.txt'))];rows=[]
 def check(name,value,want):assert value==want,(name,value,want);rows.append({'case':name,'observed':value})
 src=b/'source.txt';src.write_text('SOURCE');dst=b/'destination.txt';dst.write_text('ORIGINAL DESTINATION')
 m.SideBarDuplicateCommand(w).copy(str(src),str(dst));check('duplicate_overwrites_existing_file',[src.read_text(),dst.read_text(),w.opened[-1]==str(dst)],['SOURCE','SOURCE',True])
 directory=b/'directory';directory.mkdir();(directory/'child.txt').write_text('CHILD');existing=b/'existing';existing.mkdir();(existing/'retain.txt').write_text('RETAIN');m.SideBarDuplicateCommand(w).copy(str(directory),str(existing));check('duplicate_existing_directory_errors_preserves_destination',[(existing/'retain.txt').read_text(),(existing/'child.txt').exists(),'Error copying' in w.messages[-1]],['RETAIN',False,True])
 link=b/'link.txt';link.symlink_to(src.resolve());copy=b/'link-copy.txt';m.SideBarDuplicateCommand(w).copy(str(link),str(copy));check('duplicate_dereferences_owned_symlink',[copy.read_text(),copy.is_symlink()],['SOURCE',False])
 root=b/'root';root.mkdir();outside=b/'root-extra/file.txt';w.roots=[str(root)];m.SideBarCopyRelativePathCommand(w).run([str(outside)]);check('relative_path_uses_string_prefix_boundary',s.clipboard,'../root-extra/file.txt')
 inside=root/'in.txt';outside2=b/'outside.txt';m.SideBarCopyRelativePathCommand(w).run([str(inside),str(outside2)]);check('mixed_selection_drops_outside_path',s.clipboard,'in.txt')
 w.roots=[];m.SideBarCopyRelativePathCommand(w).run([str(inside),str(outside2)]);check('no_roots_preserves_basenames',s.clipboard,'in.txt\noutside.txt')
 old=b/'move-file.txt';old.write_text('MOVED');target=b/'move-target.txt';target.write_text('OVERWRITE');v=View(str(old));w.v=[v];m.SideBarMoveCommand(w).move(str(old),str(target));check('move_overwrites_existing_file',[old.exists(),target.read_text(),v.p==str(target.resolve())],[False,'MOVED',True])
 olddir=b/'move-dir';olddir.mkdir();(olddir/'child.txt').write_text('DIR');container=b/'container';container.mkdir();v=View(str(olddir/'child.txt'));w.v=[v];m.SideBarMoveCommand(w).move(str(olddir),str(container));check('move_into_existing_directory_retargets_wrong_path',[(container/'move-dir/child.txt').exists(),v.p==str(container/'child.txt'),Path(v.p).exists()],[True,True,False])
 fresh=b/'new.txt';m.SideBarNewCommand(w).create_file(str(fresh));check('new_file_created',[fresh.exists(),fresh.read_bytes()==b'',w.opened[-1]==str(fresh)],[True,True,True])
 fresh.write_text('KEEP');m.SideBarNewCommand(w).create_file(str(fresh));check('new_existing_file_preserved',fresh.read_text(),'KEEP')
 block=b/'parent-is-file';block.write_text('BLOCK');bad=block/'cannot-create.txt';m.SideBarNewCommand(w).create_file(str(bad));check('failed_create_still_opens_nonexistent_path',[bad.exists(),w.opened[-1]==str(bad),'Error creating' in w.messages[-1]],[False,True,True])
 w.v=[];check('missing_active_view_discards_default_setting',m.get_setting(m.SideBarCompareCommand(w),'difftool'),None);w.v=[View(str(fresh))]
 c=m.SideBarCompareCommand(w);check('compare_visibility_and_enable_disagree_three_paths',[c.is_visible([str(src)]*3),c.is_enabled([str(src)]*3)],[False,True])
 calls=[];original=m.subprocess.Popen;m.subprocess.Popen=lambda args:calls.append(args)
 try:c.run([str(src),str(dst),str(fresh)])
 finally:m.subprocess.Popen=original
 check('external_compare_argv_first_two_only',calls,[['owned-diff',str(src),str(dst)]])
 # Normalize owned absolute paths only in diagnostic output so runs compare exactly.
 rows[-1]['observed']=[['owned-diff','<src>','<dst>']]
 m.SideBarDeleteCommand(w).run([str(src)]);check('delete_delegates_with_prompt',w.commands[-1],('delete_file',{'files':[str(src)],'prompt':True}));rows[-1]['observed']=['delete_file',{'files':['<src>'],'prompt':True}]
 w.project=None;error=False
 try:m.RemoveFolderListener().on_post_window_command(w,'delete_folder',{})
 except TypeError:error=True
 check('missing_project_data_listener_raises',error,True)
 check('make_dirs_existing_returns_false',m.SideBarCommand.make_dirs_for(str(root/'child.txt')),False)
 m.SideBarNewCommand(w).create_directory(str(root));check('create_existing_directory_reports_failure','could not be created' in s.dialogs[-1],True)
 runs.append(rows)
assert len(runs[0])==18 and runs[0]==runs[1]
report={'status':'verified_complete_original_module_18controls_twice_owned_files','producer_sha256':h(Path(__file__)),'source_pins':{str((S/'SideBar.py').relative_to(R)):h(S/'SideBar.py')},'runs':runs,'scope':'Complete unchanged SideBar.py imported with explicit fakeSublime/SublimePlugin window/view/settings/clipboard adapters. Original copy/move/create/path/settings/listener/compare bodies executed, real Python os/shutil/filesystem onownedfixtures. ExternalPopen replaced by argv recorder; delete_file delegated only, no actual delete. Threads/input-panels/liveSublimeUI notexecuted.18controls twice.','findings':['Nativeownedfilesystem: duplicate and move overwrite existing file without own confirmation; duplicate existing directory errors and preserves contents.','Nativeownedfilesystem: moving directory into existing directory succeeds into basename child, but original retarget body points open view to missing container/child path.','Nativepath/body: stringprefix accepts sibling root-extra; mixed selection omits outside paths; create failure still opens missing path; missing projectdata listener raises.','UIadapter controls: missing activeview discards default setting; compare enable/visible differ for3paths; externalcommand receives first2argv; delete delegates promptTrue.'],'limits':['No liveSublime/plugin-host/thread-affinity/input-panel/upstreamsuite/platform/performance or production adoption proof.','No actual external diff process or delete command; all copy/move/overwrite operations confined to owned testfolder.','This wrapper duplicates files and delegates compare; it provides no image/content duplicate detector.']}
p=R/'docs/research/dedup-500/sidebar55-native-controls-20261016.json';assert not p.exists();p.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'scenarios':18,'runs':36,'status':'verified'}))
