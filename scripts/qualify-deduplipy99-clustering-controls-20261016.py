"""Unchanged clustering/imputer bodies with real numeric dependencies and synthetic scored pairs."""
from pathlib import Path
import sys,json,hashlib,os
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/deduplipy99-20261016';S=W/'source';os.environ['OPENBLAS_NUM_THREADS']='1';sys.path[:0]=[str(W/'python-deps'),str(S)]
import numpy as np,pandas as pd,scipy,networkx,fancyimpute
from deduplipy.clustering import hierarchical_clustering,fill_missing_links
h=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();runs=[]
def pairs(edges):return pd.DataFrame([{'row_number_1':a,'row_number_2':b,'name_1':str(a),'name_2':str(b),'score':score} for a,b,score in edges])
for n in range(2):
 rows=[]
 def check(name,value,want):assert value==want,(name,value,want);rows.append({'case':name,'observed':value})
 a=np.array([[0.,.8,0.],[.8,0.,.8],[0.,.8,0.]]);copy=a.copy();filled=fill_missing_links(a);check('known_zero_also_treated_missing',bool(filled[0,2]>.85),True);check('imputation_input_not_mutated',bool(np.array_equal(a,copy)),True)
 data=pairs([(0,1,.8),(1,2,.8),(0,2,0.)]);yes=hierarchical_clustering(data,['name'],fill_missing=True);no=hierarchical_clustering(data,['name'],fill_missing=False);check('explicit_zero_rejection_imputed_into_single_cluster',[int(yes.deduplication_id.nunique()),int(no.deduplication_id.nunique())],[1,2])
 data=pairs([(0,1,.9),(0,2,.7),(1,2,.49)]);res=hierarchical_clustering(data,['name'],cluster_threshold=.5,fill_missing=False);check('centroid_cluster_contains_below_threshold_pair',int(res.deduplication_id.nunique()),1)
 a=np.array([[0.,.8],[.8,0.]]);result=fill_missing_links(a);result[0,1]=.2;check('no_missing_returns_input_alias',[result is a,float(a[0,1])],[True,.2])
 a=np.array([[0.,.8],[.7,0.]]);result=fill_missing_links(a);check('no_missing_asymmetric_input_remains_asymmetric',bool(result[0,1]!=result[1,0]),True)
 res=hierarchical_clustering(pairs([(0,1,.9),(0,1,.1)]),['name'],fill_missing=False);other=hierarchical_clustering(pairs([(0,1,.1),(0,1,.9)]),['name'],fill_missing=False);check('duplicate_edge_last_score_wins',[int(res.deduplication_id.nunique()),int(other.deduplication_id.nunique())],[2,1])
 bad=False
 try:hierarchical_clustering(pairs([(0,1,float('nan'))]),['name'],fill_missing=False)
 except ValueError:bad=True
 check('nonfinite_score_raises',bad,True)
 res=hierarchical_clustering(pairs([(0,0,.9)]),['name'],fill_missing=False);check('self_edge_singleton_output',len(res),1)
 runs.append(rows)
assert len(runs[0])==9 and runs[0]==runs[1]
versions={name:getattr(module,'__version__',None) for name,module in [('numpy',np),('pandas',pd),('scipy',scipy),('networkx',networkx),('fancyimpute',fancyimpute)]}
p=R/'docs/research/dedup-500/deduplipy99-clustering-controls-20261016.json';assert not p.exists();p.write_text(json.dumps({'status':'verified_complete_clustering_modules_9_native_controls_twice','producer_sha256':h(Path(__file__)),'source_pins':{str(p.relative_to(R)):h(p) for p in [*list((S/'deduplipy/clustering').glob('*.py')),S/'deduplipy/config.py',S/'deduplipy/__init__.py']},'dependency_versions':versions,'loaded_dependency_file_pins':{str(Path(v.__file__)):h(Path(v.__file__)) for v in list(sys.modules.values()) if getattr(v,'__file__',None) and any(v.__name__==prefix or v.__name__.startswith(prefix+'.') for prefix in ['numpy','pandas','scipy','networkx','fancyimpute']) and Path(v.__file__).is_file()},'runs':runs,'scope':'Wholeunchanged clustering.py/fill_missing_edges.py/config/init modules, actualNumPy/Pandas/SciPy/NetworkX/FancyImpute SoftImpute.9synthetic scoredpair/matrix controls twice, no adapters/extraction/modelnetwork. Dependencyversions resolvedunlocked andrecorded.','findings':['Explicit score0 and absentedge both convertedNaN; realSoftImpute raisesA-C scoreabove.85, defaultclusteringmerges all3 whereasfill_missingFalse yields2groups. Cannotpreserveknownnegativepair constraints.','Centroidlinkage groupsall3 evenwhenone suppliedpairscore.49 below.5threshold. Intendedcentroidpolicy isnotall-pairthresholdguarantee.','No-missing branch returnsinputalias andkeepsasymmetry; duplicateedges overwritelastscore leadingorderdependence; nonfinitescore raises.'],'limits':['Selectedwholeclusteringmodules only; otherlibrary/modules/data/notebooks/docs/media notyetread; no whole-source countincrease.','Entitylinkage scores/imputation are not exact/image/pixel identity; no precision/recall/fullupstreamsuite/performance/hardRSS/platform/currentlibrary/adoption proof.','No realuserdata/network/externalmessages or productioncodecopy.']},indent=2)+'\n');print(json.dumps({'status':'verified','controls':18,'versions':versions}))
