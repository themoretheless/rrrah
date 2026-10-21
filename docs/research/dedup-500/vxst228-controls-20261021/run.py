from pathlib import Path
import subprocess,tempfile,hashlib,json,lzma
root=Path.cwd();cache=root/'.dedup-work/vxst228-20261021';exe=cache/'dedup'
def run(*args):return subprocess.run([str(exe),*map(str,args)],capture_output=True,timeout=15)
with tempfile.TemporaryDirectory(prefix='vxst228-') as temp:
 t=Path(temp)
 for n in [0,1,511,512,513,1024,1537]:
  data=(bytes(range(256))*((n+255)//256))[:n];src=t/'src';encoded=t/'encoded';decoded=t/'decoded';src.write_bytes(data);assert run('-e',src,encoded).returncode==0;assert run('-d',encoded,decoded).returncode==0;assert decoded.read_bytes()==data
 print('PASS exact original CLI roundtrips7 boundary lengths0,1,511,512,513,1024,1537')
 src=t/'same';src.write_bytes(b'original file contents');r=run('-e',src,src);assert r.returncode==0;assert src.read_bytes()!=b'original file contents';assert src.read_bytes().startswith(b'DEDUP');print('PASS same input/output path destroys original contents and exits0')
 src=t/'invalid';src.write_bytes(b'invalid format'+bytes(512));r=run('-d',src,t/'invalid-output');assert r.returncode==0;assert b'Invalid header' in r.stdout;assert (t/'invalid-output').read_bytes()==b'';print('PASS invalid-header decode reports text error but exits0 with empty output')
 sample=lzma.decompress((cache/'test_data.xz').read_bytes());assert sample==(cache/'A').read_bytes()==(cache/'C').read_bytes();print(json.dumps({'sample_bytes':len(sample),'sample_sha256':hashlib.sha256(sample).hexdigest(),'encoded_sample_bytes':(cache/'B').stat().st_size,'authored_controls':3,'boundary_cases':7}))
