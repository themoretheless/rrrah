"""Unmodified selected dupeGuru filesystem module; real SQLite/files/hashlib."""
import pathlib, json, hashlib, sys, types, importlib.util, tempfile, os, sqlite3

R = pathlib.Path(__file__).resolve().parents[1]
S = pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-dupeguru-exact-source-20261013')
util = types.ModuleType('hscommon.util')
util.nonone = lambda value, fallback: fallback if value is None else value
util.get_file_ext = lambda name: pathlib.Path(name).suffix
sys.modules['hscommon.util'] = util
sys.modules['xxhash'] = None  # Explicitly exercise upstream's hashlib fallback.
spec = importlib.util.spec_from_file_location('qualified_dupeguru_fs', S/'core/fs.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
rows = []
def check(name, action):
    module.hasher = hashlib.md5
    module.filesdb = module.FilesDB()
    module.filesdb.connect(':memory:')
    with tempfile.TemporaryDirectory(prefix='dupeguru-qualified-') as folder:
        result = action(pathlib.Path(folder))
    module.filesdb.close()
    rows.append(dict(name=name, passed=True, result=result))
def full(root):
    p=root/'file'; data=b'abc'*17000; p.write_bytes(data)
    assert module.File(p).digest == hashlib.md5(data).digest()
    return 'Actual full file MD5 equals independent hashlib.'
check('full_digest_fallback_control',full)
def fresh(root):
    p=root/'file';p.write_bytes(b'a'*64);old=module.File(p).digest;stat=p.stat()
    p.write_bytes(b'b'*64);os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns+1_000_000_000))
    assert module.File(p).digest != old
    return 'Newmtime without prior alternate columns recomputes full digest.'
check('ordinary_mtime_change_recomputes',fresh)
def restored(root):
    p=root/'file';p.write_bytes(b'a'*64);old=module.File(p).digest;stat=p.stat()
    p.write_bytes(b'b'*64);os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns))
    assert p.stat().st_mtime_ns == stat.st_mtime_ns
    assert module.File(p).digest == old != hashlib.md5(p.read_bytes()).digest()
    return 'Actual changed bytes with restored size/mtime reuse olddigest.'
check('restored_metadata_stale_digest',restored)
def cross_field(root):
    p=root/'file';p.write_bytes(b'a'*65536);first=module.File(p)
    old_full,old_partial=first.digest,first.digest_partial;stat=p.stat()
    data=bytearray(p.read_bytes());data[16390]=ord('b');p.write_bytes(data)
    os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns+1_000_000_000))
    second=module.File(p);assert second.digest != old_full
    assert second.digest_partial == old_partial
    assert second._calc_digest_partial() != old_partial
    return 'Updating full digest advances shared row metadata but retains old partial digest.'
check('cross_field_upsert_retains_stale_partial',cross_field)
def ignore_mtime(root):
    module.filesdb.ignore_mtime=True
    p=root/'file';p.write_bytes(b'a'*64);old=module.File(p).digest;stat=p.stat()
    p.write_bytes(b'b'*64);os.utime(p,ns=(stat.st_atime_ns,stat.st_mtime_ns+1_000_000_000))
    assert module.File(p).digest == old != hashlib.md5(p.read_bytes()).digest()
    return 'Explicit ignoremtime permits stale digest at same size.'
check('explicit_ignore_mtime_staleness',ignore_mtime)
def samples(root):
    a=root/'a';b=root/'b';data=bytearray(b'a'*(5*1024*1024));a.write_bytes(data)
    data[0]=ord('b');b.write_bytes(data)
    first,second=module.File(a),module.File(b)
    assert first.digest_samples == second.digest_samples
    assert first.digest != second.digest
    return 'Three sample hashes collide for difference at first byte; full hashes differ.'
check('sample_filter_is_not_full_equality',samples)
def missing(root):
    assert module.File(root/'missing').digest == b''
    return 'File accessor catches absent-file exception and returns empty digest default; scanner not exercised.'
check('missing_file_accessor_default',missing)
def backend(root):
    p=root/'file';p.write_bytes(b'abc');old=module.File(p).digest
    module.hasher=hashlib.sha256
    assert module.File(p).digest == old != module.File(p)._calc_digest()
    return 'Injected actual SHA256 backend reuses old MD5 cache; actual xxhash not tested.'
check('hash_backend_not_in_cache_namespace',backend)
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
p=R/'docs/research/dedup-500/dupeguru-fs-controls-20261013.json'
assert not p.exists()
p.write_text(json.dumps(dict(status='passed8_selected_fs_controls',controls=rows,
    python=sys.version,sqlite=sqlite3.sqlite_version,upstream_tests_executed=0,
    adapters=['hscommon.util.nonone/get_file_ext only','xxhash import disabled to select original MD5 fallback','one explicit injected SHA256 namespace control'],
    pins={str(p):sha(p) for p in [S/'core/fs.py',pathlib.Path(__file__)]},
    scope='Whole selected core/fs.py unchanged, actual filesystem/SQLite/hashlib. No scanner/app/deletion execution or whole dupeGuru source review; no original xxhash backend qualification.'),indent=2)+'\n')
print('8 selected filesystem controls passed')
