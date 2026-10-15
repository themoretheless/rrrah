"""Pinned unchanged AST modules with explicit relative-import bindings; owned IO only."""
from pathlib import Path
import ast,hashlib,json,sys,subprocess,tempfile,os,filecmp,types,importlib.metadata as metadata
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dep66-20261015';S=W/'source';sys.path.insert(0,str(W/'deps'));sys.dont_write_bytecode=True
import xxhash,psutil,send2trash

def load(name,bindings):
 p=S/name;t=ast.parse(p.read_text());nodes=[n for n in t.body if not (isinstance(n,ast.ImportFrom) and n.level)];ns={'__name__':'dep66_'+name.replace('/','_'),**bindings};exec(compile(ast.Module(body=nodes,type_ignores=[]),str(p),'exec'),ns);return ns
u=load('duplicate/utils/init.py',{});c=load('duplicate/utils/fs/common.py',{'is_os64':u['is_os64']});p=load('duplicate/utils/fs/posix.py',{'compilecards':u['compilecards']});st=load('duplicate/structs.py',{'blkdevice':c['blkdevice'],'blksize':p['blksize']});bindings={k:c[k] for k in ['checksum','fsdecode','remove','sidesum','signature','splitpaths','walk']};bindings.update({k:p[k] for k in ['blksize','is_archived','is_hidden','is_system']});bindings.update({k:st[k] for k in ['Cache','DupInfo','FileInfo','FilterType','SkipException']});bindings['is_os64']=u['is_os64'];co=load('duplicate/core.py',bindings)
obs={};cases=[]
def check(name,f):obs[name]=f();cases.append(name);print('PASS '+name)
def throws(kind,f):
 try:f()
 except kind as e:return type(e).__name__+': '+str(e)
 raise AssertionError('expected '+kind.__name__)
