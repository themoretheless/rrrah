"""Finite malformed-output controls for rank region membership/tap auditing."""
import copy,hashlib,json,subprocess,tempfile
from pathlib import Path
base=Path('docs/research');source=base/'dedup-rank-region-real.json';verifier=Path('scripts/verify-dedup-rank-region-real.py');output=base/'dedup-rank-real-auditor-controls.json';assert not output.exists()
digest=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
original=json.loads(source.read_text());pins={str(p):digest(p) for p in [source,verifier,Path(__file__)]}
def sites(r):r['rows'][0]['evidence']['directions'][0]['sites']+=1
def valid(r):r['rows'][0]['evidence']['directions'][0]['valid_sites']-=1
def reads(r):r['rows'][0]['evidence']['directions'][0]['pixel_reads']+=5
def agreement(r):
 d=r['rows'][0]['evidence']['directions'][0];d['agreeing_pairs']=d['informative_pairs']+1
def missing(r):r['rows'].pop()
def membership(r):r['rows'][0]['case']='folded_0'
def retained(r):r['rows'][0]['evidence']['managed_used']=1
def source_hash(r):r['input_hashes'][r['rows'][0]['source']]='0'*64
results=[]
for name,mutation in [('valid',None),('site_count',sites),('valid_count',valid),('read_count',reads),('agreement_overflow',agreement),('missing_case',missing),('case_membership',membership),('retained_memory',retained),('source_hash',source_hash)]:
 report=copy.deepcopy(original)
 if mutation:
  mutation(report)
  for row in report['rows']:row['stdout']=json.dumps(row['evidence'])
 with tempfile.NamedTemporaryFile(prefix='rank-audit-control-',suffix='.json',dir=base,mode='w') as temporary:
  path=Path(temporary.name);audit=path.with_suffix('.audit.json');temporary.write(json.dumps(report));temporary.flush()
  result=subprocess.run(['python3',str(verifier),str(path),str(audit)],capture_output=True,text=True)
  try:
   assert (result.returncode==0)==(mutation is None),(name,result.stderr[-2000:])
   assert audit.exists()==(mutation is None)
  finally:
   if audit.exists():audit.unlink()
 results.append({'case':name,'returncode':result.returncode});print(name,result.returncode,flush=True)
assert all(digest(k)==v for k,v in pins.items())
output.write_text(json.dumps({'status':'passed','controls':results,'input_hashes':pins,'scope':'One accepted valid report and eight rejected malformed controls. No pixel/rank agreement oracle or classifier precision qualification.'},indent=2)+'\n')
