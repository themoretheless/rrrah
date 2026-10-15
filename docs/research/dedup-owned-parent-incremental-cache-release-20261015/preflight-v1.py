from pathlib import Path
import fcntl,hashlib,json,os,shutil,stat
R=Path('/Users/themoretheless/Documents/Sources/rrrah');D=R/'docs/research';target=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-exact-portfolio-20261011');cache=target/'debug/incremental';out=D/'dedup-owned-parent-incremental-cache-release-20261015.json';assert not out.exists();assert cache.is_dir() and not cache.is_symlink();assert cache.resolve()==cache
refs=[R/'scripts/prepare-dedup-native-collection-harness-20261014.py',R/'scripts/build-dedup-consensus-native-20261013.py',R/'scripts/build-dedup-cache-native-probe-20261014.py'];h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
for p in refs:assert 'rrrah-dedup-exact-portfolio-20261011' in p.read_text()
lock=(target/'debug/.cargo-lock').open('rb');fcntl.flock(lock.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
try:
 def siblings():
  return {p.name:dict(mode=p.lstat().st_mode,bytes=p.lstat().st_size,mtime_ns=p.lstat().st_mtime_ns) for p in (target/'debug').iterdir() if p.name not in ['incremental','.cargo-lock']}
 before_siblings=siblings();protected=[]
 for name in ['native_bank_current_all_positives','disc21_heldout_negative_collection','scheduled_outcome_collection_files']:
  p=target/'debug'/name
  if p.is_file():protected.append(dict(path=str(p),sha256=h(p)))
 assert protected
 files=0;logical=0;children=sorted(p.name for p in cache.iterdir())
 for dirpath,dirs,names in os.walk(cache,followlinks=False):
  for name in dirs:assert stat.S_ISDIR((Path(dirpath)/name).lstat().st_mode)
  for name in names:
   st=(Path(dirpath)/name).lstat();assert stat.S_ISREG(st.st_mode);files+=1;logical+=st.st_size
 assert files>0;before=shutil.disk_usage(target).free
 plan=dict(status='owned_rebuildable_compiler_incremental_cache_releasing',path=str(cache),exclusive_cargo_lock_acquired=True,producer_sha256=h(__file__),ownership_references=[dict(path=str(p.relative_to(R)),sha256=h(p)) for p in refs],files=files,logical_bytes=logical,children=children,before_free_bytes=before,protected_binaries=protected,preserved_sibling_inventory_sha256=hashlib.sha256(json.dumps(before_siblings,sort_keys=True).encode()).hexdigest(),scope='Only owned rebuildable incremental cache removed while exclusive Cargo target lock held; all debug siblings/deps/build/binaries/source/retained evidence preserved. No branch/worktree/stash or foreign cache cleanup; freed disk not library speedup.')
 out.write_text(json.dumps(plan,indent=2)+'\n');shutil.rmtree(cache);assert not cache.exists();assert siblings()==before_siblings
 for p in protected:assert h(p['path'])==p['sha256']
 plan.update(status='released_owned_rebuildable_incremental_only',after_free_bytes=shutil.disk_usage(target).free,preserved_siblings=True,protected_binaries_unchanged=True);out.write_text(json.dumps(plan,indent=2)+'\n');print(json.dumps({k:plan[k] for k in ['status','files','logical_bytes','before_free_bytes','after_free_bytes']}))
finally:fcntl.flock(lock.fileno(),fcntl.LOCK_UN);lock.close()