with tempfile.TemporaryDirectory(prefix='owned-native-',dir=W) as tmp:
 F=Path(tmp);data=bytes(range(64));a=F/'a';b=F/'b';a.write_bytes(data);b.write_bytes(data);fi=st['FileInfo'];FT=st['FilterType'];DI=st['DupInfo']
 def native_import():
  env=os.environ.copy();env.update(PYTHONPATH=os.pathsep.join([str(S),str(W/'deps')]),PYTHONDONTWRITEBYTECODE='1');child=subprocess.run([sys.executable,'-c','import duplicate'],capture_output=True,text=True,env=env,timeout=30);assert child.returncode!=0 and "cannot import name 'blksize'" in child.stderr;return {'returncode':child.returncode,'exception':child.stderr.splitlines()[-1]}
 check('actual_Mac_package_import_fails_missing_blksize_export',native_import)
 check('actual_DupInfo_initializer_rejects_namedtuple_init_arguments',lambda:throws(TypeError,lambda:DI(FT.ID,{1:[fi(str(a)),fi(str(b))]},[])))
 def bare(mapping):return tuple.__new__(DI,(FT.ID,mapping,[],None))
 def dict_mutation():return throws(RuntimeError,lambda:bare({1:[fi(str(a))]})._filter())
 check('actual_filter_mutates_live_dict_view',dict_mutation)
 def zero_parent():
  parent=bare({0:[fi(str(a)),fi(str(b))]});child=DI.__new__(DI,FT.NAME,{'x':[fi(str(a)),fi(str(b))]},[],parent,0);assert child.parent is None and isinstance(parent.dups[0],list);return 'Falsy parentkey0 notattached; constructor __new__ only'
 check('zero_parent_key_suppresses_parent_attachment',zero_parent)
 def triple_skip():
  third=F/'third';third.write_bytes(b'not equal to first');group=bare({'injected-hash-bucket':[fi(str(a)),fi(str(b)),fi(str(third))]});progress=[];co['_binaryfilter'](FT.BINARY,group,None,progress.append);assert len(group.dups['injected-hash-bucket'])==3 and progress==[];return 'Explicit3-entry collisionbucket fixture; no naturalxxhashcollision claim, BINARYstage skipped'
 check('binary_stage_skips_three_member_bucket',triple_skip)
 def native_checksum():
  assert c['checksum'](str(a),7)==xxhash.xxh64(data).hexdigest();return 'All64actualbytes hashedwithbuffer7'
 check('native_complete_checksum_matches_real_xxhash',native_checksum)
 def zero_checksum():
  assert c['checksum'](str(a),0)==xxhash.xxh64(b'').hexdigest();assert a.stat().st_size;return 'Directhelper bufsize0; actualcaller normallyusespositivebuffer'
 check('checksum_zero_buffer_returns_empty_digest',zero_checksum)
 def excessive_chunks():
  with c['readopen'](str(a)) as (read,fd):got=c['_chunksum'](fd,read,10,(4,2),(0,os.SEEK_SET))
  assert got==xxhash.xxh64(data).hexdigest() and got!=xxhash.xxh64(data[:10]).hexdigest();return {'requested':10,'actual_hashed':64}
 check('chunk_hash_nondivisible_size_hashes_to_EOF',excessive_chunks)
 def short_chunks():
  with c['readopen'](str(a)) as (read,fd):got=c['_chunksum'](fd,lambda n:read(min(n,2)),8,(4,0),(0,os.SEEK_SET))
  assert got==xxhash.xxh64(data[:4]).hexdigest();return {'requested':8,'actual_hashed':4,'adapter':'Explicitlegal2byte shortreads overactualos.read'}
 check('chunk_hash_shortread_decrements_requested_instead_of_actual_bytes',short_chunks)
 def signatures():
  x=F/'signature1';y=F/'signature2';x.write_bytes(b'A'*261+b'x');y.write_bytes(b'A'*261+b'y');assert c['signature'](str(x))==c['signature'](str(y));return 'Prefix candidate only, no final duplicate claim'
 check('261byte_signature_ignores_later_difference',signatures)
 def cached_compare():
  filecmp.clear_cache();x=F/'cache1';y=F/'cache2';x.write_bytes(b'AAAA');y.write_bytes(b'AAAA');infos=[fi(str(x)),fi(str(y))];assert co['_binarycmp'](infos,None)[0];before=y.stat();y.write_bytes(b'BBBB');os.utime(y,ns=(before.st_atime_ns,before.st_mtime_ns));assert x.read_bytes()!=y.read_bytes();assert co['_binarycmp'](infos,None)[0];filecmp.clear_cache();assert not co['_binarycmp'](infos,None)[0];return 'Actualstdlib filecmp shallowFalse cache, explicitsamemetadatarewrite; clearscache aftercontrol'
 check('binary_compare_uses_stale_stdlib_cache_under_same_metadata_rewrite',cached_compare)
 def mutated_purge():
  victim=F/'victim';victim.write_bytes(b'old duplicate');old=fi(str(victim));deleted=[];errors=[];co['_purge']([old],deleted,errors,False,lambda path:Path(path).write_bytes(b'new unique bytes'),None);assert not victim.exists() and deleted==[str(victim)] and errors==[];return 'Explicitbeforedelete callbackrewrite; newlyownedvictimonly, no realuserdata'
 check('before_delete_callback_rewrite_is_not_revalidated',mutated_purge)
 def skip_purge():
  victim=F/'skip';victim.write_bytes(b'keep');deleted=[]
  def cb(path):raise st['SkipException']
  co['_purge']([fi(str(victim))],deleted,[],False,cb,None);assert victim.exists() and not deleted;return 'DocumentedSkipException skipworks'
 check('documented_skip_exception_preserves_victim',skip_purge)
 def link_split():
  symlink=F/'link';os.symlink(a.name,symlink);result=c['splitpaths']([str(symlink)],True);assert result[2]==[] and result[3]==[str(symlink)];return 'Actuallstat symlinkmode classifiedasothernode evenfollowlinksTrue'
 check('splitpaths_followlinks_does_not_classify_file_symlink_as_link',link_split)
 def walk_api():
  root=F/'walk';(root/'sub').mkdir(parents=True);return throws(TypeError,lambda:list(c['walk'](str(root))))
 check('native_DirEntry_is_dir_rejects_positional_follow_symlinks',walk_api)
 def missing_walk():
  errors=[];text=throws(TypeError,lambda:list(c['walk'](str(F/'missing'),errors.append)));assert len(errors)==1;return text
 check('walk_error_callback_then_None_unpack_error',missing_walk)
 check('Cache_clear_calls_RLock_locked_missing_on_Python312',lambda:throws(AttributeError,lambda:st['Cache']().clear()))
 def device_typo():return throws(AttributeError,lambda:c['blkdevice'](str(a)))
 check('actual_psutil_partition_field_mountpoin_typo',device_typo)
 def hidden_basename():
  hidden=F/'.hidden';hidden.write_bytes(b'x');assert p['is_hidden'](str(hidden)) is False and p['is_hidden']('.hidden') is True;return 'ActualPOSIX fallback onMac checksfullpathprefix, UF_HIDDENunset'
 check('absolute_hidden_basename_not_recognized_by_POSIX_fallback',hidden_basename)
 pins={str(q.relative_to(S)):hashlib.sha256(q.read_bytes()).hexdigest() for q in S.rglob('*') if q.is_file()}
 print(json.dumps({'status':'passed','controls':len(cases),'cases':cases,'observations':obs,'source_pins':pins,'versions':{x:metadata.version(x) for x in ['xxhash','psutil','Send2Trash']},'python':sys.version,'adapters':['EntireASTmodules unchangedexceptrelative-import statements omitted andbound explicitly toactualselectedoriginalnames. Common/POSIX/init/structs/core, actualnativePython andrealxxhash/psutil/send2trash; send2trashneverinvoked.','FullMacpackageimport executedunchangedandfailsmissingblksize. Remainingcontrols notwholepackageexecution.','Tuple.__new__ suppliesexplicitDupInfofixtures bypassingbroken__init__ solelyfor _filter,parentattachment,binaryskip controls; no algorithm or classfix.','Two-byte readwrapper and beforedelete rewrite areexplicitfault schedules. No naturalhashcollision claim.'],'limits':['Nooriginaltests tracked: toxdefaultcompileall only. No original2017Python/dependency/platform/directIO/pyobjc/Windows/nativeLinux/fullpipeline/CLI orperformancequalification.','Allwrites/removes newlyownedfixtures; no trash, realmedia,sourceproject,userfiles orproductionadoption.']}))
