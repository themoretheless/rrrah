"""Run original pinned suite with expectations taken from its Meson registry."""
import pathlib,json,re,urllib.request,hashlib,subprocess,concurrent.futures
root=pathlib.Path(__file__).resolve().parents[4];base=root/'.dedup-work/spng454-20261021';tree=json.loads((base/'tree.json').read_text())
registry=(base/'tests/images/meson.build').read_text();cases=[]
for line in registry.splitlines():
 m=re.search(r"test\('[^']+', test_exe, args: files\('([^']+)'\)(.*)\)",line)
 if m:cases.append((m[1],'should_fail : true' in m[2]))
assert len(cases)==175
manifest={r['path']:r for r in tree['tree']};logs=base/'corpus-logs';logs.mkdir(exist_ok=True)
def download(case):
 name,expected=case;rel='tests/images/'+name;p=base/rel;r=manifest[rel]
 if not p.exists():p.write_bytes(urllib.request.urlopen('https://raw.githubusercontent.com/randy408/libspng/fb768002d4288590083a476af628e51c3f1d47cd/'+rel,timeout=30).read())
 b=p.read_bytes();assert hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==r['sha'],name
 return name,expected,p,hashlib.sha256(b).hexdigest()
with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:files=list(pool.map(download,cases))
rows=[]
for name,expected,p,sha in files:
 run=subprocess.run([str(base/'testsuite'),str(p)],cwd=base,capture_output=True,text=True,timeout=30)
 log=run.stdout+run.stderr;(logs/(name+'.log')).write_text(log)
 m=re.search(r'(\d+) test cases(?: \(skipping (\d+)\))?',log)
 rows.append(dict(image=name,sha256=sha,expected_failure=expected,exit_code=run.returncode,expectation_met=(run.returncode>0 if expected else run.returncode==0),registered=int(m[1]) if m else 0,skipped=int(m[2] or 0) if m else 0,log_sha256=hashlib.sha256(log.encode()).hexdigest()))
result={'rows':rows,'image_runs':len(rows),'expectations_met':sum(r['expectation_met'] for r in rows),'unexpected':[r for r in rows if not r['expectation_met']],'expected_failures':sum(r['expected_failure'] for r in rows),'registered_scenarios':sum(r['registered'] for r in rows),'skipped_scenarios':sum(r['skipped'] for r in rows),'limits':['Original suite on arm64 with system libpng1.6.59, no sanitizer or x86 run.','Images executed but not visually inspected; no whole dependency review claim.','Expected positive exits are invalid-input rejections; they are not successful image decodes.']}
(base/'corpus-result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='rows'},indent=2))
