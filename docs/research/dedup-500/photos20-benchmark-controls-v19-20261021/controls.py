import json, pathlib, subprocess, sys, tempfile
root=pathlib.Path(sys.argv[1]).resolve();node=sys.argv[2]
cases=[('known-regression', [{'name':'render','mean':121}],1),('missing-benchmark',[],0),('duplicate-name-overwrites-regression',[{'name':'render','mean':1000},{'name':'render','mean':100}],0)]
for name,benches,expected in cases:
 with tempfile.TemporaryDirectory() as tmp:
  p=pathlib.Path(tmp);(p/'tests/perf').mkdir(parents=True)
  (p/'bench-output.json').write_text(json.dumps({'files':[{'groups':[{'benchmarks':benches}]}]}))
  (p/'tests/perf/bench-history.json').write_text(json.dumps([{'means':{'render':100}}]*3))
  r=subprocess.run([node,str(root/'tools/check-bench.mjs')],cwd=p,capture_output=True,text=True)
  assert r.returncode==expected,(name,r.returncode,r.stdout,r.stderr)
  print(json.dumps({'name':name,'exit_code':r.returncode,'stdout':r.stdout.strip(),'stderr':r.stderr.strip(),'passed':True}))
