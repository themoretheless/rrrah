"""Unchanged native C modules/CommonCrypto/clonefile on owned sparse and tiny fixtures."""
from pathlib import Path
import hashlib,json,os,stat,subprocess
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';W=R/'.dedup-work/dedup32-20261016';S=W/'source';B=W/'native-controls';B.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
main=r'''#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <errno.h>
#include <CommonCrypto/CommonDigest.h>
#include "map.h"
#include "clone.h"
extern int populate_sha256_if_empty(FileMetadata*);
int main(int argc,char**argv){if(argc!=4)return 2;int r=0;if(!strcmp(argv[1],"hash")){struct stat a,b;if(stat(argv[2],&a)||stat(argv[3],&b))return 3;FileMetadata x={.path=argv[2],.size=a.st_size},y={.path=argv[3],.size=b.st_size};int rx=populate_sha256_if_empty(&x),ry=populate_sha256_if_empty(&y);printf("HASH %d %d %zu ",rx,ry,sizeof(CC_LONG));for(int i=0;i<32;i++)printf("%02x",x.sha256[i]);printf(" ");for(int i=0;i<32;i++)printf("%02x",y.sha256[i]);printf("\n");return 0;}if(!strcmp(argv[1],"clone"))r=replace_with_clone(argv[2],argv[3],false);else if(!strcmp(argv[1],"link"))r=replace_with_link(argv[2],argv[3]);else if(!strcmp(argv[1],"symlink"))r=replace_with_symlink(argv[2],argv[3]);else return 4;printf("RESULT %d ERRNO %d\n",r,errno);return 0;}
''';(B/'main.c').write_text(main);exe=B/'controls';cmd=['cc','-std=gnu2x','-O2','-I'+str(S),str(B/'main.c'),str(S/'map.c'),str(S/'alist.c'),str(S/'clone.c'),'-o',str(exe)]
with (B/'build.log').open('x') as stream:result=subprocess.run(cmd,stdout=stream,stderr=subprocess.STDOUT)
assert result.returncode==0,(B/'build.log').read_text();rows=[]
def invoke(mode,a,b,label,repeat):
 log=B/f'{repeat}-{label}.log'
 with log.open('x') as stream:r=subprocess.run([str(exe),mode,str(a),str(b)],stdout=stream,stderr=subprocess.STDOUT,timeout=30)
 assert r.returncode==0,log.read_text();line=next(x for x in log.read_text().splitlines() if x.startswith(('HASH ','RESULT ')));return line,log
