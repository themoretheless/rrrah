#!/usr/bin/env python3
from pathlib import Path
import json,hashlib,shutil,subprocess,os,sys,importlib.util,contextlib,io
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';E=D/'evidence/DustinWehr--minimal-bitwarden-dedup';F=Path('/private/tmp/rrrah-bitwarden128-20261015');F.mkdir(exist_ok=False);(F/'test_resources').mkdir();mapping={'bitwarden_dedup.py':'02-bitwarden_dedup.py','runtests.py':'03-runtests.py','test_resources/expected_deduped.json':'05-expected_deduped.json','test_resources/fake_bitwarden_export.json':'06-fake_bitwarden_export.json','test_resources/fake_bitwarden_export2.json':'07-fake_bitwarden_export2.json'}
for target,source in mapping.items():shutil.copyfile(E/source,F/target)
env=os.environ.copy();env['PYTHONDONTWRITEBYTECODE']='1';upstream=subprocess.run([sys.executable,'runtests.py','-v'],cwd=F,env=env,capture_output=True,text=True);(F/'upstream.log').write_text(upstream.stdout+upstream.stderr);assert upstream.returncode==0 and 'Ran 2 tests'in upstream.stderr and 'OK'in upstream.stderr
spec=importlib.util.spec_from_file_location('bitwarden128',F/'bitwarden_dedup.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);rows=[]
def run(label,items,expected=None,error=None,encrypted=False):
 p=F/(label+'-input.json');out=F/(label+'-output.json');p.write_text(json.dumps({'encrypted':encrypted,'folders':[],'items':items}));caught=None
 try:
  with contextlib.redirect_stdout(io.StringIO()):module.dedup(str(p),str(out))
 except Exception as ex:caught=type(ex).__name__
 if error:assert caught==error
 else:
  assert caught is None;assert len(json.loads(out.read_text())['items'])==expected
 rows.append(dict(label=label,retained=expected,error=caught))
run('different-ids-same-data',[{'id':'1','foo':'x'},{'id':'2','foo':'x'}],1)
run('same-id-different-data',[{'id':'1','foo':'x'},{'id':'1','foo':'y'}],2)
run('top-level-order',[{'id':'1','a':1,'b':2},{'id':'2','b':2,'a':1}],2)
run('id-key-position',[{'id':'1','a':1},{'a':1,'id':'2'}],2)
run('nested-order',[{'id':'1','data':{'a':1,'b':2}},{'id':'2','data':{'b':2,'a':1}}],2)
run('whitespace',[{'id':'1','foo':' x '},{'id':'2','foo':'x'}],2)
run('integer-float',[{'id':'1','foo':1},{'id':'2','foo':1.0}],2)
run('missing-id',[{'foo':'x'}],error='KeyError')
run('encrypted-refused',[],error='AssertionError',encrypted=True)
run('empty',[],0)
run('null-items',None,error='TypeError')
run('numeric-zero-encrypted',[],0,encrypted=0)
run('encrypted-optimized-fixture',[],error='AssertionError',encrypted=True)
p=F/'encrypted-optimized-fixture-input.json';out=F/'optimized-output.json';code='import bitwarden_dedup,sys;bitwarden_dedup.dedup(sys.argv[1],sys.argv[2])';optimized=subprocess.run([sys.executable,'-O','-c',code,str(p),str(out)],cwd=F,env=env,capture_output=True,text=True);assert optimized.returncode==0 and json.loads(out.read_text())['encrypted']is True;rows.append(dict(label='optimized-python-bypasses-encrypted-assert',retained=0))
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();O=D/'bitwarden128-runtime-controls-20261015.json';assert not O.exists();O.write_text(json.dumps(dict(status='passed2upstream_tests_and14synthetic_controls',upstream_tests=2,synthetic_controls=len(rows),rows=rows,source_pins={str(E/source):sha(E/source)for source in mapping.values()},producer_sha256=sha(__file__),upstream_log=str(F/'upstream.log'),upstream_log_sha256=sha(F/'upstream.log'),scope='Unchanged original module and2upstream unit tests on captured public fake JSON fixtures plus authored synthetic JSON controls, original module imported with unchanged function body. Python-O control demonstrates assert admission bypass. Owned temporary files only; no real vault/account/credentials/network/decryption/importback, RAMdisk/system changes or source adoption.'),indent=2)+'\n');print('2upstream tests and',len(rows),'synthetic controls passed')
