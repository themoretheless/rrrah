"""Unmodified dupeGuru filesystem/engine/scanner, selected contents paths only."""
import pathlib, json, hashlib, sys, types, importlib.util, tempfile, os, itertools

R=pathlib.Path(__file__).resolve().parents[1]
FS=pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-dupeguru-exact-source-20261013/core/fs.py')
S=pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-dupeguru-scanner-source-20261013')
class Job:
    def start_subjob(self,*args):return self
    def start_job(self,*args):pass
    def add_progress(self,*args,**kwargs):pass
    def set_progress(self,*args):pass
    def iter_with_progress(self,values,*args):return iter(values)
util=types.ModuleType('hscommon.util')
util.nonone=lambda value,fallback:fallback if value is None else value
util.get_file_ext=lambda name:pathlib.Path(name).suffix.lstrip('.')
util.rem_file_ext=lambda name:pathlib.Path(name).stem
util.flatten=itertools.chain.from_iterable
util.dedupe=lambda values:list(dict.fromkeys(values))
def unused(*args):raise AssertionError('unqualified word preprocessing called')
util.multi_replace=unused
trans=types.ModuleType('hscommon.trans');trans.tr=lambda value:value
progress=types.ModuleType('hscommon.jobprogress');progress.job=types.SimpleNamespace(nulljob=Job())
sys.modules.update({'hscommon.util':util,'hscommon.trans':trans,'hscommon.jobprogress':progress,'xxhash':None})
core=types.ModuleType('core');sys.modules['core']=core
def load(name,p):
    spec=importlib.util.spec_from_file_location(name,p);module=importlib.util.module_from_spec(spec)
    sys.modules[name]=module;spec.loader.exec_module(module);return module
fs=load('core.fs',FS);engine=load('core.engine',S/'core/engine.py');core.engine=engine
scanner=load('core.scanner',S/'core/scanner.py')
rows=[]
def files(paths,refs=False):
    result=[fs.File(p) for p in paths]
    for f in result:f.is_ref=refs
    return result
def scan(paths,bigsize=0):
    value=scanner.Scanner();value.scan_type=scanner.ScanType.CONTENTS
    value.big_file_size_threshold=bigsize
    return value.get_dupe_groups(files(paths))
def check(name,action):
    fs.filesdb=fs.FilesDB();fs.filesdb.connect(':memory:')
    with tempfile.TemporaryDirectory(prefix='dupeguru-contents-') as folder:
        result=action(pathlib.Path(folder))
    fs.filesdb.close();rows.append(dict(name=name,passed=True,result=result))
def pair(root,large=False,different=False):
    data=bytearray(b'a'*(5*1024*1024 if large else 65536))
    a,b=root/'a.bin',root/'b.bin';a.write_bytes(data)
    if different:data[0]=ord('b')
    b.write_bytes(data);return [a,b]
def identical(root):
    paths=pair(root);matches=engine.getmatches_by_contents(files(paths))
    assert len(matches)==1 and matches[0].percentage==100
    assert len(scan(paths))==1
    return 'Identical bytes accepted by actual contents scanner.'
check('default_identical_control',identical)
def default_different(root):
    paths=pair(root,True,True);assert paths[0].read_bytes()!=paths[1].read_bytes()
    assert not engine.getmatches_by_contents(files(paths)) and not scan(paths)
    return 'Default bigsize0 full-digest path rejects different5MiB files.'
check('default_full_digest_rejects_sample_collision',default_different)
def sampling(root):
    paths=pair(root,True,True);a,b=files(paths)
    assert a.digest_partial==b.digest_partial and a.digest_samples==b.digest_samples
    assert a.digest!=b.digest
    matches=engine.getmatches_by_contents(files(paths),bigsize=1024*1024)
    assert len(matches)==1 and matches[0].percentage==100
    groups=scan(paths,1024*1024);assert len(groups)==1 and len(groups[0])==2
    return 'Configured sample path emits100% and a2file group for distinct full contents; default path rejected.'
check('configured_samples_accept_distinct_contents',sampling)
def stale(root):
    paths=pair(root);assert len(scan(paths))==1;stat=paths[1].stat()
    data=bytearray(paths[1].read_bytes());data[0]=ord('b');paths[1].write_bytes(data)
    os.utime(paths[1],ns=(stat.st_atime_ns,stat.st_mtime_ns))
    assert paths[1].stat().st_mtime_ns==stat.st_mtime_ns
    assert hashlib.md5(paths[0].read_bytes()).digest()!=hashlib.md5(paths[1].read_bytes()).digest()
    groups=scan(paths);assert len(groups)==1 and len(groups[0])==2
    return 'Default contents scanner reuses stale cached full digest after same-size restoredmtime edit.'
check('default_scanner_stale_metadata_cache',stale)
def missing(root):
    paths=[root/'missing-a',root/'missing-b'];matches=engine.getmatches_by_contents(files(paths))
    assert len(matches)==1 and matches[0].percentage==100
    assert not scan(paths)
    return 'Zero-size accessor fallback matches in engine; default scanner exists check filters both missing files.'
check('default_exists_filter_mitigates_missing_files',missing)
def refs(root):
    paths=pair(root);assert not engine.getmatches_by_contents(files(paths,True))
    return 'Two reference files intentionally skipped.'
check('reference_only_pair_skipped',refs)
def groups(root):
    paths=pair(root)+[root/'c.bin'];paths[2].write_bytes(b'other');a,b,c=files(paths)
    result=engine.get_groups([engine.Match(a,b,80),engine.Match(b,c,80)])
    assert result and all(len(group)<3 for group in result)
    return 'Supplied nontransitive edge chain does not form triple; grouping-only control.'
check('nontransitive_grouping_control',groups)
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
out=R/'docs/research/dedup-500/dupeguru-contents-controls-20261013.json';assert not out.exists()
out.write_text(json.dumps(dict(status='passed7_selected_contents_scanner_controls',controls=rows,
    upstream_tests_executed=0,python=sys.version,
    pins={str(p):sha(p) for p in [FS,S/'core/engine.py',S/'core/scanner.py',pathlib.Path(__file__)]},
    scope='Unmodified three modules, real files/SQLite/MD5. Progress/translation and listed utility adapters supplied; fuzzy words deliberately disabled. Actual contents Scanner.get_dupe_groups exercised, no UI/deletion or whole368blob source review; no actualxxhash.'),indent=2)+'\n')
print('7 contents scanner controls passed')