for repeat in [1,2]:
 root=B/f'run{repeat}';root.mkdir()
 for size,label in [(2*1024*1024,'2m'),(2**32,'4g'),(2**32+65536,'4g64k')]:
  directory=root/label;directory.mkdir();a=directory/'a';b=directory/'b';offset=size//2+123
  for path,marker in [(a,b'X'),(b,b'Y')]:
   with path.open('xb') as f:f.truncate(size);f.seek(0);f.write(b'HEAD');f.seek(offset);f.write(marker);f.seek(size-1);f.write(b'Z')
  with a.open('rb') as f:f.seek(offset);left=f.read(1)
  with b.open('rb') as f:f.seek(offset);right=f.read(1)
  assert left==b'X' and right==b'Y';line,log=invoke('hash',a,b,label,repeat);_,rx,ry,width,x,y=line.split();assert rx==ry=='0' and width=='4';prefix_length=size%(2**32)
  if size>=2**32:
   with a.open('rb') as f:prefix=f.read(prefix_length)
   expected=hashlib.sha256(prefix).hexdigest();assert x==y==expected
  else:assert x!=y;expected=None
  rows.append({'repeat':repeat,'case':label,'size':size,'changed_offset':offset,'different_bytes':[left.hex(),right.hex()],'native_digests':[x,y],'prefix_length_used_by_32bit_API':prefix_length,'prefix_sha256_oracle':expected,'allocated_bytes':[a.stat().st_blocks*512,b.stat().st_blocks*512],'log':str(log.relative_to(R)),'log_sha256':h(log)})
 def paths(label,source=b'AAAA',target=b'AAAA'):
  d=root/label;d.mkdir();a=d/'src';b=d/'dst'
  if source is not None:a.write_bytes(source)
  b.write_bytes(target);return a,b
 a,b=paths('clone_equal');a.chmod(0o600);b.chmod(0o640);line,log=invoke('clone',a,b,'clone_equal',repeat);assert line.split()[1]=='0' and b.read_bytes()==b'AAAA' and stat.S_IMODE(b.stat().st_mode)==0o640;b.write_bytes(b'BBBB');assert a.read_bytes()==b'AAAA';rows.append({'repeat':repeat,'case':'clone_equal_metadata_copy_on_write','log':str(log.relative_to(R)),'log_sha256':h(log)})
 a,b=paths('clone_missing',source=None);line,log=invoke('clone',a,b,'clone_missing',repeat);assert line.split()[1]!='0' and b.read_bytes()==b'AAAA';rows.append({'repeat':repeat,'case':'clone_missing_source_preserves_target','log':str(log.relative_to(R)),'log_sha256':h(log)})
 a,b=paths('clone_staging');stage=b.with_name('.~.'+b.name);stage.write_bytes(b'KEEP');line,log=invoke('clone',a,b,'clone_staging',repeat);assert line.split()[1]!='0' and b.read_bytes()==b'AAAA' and stage.read_bytes()==b'KEEP';rows.append({'repeat':repeat,'case':'clone_existing_staging_preserved','log':str(log.relative_to(R)),'log_sha256':h(log)})
 a,b=paths('clone_zero',source=b'');line,log=invoke('clone',a,b,'clone_zero',repeat);assert line.split()[1]!='0' and b.read_bytes()==b'AAAA';rows.append({'repeat':repeat,'case':'zero_source_clone_refuses_preserves_target','log':str(log.relative_to(R)),'log_sha256':h(log)})
 a,b=paths('link_missing',source=None);line,log=invoke('link',a,b,'link_missing',repeat);assert line.split()[1]!='0' and not b.exists();rows.append({'repeat':repeat,'case':'direct_link_missing_source_loses_target','log':str(log.relative_to(R)),'log_sha256':h(log)})
 a,b=paths('link_equal');line,log=invoke('link',a,b,'link_equal',repeat);assert line.split()[1]=='0' and a.stat().st_ino==b.stat().st_ino;rows.append({'repeat':repeat,'case':'direct_link_success_shared_inode','log':str(log.relative_to(R)),'log_sha256':h(log)})
 a,b=paths('symlink_equal');line,log=invoke('symlink',a,b,'symlink_equal',repeat);assert line.split()[1]=='0' and b.is_symlink() and b.read_bytes()==a.read_bytes() and not os.path.isabs(os.readlink(b));rows.append({'repeat':repeat,'case':'direct_symlink_relative_success','log':str(log.relative_to(R)),'log_sha256':h(log)})
assert len(rows)==20;out=D/'dedup32-native-large-clone-controls-20261016.json';assert not out.exists();out.write_text(json.dumps({'status':'verified_original_native_modules_10controls_twice','whole_repo_complete':False,'repository':'ttkb-oss/dedup','producer_sha256':h(__file__),'rows':rows,'source_pins':{str(p.relative_to(R)):h(p) for p in [S/'map.c',S/'map.h',S/'alist.c',S/'alist.h',S/'clone.c',S/'clone.h',S/'attr.h',B/'main.c',exe,B/'build.log']},'command':cmd,'scope':'Entire unchanged originalmap/alist/clone C modules linked against actual macOS CommonCrypto/rbtree/clonefile/copyfile/libc. No data or syscall adapters.10controls twice; native digest uses32bitCC_LONG truncation at4GiB and4GiB+64KiB on sparse distinct-byte files. Small2MiB differingdigest control. Actualtinyclone metadata/COW/failures/hardlink/symlink onowned fixtures only. Not wholeCLI/upstreamsuite/profile/snapshotrace/nativeotherplatform/RSS/physicalIO/performance/adoption qualification.','limits':['Largefiles sparse, hashes match independently computed truncated prefix; a differing byte proves files unequal without full4GiBhash/IO/RSS claim','Directlink missing-source loss tests invalid/disappeared source at module API, not wholeCLI reproduction','Original make strictprofile/upstreamChecksuite not run; no production code copied, no userfiles touched']},indent=2)+'\n');print('10native modulecontrols twice passed,4GiBhashprefix truncation reproduced; ownedfixtures only')
