"""Pinned aggregate scoring helper on synthetic measurements, not performance data."""
import copy
import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
BASE=Path(__file__).resolve().parent
EV=BASE/'evidence/Hao0321--claude-skill-code-cleanup'
script=BASE.parents[2]/'.dedup-work/cleanup067-20261023/run-benchmark-driven-rd/scripts/score_gate.py'
data=script.read_bytes();pin=next(x for x in json.loads((EV/'tree.json').read_text())['tree'] if x['path']=='run-benchmark-driven-rd/scripts/score_gate.py')
assert hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()==pin['sha']
spec=importlib.util.spec_from_file_location('rrrah_cleanup067_score',script);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
r=subprocess.run([sys.executable,str(script),'--self-test'],capture_output=True,text=True,timeout=10);assert r.returncode==0,r.stderr
protocol={'runsPerScenario':30,'metricVersion':'1','stableFrames':3,'acquireDeadlineMs':350,'reacquireDeadlineMs':450,'poseTolerancePx':3}
baseline={'status':'measured','engine':{'device':'fixture'},'dataset':{'id':'authored','hash':'sha256:'+'a'*64,'blinded':True},'protocol':protocol,'aggregate':{'latency':20.}}
candidate=copy.deepcopy(baseline);candidate['aggregate']['latency']=17.
config={'minRunsPerScenario':30,'gates':[{'metric':'latency','direction':'lower','relativeImprovement':.1,'absoluteCeiling':18.}]}
assert module.evaluate(candidate,baseline,config)==[]
bad=copy.deepcopy(candidate);bad['aggregate']['latency']=21.;assert module.evaluate(bad,baseline,config)
nan=copy.deepcopy(candidate);nan['aggregate']['latency']=float('nan');assert module.evaluate(nan,baseline,config)==[]
short=copy.deepcopy(baseline);short['protocol']['runsPerScenario']=1;assert module.evaluate(candidate,short,config)==[]
unblinded=copy.deepcopy(baseline);unblinded['dataset']['blinded']=False;assert module.evaluate(candidate,unblinded,config)==[]
noid=copy.deepcopy(candidate);noid['dataset'].pop('id');noidbase=copy.deepcopy(baseline);noidbase['dataset'].pop('id');assert module.evaluate(noid,noidbase,config)==[]
assert module.evaluate(bad,baseline,{**config,'gates':[]})==[]
malformed=copy.deepcopy(candidate);malformed['aggregate']['latency']='not-a-number'
try:module.evaluate(malformed,baseline,config)
except ValueError:pass
else:raise AssertionError('non-number not rejected')
print(json.dumps({'pinned_module_verified':True,'original_cli_self_test_passed':True,'additional_controls_passed':8,'ordinary_regression_rejected':True,'nan_metric_passes':True,'baseline_one_run_passes':True,'baseline_unblinded_passes':True,'both_dataset_ids_missing_pass':True,'empty_gates_ignore_regression':True,'malformed_metric_raises_value_error':True,'real_benchmark_or_speedup_claim':False},indent=2))
