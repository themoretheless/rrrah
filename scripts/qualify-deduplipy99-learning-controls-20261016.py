"""Whole original classifier/active learner/core; actual sklearn/modAL and synthetic stdin only."""
from pathlib import Path
import sys,os,json,hashlib,subprocess,xml.etree.ElementTree as ET,contextlib,io,builtins,importlib.metadata as md
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/deduplipy99-20261016';S=W/'source';A=W/'learning-controls';A.mkdir(exist_ok=False);h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();os.environ['OPENBLAS_NUM_THREADS']='1';sys.path[:0]=[str(W/'sampling-deps'),str(W/'python-deps'),str(S)]
import numpy as np,pandas as pd
from sklearn.exceptions import NotFittedError
from deduplipy.classifier_pipeline import ClassifierPipeline
from deduplipy.active_learning import ActiveStringMatchLearner
from deduplipy.active_learning.utils_active_learning import input_assert
from deduplipy.deduplicator import Deduplicator
from deduplipy.blocking.blocking_rules import first_word
upstream=[];runs=[]
for n in range(2):
 np.random.seed(1234);junit=A/f'upstream{n}.xml';env=dict(os.environ,PYTHONPATH=os.pathsep.join(map(str,[W/'sampling-deps',W/'python-deps',S])),PYTEST_DISABLE_PLUGIN_AUTOLOAD='1',OPENBLAS_NUM_THREADS='1');p=subprocess.run([sys.executable,'-m','pytest','tests/test_classifier_pipeline','-q','--junitxml',str(junit),'-o','cache_dir='+str(A/f'cache{n}')],cwd=S,env=env,capture_output=True,text=True,timeout=30);log=A/f'upstream{n}.log';log.write_text(p.stdout+p.stderr);assert p.returncode==0 and len(list(ET.parse(junit).getroot().iter('testcase')))==1;upstream.append({'exit_code':p.returncode,'log':str(log.relative_to(R)),'log_sha256':h(log),'junit':str(junit.relative_to(R)),'junit_sha256':h(junit)})
 rows=[]
 def check(name,value,want):assert value==want,(name,value,want);rows.append({'case':name,'observed':value})
 X=np.array([[0.],[1.]]);c=ClassifierPipeline();c.fit(X,[0,0]);failed=False
 try:c.predict(X)
 except NotFittedError:failed=True
 check('single_class_initial_fit_stays_unfitted',failed,True)
 c.fit(X,[0,1]);before=c.predict_proba(X);c.fit(X,[1,1]);check('single_class_refit_preserves_old_model',bool(np.array_equal(before,c.predict_proba(X))),True)
 c=ClassifierPipeline();c.fit(X,pd.DataFrame({'match':[0,1]}));failed=False
 try:c.predict(X)
 except NotFittedError:failed=True
 check('dataframe_binary_labels_misread_as_single_class',failed,True)
 failed=False
 try:ClassifierPipeline().get_params()
 except AttributeError:failed=True
 check('sklearn_get_params_missing_interaction_attribute',failed,True)
 d=Deduplicator(['name'],rules=[first_word]);check('custom_rule_list_ignored_uses_all_rules',[len(d.rules_info['name']),d.rules_info['name'][0].__name__],[22,'whole_field'])
 failed=False
 try:Deduplicator._add_singletons(pd.DataFrame({'name':['a','b'],'deduplication_id':[np.nan,np.nan]}))
 except ValueError:failed=True
 check('all_singletons_nan_max_raises',failed,True)
 result=Deduplicator._add_singletons(pd.DataFrame({'name':['a','b'],'deduplication_id':[5.,np.nan]}));check('mixed_singleton_ids_positive_control',result.deduplication_id.tolist(),[5.,6.])
 active=ActiveStringMatchLearner(['name']);active.uncertainties=[.05]*6;v=active._get_last_uncertainty_improvement();check('perfect_plateau_improvement_is_falsy_zero',[float(v),bool(v)],[0.,False])
 frame=pd.DataFrame({'name_1':['synthetic','zero','middle','twenty'],'name_2':['synthetic','other-zero','other-middle','other-twenty'],'similarities':[[100.],[0.],[50.],[20.]],'synthetic_perfect_match':[True,False,False,False]})
 active=ActiveStringMatchLearner(['name']);output=io.StringIO();answers=iter(['n','p','y','f']);prompts=[];original_input=builtins.input
 def answer(_):
  seen=[x[2:].strip() for x in output.getvalue().splitlines() if x.startswith('->')];prompts.append(seen[-2:]);return next(answers)
 builtins.input=answer
 try:
  with contextlib.redirect_stdout(output):active.fit(frame.copy())
 finally:builtins.input=original_input
 manual=active.train_samples[active.train_samples['y'].notna()];assert len(manual)==2 and len(prompts)==4 and prompts[0]==prompts[2] and prompts[0]!=prompts[1];check('previous_answer_labels_current_not_previous',[manual.y.tolist(),bool(manual.iloc[0].name_1 in prompts[0][0]),bool(manual.iloc[1].name_1 in prompts[1][0])],[[0.,1.],True,True]);(A/f'previous-run{n}.log').write_text(output.getvalue())
 active=ActiveStringMatchLearner(['name']);builtins.input=lambda _:'p';failed=False
 try:
  with contextlib.redirect_stdout(io.StringIO()):active.fit(frame.copy())
 except TypeError:failed=True
 finally:builtins.input=original_input
 check('previous_on_first_query_raises',failed,True)
 answers=iter([' y ','Y']);builtins.input=lambda _:next(answers)
 try:
  with contextlib.redirect_stdout(io.StringIO()):value=input_assert('', ['y','n'])
 finally:builtins.input=original_input
 check('input_lowercase_but_no_whitespace_trim',value,'y')
 assert len(rows)==11;runs.append(rows)
