"""All original tests twice and complete original predict with actual trained components."""
from pathlib import Path
import os,sys,subprocess,json,hashlib,xml.etree.ElementTree as ET,contextlib,io
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/deduplipy99-20261016';S=W/'source';A=W/'full-tests';A.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();runs=[]
for n in range(2):
 junit=A/f'run{n}.xml';env=dict(os.environ,PYTHONPATH=os.pathsep.join(map(str,[W/'sampling-deps',W/'python-deps',S])),PYTEST_DISABLE_PLUGIN_AUTOLOAD='1',OPENBLAS_NUM_THREADS='1',OMP_NUM_THREADS='1');code="import numpy as np,pytest; np.random.seed(1234); raise SystemExit(pytest.main(['tests','-q','--junitxml',"+repr(str(junit))+",'-o','cache_dir='+"+repr(str(A/f'cache{n}'))+"]))";p=subprocess.run([sys.executable,'-c',code],cwd=S,env=env,capture_output=True,text=True,timeout=120);log=A/f'run{n}.log';log.write_text(p.stdout+p.stderr);assert p.returncode==0 and junit.exists(),(p.returncode,p.stdout,p.stderr);root=ET.parse(junit).getroot();cases=[t.get('classname')+'::'+t.get('name') for t in root.iter('testcase')];assert len(cases)==49 and not list(root.iter('failure')) and not list(root.iter('error'));runs.append({'cases':cases,'exit_code':p.returncode,'log':str(log.relative_to(R)),'log_sha256':h(log),'junit':str(junit.relative_to(R)),'junit_sha256':h(junit)})
assert runs[0]['cases']==runs[1]['cases'];os.environ['OPENBLAS_NUM_THREADS']='1';sys.path[:0]=[str(W/'sampling-deps'),str(W/'python-deps'),str(S)]
import numpy as np,pandas as pd
from deduplipy.deduplicator import Deduplicator
from deduplipy.blocking.blocking_rules import whole_field
controls=[]
for n in range(2):
 d=Deduplicator(['name'],rules={'name':[whole_field]});d.myActiveLearner.learner.teach([[0.,0.],[100.,100.]],[0,1])
 with contextlib.redirect_stdout(io.StringIO()):d.myBlocker.fit(pd.DataFrame({'name_1':['alpha','beta'],'name_2':['alpha ','gamma']}),np.array([1,0]))
 rows=[]
 for name,names,threshold in [('identical_only',['alpha','alpha'],.1),('distinct_only',['alpha','beta'],.1),('all_scores_filtered',['alpha','alpha ','beta'],1.)]:
  failed=False
  try:d.predict(pd.DataFrame({'name':names}),score_threshold=threshold,fill_missing=False)
  except ValueError:failed=True
  assert failed,name;rows.append({'case':name,'ValueError':failed})
 result=d.predict(pd.DataFrame({'name':['alpha','alpha ','beta']}),fill_missing=False);assert len(result)==3 and result.deduplication_id.nunique()==2;rows.append({'case':'matched_pair_plus_singleton_positive','rows':len(result),'groups':int(result.deduplication_id.nunique())});controls.append(rows)
assert controls[0]==controls[1]
p=R/'docs/research/dedup-500/deduplipy99-full-tests-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_all49_original_tests_twice_and4_full_predict_controls_twice','producer_sha256':h(Path(__file__)),'runs':runs,'predict_controls':controls,'artifact_pins':{str(p.relative_to(R)):h(p) for p in A.iterdir() if p.is_file()},'scope':'All49unchangedoriginalPytest tests twice incloriginalCSV/500x500case(seed1234), actualdependencies. FullunchangedDeduplicator.predict4controls twice withreallearnertrainedonsynthetic2featurevectors/actualBlockingfit. No estimator/API/bodyadapters; interactivefullfit notrunhere (previousstage actualmodALfit withstdinqueue separate).','findings':['Completepredict raisesValueError foridentical-only/distinct-only no-candidate tables andall-scores-filtered case. Nativefullflow extends priorhelper diagnosis.','Matchednonidenticalwhitespacepair plus singleton succeeds3rows/2groups positivecontrol.','Alloriginaltests pass but do notcover thesecompletepredict boundarycases.'],'limits':['ResolvedunlockedMacARM/Python3.12env nothistoricalrelease/allplatform/largecorpus/quality/performance/adoption qualification.','Synthetictrained model demonstratescontrolflow only, notreal entity/image accuracy orend-to-endhumanfit quality. No userfiles/network/productioncodecopy.']},indent=2)+'\n');print(json.dumps({'status':'verified','upstream_passes':98,'full_predict_controls':8}))
