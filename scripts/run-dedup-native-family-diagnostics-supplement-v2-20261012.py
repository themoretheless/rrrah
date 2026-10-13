"""Run the correctly named native-family tests from the newly compiled binary."""
import pathlib,json,subprocess,re,time,hashlib,os
R=pathlib.Path(__file__).resolve().parents[1];D=R/'docs/research';P=D/'dedup-native-family-diagnostics-supplement-v2-20261012.json';Q=D/'dedup-model-miss-diagnostics-20261012.json';W=pathlib.Path('/Users/themoretheless/.codex/tmp/rrrah-model-miss-diagnostics-20261012')
assert not P.exists();data={'status':'waiting_for_compiled_search_portfolio','producer_pid':os.getpid()}
def save():P.write_text(json.dumps(data,indent=2)+'\n')
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
save()
while True:
 d=json.loads(Q.read_text());g=next((g for g in d['gates'] if g['test']=='search_portfolio' and 'returncode' in g),None)
 if g:break
 assert d['status']!='failed','preceding build failed'
 time.sleep(2)
assert g['returncode']==0
binary=re.search(r'Running tests/[^\n]+ \(([^)]+)\)',pathlib.Path(g['log']).read_text())[1]
listing=subprocess.check_output([binary,'--list','native_family_first'],text=True);assert listing.count(': test')==4,listing
log=W/'native-family-supplement-v2.log';data.update(status='running',binary=binary,binary_sha256=sha(binary),listing=listing,command=[binary,'native_family_first'],log=str(log));save()
with log.open('x') as out:
 child=subprocess.Popen(data['command'],stdout=out,stderr=subprocess.STDOUT);data['child_pid']=child.pid;save();code=child.wait()
text=log.read_text();data.update(status='passed' if code==0 and 'test result: ok. 4 passed; 0 failed; 0 ignored;' in text else 'failed',returncode=code,log_sha256=sha(log));save();print(data['status'])
