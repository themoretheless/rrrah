"""Run original standalone tests against actual minimal library sources."""
import pathlib,json,subprocess
root=pathlib.Path(__file__).resolve().parents[4];source=root/'.dedup-work/libphash454-20261021';results=[]
for name in ['test_hash_logic','test_image_primitives']:
 binary=root/('.dedup-work/libphash454-'+name)
 subprocess.run(['cc','-O2','-I'+str(source/'src'),'-I'+str(source/'include'),'-I'+str(source/'tests/src'),str(source/'tests/src'/f'{name}.c'),*[str(p) for p in sorted((source/'src').rglob('*.c'))],'-o',str(binary)],check=True,capture_output=True)
 run=subprocess.run([str(binary)],capture_output=True,text=True)
 results.append({'test':name,'exit_code':run.returncode,'named_checks_passed':run.stdout.count('[PASS]'),'stdout':run.stdout,'stderr':run.stderr})
 assert run.returncode==0,results[-1]
print(json.dumps({'results':results,'method':'Unchanged original tests and complete authored .c sources; AppleClang-O2 minimalstb backend; noPH_TESTING,noASAN.','limits':['Not all upstream testexecutables','Someedge tests onlycheck completion and use uninitializedsynthetic source arrays; no pixelcorrectness claim from thosecases','No codecsubmodule or speedqualification']},indent=2))
