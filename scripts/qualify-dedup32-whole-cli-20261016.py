"""Compile all unchanged production C and exercise actual CLI only on owned fixtures."""
from pathlib import Path
import hashlib,json,os,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dedup32-20261016';S=W/'source';B=W/'whole-cli';B.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();exe=B/'dedup';sources=['dedup.c','alist.c','clone.c','map.c','progress.c','queue.c','utils.c'];cmd=['cc','-std=gnu2x','-O2','-pthread','-DVERSION="0.0.6"','-DBUILD_DATE="qualification"',*[str(S/x) for x in sources],'-o',str(exe)]
with (B/'build.log').open('x') as stream:r=subprocess.run(cmd,stdout=stream,stderr=subprocess.STDOUT)
assert r.returncode==0,(B/'build.log').read_text();rows=[]
for repeat in [1,2]:
 for size,label in [(2*1024*1024,'2m-distinct'),(2**32,'4g-distinct'),(2**32+65536,'4g64k-distinct')]:
  case=B/f'run{repeat}-{label}';case.mkdir();offset=size//2+123;files=[case/'a',case/'b']
  for path,marker in zip(files,[b'X',b'Y']):
   with path.open('xb') as f:f.truncate(size);f.write(b'HEAD');f.seek(offset);f.write(marker);f.seek(size-1);f.write(b'Z')
  before=[{'size':x.stat().st_size,'inode':x.stat().st_ino,'mtime_ns':x.stat().st_mtime_ns,'blocks':x.stat().st_blocks} for x in files];log=B/f'run{repeat}-{label}.log';command=[str(exe),'-t0','-P','-n',str(case)]
  with log.open('x') as stream:r=subprocess.run(command,cwd=B,stdout=stream,stderr=subprocess.STDOUT,timeout=30)
  assert r.returncode==0,log.read_text();text=log.read_text();values={key:int(next(l.split(': ',1)[1] for l in text.splitlines() if l.startswith(key+': '))) for key in ['duplicates found','bytes saved','already saved']}
  assert before==[{'size':x.stat().st_size,'inode':x.stat().st_ino,'mtime_ns':x.stat().st_mtime_ns,'blocks':x.stat().st_blocks} for x in files]
  markers=[]
  for x in files:
   with x.open('rb') as f:f.seek(offset);markers.append(f.read(1).hex())
  assert markers==['58','59']
  if size<2**32:assert values=={'duplicates found':0,'bytes saved':0,'already saved':0}
  else:assert values['bytes saved']+values['already saved']==size
  rows.append({'repeat':repeat,'case':label,'command':command,'file_size':size,'different_byte_offset':offset,'different_bytes':markers,'metrics':values,'dry_run_metadata_unchanged':True,'log':str(log.relative_to(R)),'log_sha256':h(log)})
 # Full non-dry CLI path on fresh tiny exact duplicates, real APFS clonefile.
 case=B/f'run{repeat}-tiny-equal';case.mkdir();a=case/'a';b=case/'b';a.write_bytes(b'owned exact bytes');b.write_bytes(b'owned exact bytes');log=B/f'run{repeat}-tiny-equal.log';command=[str(exe),'-t0','-P',str(case)]
 with log.open('x') as stream:r=subprocess.run(command,cwd=B,stdout=stream,stderr=subprocess.STDOUT,timeout=30)
 assert r.returncode==0 and a.read_bytes()==b.read_bytes()==b'owned exact bytes' and 'cloned to' in log.read_text();rows.append({'repeat':repeat,'case':'tiny-exact-real-clone','command':command,'log':str(log.relative_to(R)),'log_sha256':h(log)})
 # Full non-dry CLI on owned distinct4GiB+64KiB sparsefiles: demonstrates actual wrong replacement.
 case=B/f'run{repeat}-4g64k-live';case.mkdir();size=2**32+65536;offset=2**31+123;files=[case/'a',case/'b']
 for path,marker in zip(files,[b'X',b'Y']):
  with path.open('xb') as f:f.truncate(size);f.write(b'HEAD');f.seek(offset);f.write(marker);f.seek(size-1);f.write(b'Z')
 log=B/f'run{repeat}-4g64k-live.log';command=[str(exe),'-t0','-P',str(case)]
 with log.open('x') as stream:r=subprocess.run(command,cwd=B,stdout=stream,stderr=subprocess.STDOUT,timeout=30)
 assert r.returncode==0 and 'cloned to' in log.read_text();markers=[]
 for path in files:
  assert path.stat().st_size==size
  with path.open('rb') as f:f.seek(offset);markers.append(f.read(1).hex())
 assert markers[0]==markers[1] and markers[0] in ['58','59'];rows.append({'repeat':repeat,'case':'4g64k-distinct-actual-wrong-clone','command':command,'file_size':size,'different_byte_offset':offset,'before_distinct_bytes':['58','59'],'after_equal_bytes':markers,'log':str(log.relative_to(R)),'log_sha256':h(log)})
assert len(rows)==10;out=D/'dedup32-whole-cli-controls-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_whole_original_cli_5controls_twice','whole_repo_complete':False,'producer_sha256':h(__file__),'source_pins':{str(x.relative_to(R)):h(x) for x in [*sorted(S.glob('*.c')),*sorted(S.glob('*.h')),exe,B/'build.log']},'command':cmd,'compiler':subprocess.check_output(['cc','--version'],text=True),'rows':rows,'scope':'All7unchangedproductionCmodules/headers compile and actual nativeCLI executed witht0. CommonCrypto/rbtree/clonefile/copyfile actualDarwinAPIs.3dryrun distinctsizes plusactualtinyclone andactualwrongreplacement ofdistinct4GiB+64KiBsparsefiles, twice. Allwrites/clones ownedsynthetic fixtures. No call/data adapters, no actualuserfiles.','limits':['Originalmake/Check/ASAN/strictprofile not executed; no HFSdiskimage mounts, ACL/immutability/fullupstreamtestsuite/parallelworkers/otherplatform proof','Sparse logicalsizes, differing-byte oracle only; no full4GiB physicalIO/RSS/performance qualification','No productioncodecopied or adoption; this is competitor correctness evidence, not our library speedup']},indent=2)+'\n');print('Whole unchangedCLI5controls twice: actualwrong4GiB+64KiBclone reproduced onowned sparsefixtures')
