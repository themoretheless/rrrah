import pathlib,tempfile,json,io,contextlib,subprocess,sys
from unittest.mock import patch
import orchestrate
results=[]
with tempfile.TemporaryDirectory(prefix="autograph104-orchestrate-owned-") as tmp:
 root=pathlib.Path(tmp);(root/'schema.json').write_text(json.dumps({'node_types':{}}))
 p=subprocess.run([sys.executable,str(pathlib.Path(__file__).parent/'enforce.py'),str(root),'--apply'],capture_output=True,text=True)
 assert p.returncode!=0 and '--apply' in p.stderr and 'FileNotFoundError' in p.stderr
 results.append({'control':'bootstrap-enforce-argv-missing-schema','exit_code':p.returncode,'error':'FileNotFoundError for schema path --apply'})
 with patch('orchestrate.run_script',return_value=(7,'owned failure')) as run,contextlib.redirect_stdout(io.StringIO()):
  try:orchestrate.main(['bootstrap',str(root)])
  except SystemExit as e:raise AssertionError('unexpected propagated failure')
 assert run.call_count==1
 results.append({'control':'bootstrap-failure-not-propagated-by-cli','first_step_exit':7,'main_returned_normally':True})
 d=root/'.graph';d.mkdir();(d/'vault-graph.json').write_text(json.dumps({'stats':{'health_score':88},'nodes':{'stale':{'type':'note'}}}))
 with patch('orchestrate.run_script',return_value=(9,'owned graph failure')),contextlib.redirect_stdout(io.StringIO()):orchestrate.cmd_graph_prepare(root)
 out=json.loads((d/'graph-analysis-input.json').read_text());assert out['sample_nodes'][0]['path']=='stale' and out['stats']['health_score']==88
 results.append({'control':'graph-prepare-reuses-stale-report-after-failure','failed_step_exit':9,'stale_node_retained':True})
print(json.dumps({'passed':3,'author_controls':results,'scope':'Original orchestrator helpers, mocked failing child jobs and one actual enforce CLI in owned temp vault; no agents/network or user data'},indent=2))
