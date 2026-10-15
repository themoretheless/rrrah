"""Clone only owned compiled Cargo caches under exclusive source lock; no incremental or deletion."""
from pathlib import Path
import fcntl,json,hashlib,subprocess,shutil,datetime
R=Path(__file__).resolve().parents[1];D=R/'docs/research';source=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-exact-portfolio-20261011');target=R/'.dedup-work/20261015-common-v5/target';out=D/'dedup-durable-pair-v5-20261015/cache-seed.json';assert not out.exists() and not target.exists();target.mkdir();(target/'debug').mkdir();lock=(source/'debug/.cargo-lock').open('rb');rows=[]
try:
 fcntl.flock(lock.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
 for name in ['deps','build','.fingerprint']:
  p=source/'debug'/name
  if p.exists():subprocess.run(['cp','-cR',str(p),str(target/'debug'/name)],check=True);rows.append(dict(source=str(p),target=str(target/'debug'/name),apfs_clone_command_succeeded=True))
 p=source/'.rustc_info.json'
 if p.is_file():shutil.copy2(p,target/p.name)
finally:fcntl.flock(lock.fileno(),fcntl.LOCK_UN);lock.close()
assert not (target/'debug/incremental').exists();out.write_text(json.dumps(dict(status='seeded_owned_compiled_cache_not_test_qualification',observed_at_utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),source=str(source),target=str(target),rows=rows,exclusive_source_cargo_lock=True,incremental_copied=False,deleted_nothing=True,producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),scope='Rebuildablecache seed only; Cargo mustvalidatecurrentcompiler/source fingerprints, freshunfilteredfullbaseline+candidate execution/audits stillrequired. No cachecontent/performance oroldbinary passingclaim. Originalsourcecache unchanged.'),indent=2)+'\n');print('Ownedcompiled cache cloned; noincremental/deletion or testcredit')
