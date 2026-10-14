#!/usr/bin/env python3
from pathlib import Path,PurePosixPath
import subprocess,hashlib,json,os
R=Path(__file__).resolve().parents[1];D=R/'docs/research';commit=subprocess.check_output(['git','rev-parse','415443172'],cwd=R,text=True).strip();assert commit=='415443172180ef2838716eac4afbdd1cf588e879';F=Path('/private/tmp/rrrah-committed-build-source-415443172-20261015');F.mkdir(exist_ok=False);O=D/'dedup-committed-build-source-415443172-20261015.json';assert not O.exists();extra={'docs/research/dedup-gradient-index-before-borrowed-distance.rs','docs/research/dedup-gradient-index-before-bounded-result-growth.rs','docs/research/dedup-gradient-index-before-distance-early-exit.rs'};raw=subprocess.check_output(['git','ls-tree','-r','-l','-z',commit],cwd=R);selected=[];skipped=[]
for record in raw.split(b'\0'):
 if not record:continue
 metadata,path=record.split(b'\t',1);mode,kind,blob,size=metadata.decode().split();name=path.decode('utf-8');parts=PurePosixPath(name).parts
 include=parts[0]in ['crates','vendor','tests','.cargo']or name in extra or ('/'not in name and (name.endswith('.toml')or name in ['Cargo.lock','README.md']or name.startswith('LICENSE')))
 if not include:continue
 assert not PurePosixPath(name).is_absolute()and '..'not in parts and not any(p in ['.git','.aws']or p.startswith('.env')for p in parts)
 if kind!='blob'or mode not in ['100644','100755']:
  skipped.append(dict(path=name,kind=kind,mode=mode));continue
 selected.append((name,blob,int(size),mode))
assert all(any(v[0]==name for v in selected)for name in extra);child=subprocess.Popen(['git','cat-file','--batch'],cwd=R,stdin=subprocess.PIPE,stdout=subprocess.PIPE);rows=[];total=0
try:
 for name,blob,size,mode in selected:
  child.stdin.write((blob+'\n').encode());child.stdin.flush();header=child.stdout.readline().decode().split();assert header==[blob,'blob',str(size)];dest=F/name;dest.parent.mkdir(parents=True,exist_ok=True);git_hash=hashlib.sha1(b'blob '+str(size).encode()+b'\0');sha=hashlib.sha256();remaining=size
  with dest.open('xb')as output:
   while remaining:
    data=child.stdout.read(min(1024*1024,remaining));assert data;output.write(data);git_hash.update(data);sha.update(data);remaining-=len(data)
  assert child.stdout.read(1)==b'\n'and git_hash.hexdigest()==blob;dest.chmod(0o755 if mode=='100755'else 0o644);rows.append(dict(path=name,git_blob_sha1=blob,sha256=sha.hexdigest(),bytes=size,mode=mode));total+=size
 child.stdin.close();assert child.wait()==0
except BaseException:
 child.kill();child.wait();raise
assert not skipped
for row in rows:assert hashlib.sha256((F/row['path']).read_bytes()).hexdigest()==row['sha256']
O.write_text(json.dumps(dict(status='verified_committed_build_source_snapshot',commit=commit,source=str(F),producer_pid=os.getpid(),producer_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),files=len(rows),bytes=total,skipped_selected_nonregular=skipped,source_files=rows,scope='Regular tracked build source/manifests/vendor/public testfixtures from immutable publiclypushed415443172, each rawGitblob SHA1/size and resulting SHA256 verified before/aftercapture. Three required docs/research Rust example baselines included. No dirty/untracked/private corpus/credentials/symlink/submodule copies. Original500 research artifacts and other research media excluded. Committedcheckpoint only, not latestmutableRoot or allrepositorycontent/runtime qualification.'),indent=2)+'\n');print(len(rows),'files',total,'bytes captured from415443172')
