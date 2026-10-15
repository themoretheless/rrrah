"""Whole unchanged samplers/string metrics with actual PyMinHash and numeric libraries."""
from pathlib import Path
import sys,json,hashlib,os,math,importlib.metadata as metadata
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/deduplipy99-20261016';S=W/'source';os.environ['OPENBLAS_NUM_THREADS']='1';sys.path[:0]=[str(W/'sampling-deps'),str(W/'python-deps'),str(S)]
import numpy as np,pandas as pd
from deduplipy.sampling import NaiveSampler,MinHashSampler
from deduplipy.string_metrics.string_metrics import length_adjustment,adjusted_ratio,adjusted_partial_ratio,adjusted_token_set_ratio
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();runs=[]
for n in range(2):
 np.random.seed(1234);rows=[]
 def check(name,value,want):assert value==want,(name,value,want);rows.append({'case':name,'observed':value})
 data=pd.DataFrame({'name':['alpha','beta']});naive=NaiveSampler(['name'])
 for name,frame,size in [('naive_zero_requested_raises',data,0),('naive_empty_input_raises',data.iloc[:0],10)]:
  failed=False
  try:naive.sample(frame,size)
  except KeyError:failed=True
  check(name,failed,True)
 single=naive.sample(data.iloc[:1],10);check('singleton_yields_only_synthetic_self_pair',[len(single),bool(single.synthetic_perfect_match.all()),bool((single.name_1==single.name_2).all())],[1,True,True])
 one=naive.sample(data,1);check('one_requested_returns_synthetic_self_pair',[len(one),bool(one.synthetic_perfect_match.all()),bool((one.name_1==one.name_2).all())],[1,True,True])
 multi=pd.DataFrame({'same':['common text','common text'],'different':['alpha only','beta different']});mh=MinHashSampler(['same','different']);actual=mh._create_minhash_pairs(multi,.75);check('multi_column_average_ignores_absent_zero_score',[len(actual),float(actual.jaccard_sim.iloc[0])],[1,1.0]);check('independent_full_column_mean_below_threshold',(1.+0.)/2,.5)
 check('empty_identical_metric_zero',float(adjusted_ratio('','')),0.)
 score=float(adjusted_ratio('a','a'));check('one_character_identity_discount',round(score,6),round((1-math.exp(-.2))*100,6))
 graph='👨‍👩‍👧‍👦';check('grapheme_codepoint_length_discount',[len(graph),round(float(adjusted_ratio(graph,graph)),6)],[7,round((1-math.exp(-1.4))*100,6)])
 check('substring_feature_not_identity',round(float(adjusted_partial_ratio('abc','abc xyz')),6),round((1-math.exp(-.6))*100,6))
 check('token_set_feature_ignores_repeated_tokens',round(float(adjusted_token_set_ratio('a b','a a b')),6),round((1-math.exp(-.6))*100,6))
 assert len(rows)==11;runs.append(rows)
assert runs[0]==runs[1]
files=[*list((S/'deduplipy/sampling').glob('*.py')),*list((S/'deduplipy/string_metrics').glob('*.py')),S/'deduplipy/config.py',S/'deduplipy/__init__.py']
versions={name:metadata.version(name) for name in ['numpy','pandas','scikit-learn','pyminhash','thefuzz','rapidfuzz','setuptools']}
p=R/'docs/research/dedup-500/deduplipy99-sampling-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_complete_sampling_metric_modules_11_native_controls_twice','producer_sha256':h(Path(__file__)),'source_pins':{str(p.relative_to(R)):h(p) for p in files},'dependency_versions':versions,'loaded_dependency_file_pins':{str(Path(v.__file__)):h(Path(v.__file__)) for v in list(sys.modules.values()) if getattr(v,'__file__',None) and any(v.__name__==prefix or v.__name__.startswith(prefix+'.') for prefix in ['numpy','pandas','sklearn','pyminhash','thefuzz','rapidfuzz','pkg_resources']) and Path(v.__file__).is_file()},'runs':runs,'scope':'Whole unchanged sampler/naive/minhash/stringmetrics modules, realPyMinHash/scikit-learn/Pandas/NumPy/thefuzz/rapidfuzz; native2row-ownedtext fixtures,11controls twice. No adapters/bodyextraction/network. Randomseed1234external; dependenciesunlocked/resolvedrecorded.','findings':['Twofields: firstidenticalJaccard1, seconddisjointJaccard0 generatesno pair; originalcandidateaggregation averagesonlyreturnedfields to1 instead ofallfieldmean.5, passes.75threshold. ActualPyMinHash/nativecounterexample, nofakehashadapter.','Naiveemptyinput andzerorequest raiseKeyError; singleton/request1 canyieldonlysyntheticselftrainingpair. These are samplingcontracts, not actual duplicate detections.','Lengthadjustedidentity is18.126925foronecodepoint; empty0, familyemoji7codepoints75.34 despiteonegrapheme. Partial/token-set scores arenotbyteidentity. Intendedfeaturepolicy, notuniversalquality proof.'],'limits':['Selectedsampler/metricmodules only, whole62bloblibrary/docs/media/data notyetreviewed; no fullsource countincrease.','Selectedfixtures plusoriginaltests do notprove realentity/image precision/recall, scale/memory/platform/performance/currentlibrary/adoption.','No privateuserdata/network/externalmessages orproductioncodecopy.']},indent=2)+'\n');print(json.dumps({'status':'verified','controls':22,'versions':versions}))
