"""Original full minimal source build, failure semantics only; no speed claims."""
import pathlib,subprocess,json
root=pathlib.Path(__file__).resolve().parents[4];source=root/'.dedup-work/libphash454-20261021';binary=root/'.dedup-work/libphash454-test-benchmark'
subprocess.run(['cc','-O2','-I'+str(source/'src'),'-I'+str(source/'include'),'-DTEST_DATA_DIR="'+str(source/'tests/data')+'"',str(source/'tests/src/test_benchmark.c'),*[str(p) for p in sorted((source/'src').rglob('*.c'))],'-o',str(binary)],check=True,capture_output=True)
missing=source/'deliberately-absent-for-control.png';assert not missing.exists();rows=[]
for mode in ['load','full','hash']:
 r=subprocess.run([str(binary),'--json',mode,str(missing),'1'],capture_output=True,text=True)
 rows.append({'mode':mode,'exit_code':r.returncode,'stdout':r.stdout,'stderr':r.stderr})
assert [r['exit_code'] for r in rows]==[0,0,1]
print(json.dumps({'cases':rows,'method':'Actual unchanged benchmark and all authored source .c files compiled minimal original stb backends; noPH_TESTING, no native codec submodules, noASAN.','finding':'load andfull report success despite all image loads failing; hash correctlyreturns1.','limits':['Reported elapsed times not used as performance evidence','Vendor headers used forbuild but notwhole-source-reviewed here','Not full original testsuite']},indent=2))
