#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess
from pathlib import Path
R=Path(__file__).resolve().parents[1];D=R/'docs/research';T=Path('/Users/themoretheless/.codex/tmp')
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 manifests={'baseline':D/'dedup-loftr-selected-windows-current-baseline-20261012.json','selected':D/'dedup-loftr-selected-windows-source-v2-20261012.json'}
 old=T/'rrrah-disc21-heldout-negative-harness-20261012';oldsource=T/'rrrah-disc21-heldout-negative-source-20261012';out=D/'dedup-loftr-selected-windows-collection-build-20261012.json';assert not out.exists()
 pins={str(p):sha(p) for p in [Path(__file__),old/'Cargo.toml',old/'Cargo.lock',old/'src/bin/disc21_heldout_negative_collection.rs',*manifests.values()]};r={'status':'building','producer_pid':os.getpid(),'pins':pins,'builds':[]}
 def save():
  temp=out.with_suffix('.tmp');temp.write_text(json.dumps(r,indent=2)+'\n');temp.replace(out)
 save();runtime=T/'rrrah-loftr-native-runtime-20261010';env=dict(os.environ,LIBTORCH_USE_PYTORCH='1');env['PATH']=str(runtime/'bin')+':'+env['PATH']
 for mode,mp in manifests.items():
  m=json.loads(mp.read_text());s=Path(m['snapshot']);assert all(sha(s/k)==h for k,h in m['file_hashes'].items())
  h=T/f'rrrah-loftr-selected-windows-collection-{mode}-20261012';assert not h.exists();(h/'src/bin').mkdir(parents=True)
  (h/'Cargo.toml').write_text((old/'Cargo.toml').read_text().replace(str(oldsource),str(s)));shutil.copy2(old/'Cargo.lock',h/'Cargo.lock');name='selected_window_collection_'+mode;probe=h/'src/bin'/f'{name}.rs';shutil.copy2(old/'src/bin/disc21_heldout_negative_collection.rs',probe)
  for p in [h/'Cargo.toml',h/'Cargo.lock',probe]:pins[str(p)]=sha(p)
  cmd=['cargo','build','--manifest-path',str(h/'Cargo.toml'),'--bin',name,'--locked','--offline','--target-dir',str(T/'rrrah-dedup-exact-portfolio-20261011')];log=T/f'rrrah-loftr-selected-windows-collection-{mode}-build-20261012.log';row={'mode':mode,'command':cmd,'status':'building'};r['builds'].append(row)
  with log.open('w') as f:
   child=subprocess.Popen(cmd,cwd='/tmp',env=env,stdout=f,stderr=subprocess.STDOUT);row['child_pid']=child.pid;save();rc=child.wait()
  row.update(returncode=rc,log=str(log));pins[str(log)]=sha(log);save();assert rc==0
  exe=T/f'rrrah-loftr-selected-windows-collection-{mode}-executable-20261012';assert not exe.exists();shutil.copy2(T/'rrrah-dedup-exact-portfolio-20261011/debug'/name,exe);pins[str(exe)]=sha(exe);row.update(status='compiled_not_executed',executable=str(exe));save();print('Built',mode,flush=True)
 r['status']='complete_two_preserved_executables_not_executed';save()
if __name__=='__main__':main()