assert runs[0]==runs[1]
paths=[*list((S/'deduplipy/classifier_pipeline').glob('*.py')),*list((S/'deduplipy/active_learning').glob('*.py')),*list((S/'deduplipy/deduplicator').glob('*.py')),*list((S/'tests/test_classifier_pipeline').glob('*.py'))]
versions={name:md.version(name) for name in ['numpy','pandas','scikit-learn','modAL-python']}
p=R/'docs/research/dedup-500/deduplipy99-learning-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_original_classifier_tests_and11_native_learning_controls_twice','producer_sha256':h(Path(__file__)),'source_pins':{str(p.relative_to(R)):h(p) for p in paths},'dependency_versions':versions,'loaded_dependency_file_pins':{str(Path(v.__file__)):h(Path(v.__file__)) for v in list(sys.modules.values()) if getattr(v,'__file__',None) and any(v.__name__==prefix or v.__name__.startswith(prefix+'.') for prefix in ['numpy','pandas','sklearn','modAL']) and Path(v.__file__).is_file()},'upstream_tests':upstream,'runs':runs,'artifact_pins':{str(p.relative_to(R)):h(p) for p in A.iterdir() if p.is_file()},'scope':'Wholeunchanged classifier/activelearner/deduplicator modules imported, realmodAL0.4.2.1/scikit-learn/Pandas/NumPy. OriginalclassifierPytest case twice.11nativecontrols twice includingfulloriginalactivefit loop. Onlybuiltins.input replaced withownedanswerqueue, restoredfinally; no learner/estimator/bodyadapters.','findings':['Previouscommand redisplayspriorpair butteacheslabel tocurrentpair; firstqueryprevious raisesTypeError. NativeactualmodALfit trace retained.','Singleclassfit leavesunfittedoroldmodel; DataFramebinarylabels skippedasoneclass; sklearnget_params failsmissinginteractionattribute.','PublicDeduplicator ruleslist ignoredin favorofall22rules; all-singletons helperraises dueNaNmax, mixedhelperpositive control passes.','Zero uncertainty improvement falsy so convergencehint guard skipsperfectplateau. Nativehelper+staticguard finding, notend-to-endconvergence proof.'],'limits':['Selectedcore modules only; no full62blobsource/media/LICENSE/doc/data review orfullupstreamsuite, realentity/image precision/recall/performance/platform/adoption.','Syntheticinteractiveanswers/featurevectors areonlyownedfixtures, no humanlabel/service/network/userdata.','Corepredict/all-singleton fullflow notexecuted; directoriginalhelper failure only. No productioncodecopy.']},indent=2)+'\n');print(json.dumps({'status':'verified','upstream_passes':2,'native_controls':22,'versions':versions}))
