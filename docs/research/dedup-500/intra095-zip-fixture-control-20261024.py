import ast,collections,hashlib,json,subprocess,sys,zipfile
from pathlib import Path
base=Path(__file__).resolve().parent;src=base/'intra095-20261024';sys.path.insert(0,str(src))
import scanner.hashFile as hf
r=src/'Tests/Test_validateTestArches.py';t=ast.parse(r.read_text());expected=ast.literal_eval(next(n.value for n in t.body if isinstance(n,ast.Assign) and any(isinstance(v,ast.Name) and v.id=='expect' for v in n.targets)))
inv=json.loads((base.parent/'docs/research/dedup-500/intra095-inventory-20261024.json').read_text());archives=[];compared=[]
for f in inv['files']:
 if not f['path'].lower().endswith('.zip'):continue
 p=src/f['path'];assert hashlib.sha256(p.read_bytes()).hexdigest()==f['sha256']
 with zipfile.ZipFile(p) as z:
  assert z.testzip() is None
  members=[]
  for item in z.infolist():
   data=z.read(item)
   members.append(dict(name=item.filename,bytes=len(data),sha256=hashlib.sha256(data).hexdigest(),directory=item.is_dir()))
  archives.append(dict(path=f['path'],sha256=f['sha256'],members=members,crc_ok=True,duplicate_names=len(members)-len({m['name'] for m in members})))
  if p.parent.name=='test_ptree_base' and p.name in expected:
   got=[]
   for item in sorted(z.namelist()):
    data=z.read(item);info=hf.getHashDict(item,data)
    m=subprocess.run(['/usr/bin/file','--mime-type','-b','-'],input=data,capture_output=True,check=True).stdout.decode().strip()
    info['type']='application/CDFV2' if m=='application/CDFV2-corrupt' else m
    got.append((item,info))
   diffs=[]
   for a,b in zip(expected[p.name],got):
    if a[0]!=b[0]:diffs.append(dict(expected_name=a[0],actual_name=b[0]))
    for k in a[1]:
     if a[1][k]!=b[1][k]:diffs.append(dict(member=a[0],field=k,expected=a[1][k],actual=b[1][k]))
   compared.append(dict(archive=p.name,expected_members=len(expected[p.name]),actual_members=len(got),matches=(expected[p.name]==got),differences=diffs))
result=dict(zip_archives=len(archives),members=sum(len(a['members']) for a in archives),archives=archives,expected_archive_comparisons=compared,matched_expectations=sum(c['matches'] for c in compared),scope='Every tracked ZIP CRC/full member-byte reads and SHA256;13original expectation dictionaries replayed via Python zipfile,original getHashDict and macOS file MIME;not original PhashArchive or upstream test invocation;no extraction/visual review/7z decode')
Path(str(base/'intra095-zip-fixture-control-20261024.json')).write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k not in ['archives','expected_archive_comparisons']}));print(json.dumps(compared))
