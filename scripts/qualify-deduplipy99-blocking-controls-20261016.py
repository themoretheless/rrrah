"""Whole original blocking/set-cover bodies, actual Pandas/scikit-learn, owned scored pairs."""
from pathlib import Path
import sys,json,hashlib,contextlib,io,os
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/deduplipy99-20261016';S=W/'source';os.environ['OPENBLAS_NUM_THREADS']='1';sys.path[:0]=[str(W/'python-deps'),str(S)]
import pandas as pd,numpy as np,sklearn
from deduplipy.blocking import Blocking,greedy_set_cover
from deduplipy.blocking.blocking_rules import first_word,last_word,whole_field,three_letter_abbreviation,first_three_letters_no_space
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();runs=[]
for n in range(2):
 rows=[]
 def check(name,value,want):assert value==want,(name,value,want);rows.append({'case':name,'observed':value})
 with contextlib.redirect_stdout(io.StringIO()):normal=greedy_set_cover([{0,1},{1,2}],{0,1,2});outside=greedy_set_cover([{0,99},{1}],{0,1},1.0)
 check('normal_cover_complete',sorted(set().union(*normal)),[0,1,2]);check('outside_universe_stops_below_requested_recall',[sorted(set().union(*outside)),len(set().union(*outside)&{0,1})/2],[[0,99],.5])
 error=False
 try:
  with contextlib.redirect_stdout(io.StringIO()):greedy_set_cover([{0}],{0,1},1.)
 except IndexError:error=True
 check('uncoverable_universe_raises_indexerror',error,True)
 check('empty_universe_returns_empty',greedy_set_cover([{0}],set()),[])
 X=pd.DataFrame({'name_1':['alpha one','beta end','alpha zero'],'name_2':['alpha two','gamma end','alpha other']});y=np.array([1,1,0]);block=Blocking(['name'],{'name':[first_word,last_word]},recall=1.)
 with contextlib.redirect_stdout(io.StringIO()):block.fit(X,y)
 covered=set().union(*block.cover);check('blocking_recall1_training_positive_pair_omitted',[len(covered&set(block.matches))/len(block.matches),[r['function_name'] for r in block.rules_selected]],[.5,['first_word']])
 records=pd.DataFrame({'name':['alpha one','alpha two','beta end','gamma end'],'row_number':[0,1,2,3]});pairs=block.transform(records);check('learned_blocking_omits_positive_beta_gamma',bool(((pairs.row_number_1==2)&(pairs.row_number_2==3)).any()),False)
 train=X.copy();train.index=[40,50,60];b=Blocking(['name'],{'name':[first_word,last_word]},recall=1.)
 with contextlib.redirect_stdout(io.StringIO()):b.fit(train,y)
 check('nondefault_training_index_loses_coverage',len(set().union(*b.cover)&set(b.matches)),0)
 one=Blocking(['name'],{'name':[whole_field]})
 with contextlib.redirect_stdout(io.StringIO()):one.fit(pd.DataFrame({'name_1':['same'],'name_2':['same']}),np.array([1]))
 good=pd.DataFrame({'name':['same','same'],'row_number':[0,1]});shifted=good.copy();shifted.index=[10,20];check('nondefault_transform_index_drops_identical_pair',[len(one.transform(good)),len(one.transform(shifted))],[1,0])
 check('non_ascii_abbreviation_none',three_letter_abbreviation('東京 京都 大阪'),None)
 check('no_space_rule_keeps_tabs',first_three_letters_no_space('a\tb c'),'a\tb')
 assert len(rows)==10;runs.append(rows)
assert runs[0]==runs[1]
p=R/'docs/research/dedup-500/deduplipy99-blocking-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_complete_blocking_modules_10_native_controls_twice','producer_sha256':h(Path(__file__)),'source_pins':{str(p.relative_to(R)):h(p) for p in [*list((S/'deduplipy/blocking').glob('*.py')),S/'deduplipy/config.py',S/'deduplipy/__init__.py']},'dependency_versions':{'numpy':np.__version__,'pandas':pd.__version__,'scikit-learn':sklearn.__version__},'loaded_dependency_file_pins':{str(Path(v.__file__)):h(Path(v.__file__)) for v in list(sys.modules.values()) if getattr(v,'__file__',None) and any(v.__name__==prefix or v.__name__.startswith(prefix+'.') for prefix in ['numpy','pandas','sklearn']) and Path(v.__file__).is_file()},'runs':runs,'scope':'Whole unchanged Blocking/all22rules/greedy_set_cover/config/init modules imported, actualPandas/NumPy/scikit-learn.10nativeboundarycontrols twice, no API/bodyadapters/extraction/network. Resolvedunlockeddepsrecorded.','findings':['Greedysetcover includesoutside-universe elements andwhile strictsubsetcondition exitsat50%coverage despite recall1.0; actualBlockingfit withone negative matchingrule reproduces50%trainingrecall andomits truepair.','Nondefaulttrainingindex mixespositionalrule indices withoriginalpositiveindex,coverage0; nondefaulttransformindex alignsnewSeries0..N tocallerindex anddropsidenticalpair.','Uncoverableuniverse exhaustsheap andraisesIndexError; standardcoveredcase/emptyuniverse controls pass. ASCIIabbreviation andliteral-space rules have explicitUnicode/tab limitations.'],'limits':['Onlyblocking+priorclustering source subset, notwhole62blob source/docs/data/notebook/media review; no fullsource countincrease.','Trainingrecall boundary fixtures do notmeasure generalentity/image precision/recall orscale/performance/hardRSS/platform/currentlibrary/adoption.','No realuserdata/externalmessages/network or productioncodecopy.']},indent=2)+'\n');print(json.dumps({'status':'verified','controls':20}))
