"""Waited whole CLI subprocess controls, all writes in owned temporary directory."""
import hashlib,json,pathlib,shutil,subprocess,tempfile
root=pathlib.Path(__file__).resolve().parents[3];w=root/'.dedup-work/yarn084-20261024';node=shutil.which('node');cli=w/'dist/cli.js';fixture=(w/'tests/fixtures/yarn.lock').read_bytes();results=[]
with tempfile.TemporaryDirectory(prefix='rrrah-yarn084-') as d:
 p=pathlib.Path(d)/'owned.lock'
 def run(flags,data):
  p.write_bytes(data);before=p.read_bytes();r=subprocess.run([node,str(cli),*flags,str(p)],capture_output=True,timeout=30);return r,before,p.read_bytes()
 r,b,a=run(['--list','--fail'],fixture);assert r.returncode==1 and a==b and b'Found duplicated' in r.stderr;results.append({'case':'list-fail','exit_code':1,'bytes_preserved':True})
 r,b,a=run(['--fail'],fixture);assert r.returncode==1 and a!=b and b'Found duplicated' in r.stderr;results.append({'case':'write-fail','exit_code':1,'bytes_changed':True})
 r,b,a=run(['--fail'],a);assert r.returncode==0 and a==b;results.append({'case':'second-write-fail','exit_code':0,'idempotent':True})
 r,b,a=run([],fixture.replace(b'\n',b'\r\n'));assert r.returncode==0 and b'\r\n' in a and b'\n' not in a.replace(b'\r\n',b'');results.append({'case':'CRLF','exit_code':0,'only_CRLF':True})
script="const fs=require('fs'),yaml=require('js-yaml'); const doc=yaml.load(fs.readFileSync('yarn.lock','utf8'));const entries=Object.entries(doc).filter(([k])=>k!=='__metadata');for(const [k,v] of entries){if(!v||typeof v.version!=='string'||typeof v.resolution!=='string')throw Error(k)}console.log(JSON.stringify({metadata:doc.__metadata,entries:entries.length}));"
r=subprocess.run([node,'-e',script],cwd=w,capture_output=True,text=True,timeout=30);assert r.returncode==0,r.stderr
print(json.dumps({'node':subprocess.check_output([node,'--version'],text=True).strip(),'node_path':node,'compiled_cli_sha256':hashlib.sha256(cli.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256(fixture).hexdigest(),'cli_controls':results,'root_lock_structural':json.loads(r.stdout),'root_lock_sha256':hashlib.sha256((w/'yarn.lock').read_bytes()).hexdigest(),'limits':['Whole YAML structural parse is not manual reading of every lock entry or locked dependency replay.','Owned CLI fixtures only; root manifests untouched.','Node version captured for this run separately from prior test audit.']},indent=2))
