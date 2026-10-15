"""Original selected whole classes/functions; real KD implementations, explicit API shim only."""
from pathlib import Path
import ast,contextlib,hashlib,io,json,math,os,shutil,sys,tempfile
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/near41-20261015';S=W/'source/src/deduplication'
sys.path.insert(0,str(W/'deps'))
import numpy as np
import pandas as pd
import scipy,sklearn,imagehash
from scipy.spatial import cKDTree
from sklearn.neighbors import KDTree
from PIL import Image
from tqdm import tqdm
from natsort import natsorted

ns={'np':np,'pd':pd,'os':os,'shutil':shutil,'Image':Image,'tqdm':tqdm,'natsorted':natsorted,'KDTree':KDTree,'cKDTree':cKDTree}
pins={}
def select(path,names):
 p=S/path;b=p.read_bytes();pins[str(path)]=hashlib.sha256(b).hexdigest();tree=ast.parse(b)
 nodes=[n for n in tree.body if isinstance(n,(ast.ClassDef,ast.FunctionDef)) and n.name in names];assert len(nodes)==len(names)
 exec(compile(ast.Module(body=nodes,type_ignores=[]),str(p),'exec'),ns)
select(Path('duplicatefinder/NearDuplicateImageFinder.py'),['NearDuplicateImageFinder'])
select(Path('duplicatefinder/KDTreeFinder.py'),['KDTreeFinder'])
select(Path('duplicatefinder/cKDTreeFinder.py'),['cKDTreeFinder'])
select(Path('dataset/ImageToHash.py'),['ImageToHash'])
select(Path('utils/FileSystem.py'),['FileSystem'])
select(Path('commands/helpers.py'),['backup_images','delete_images','save_results'])
ns['hash_algo_dict']={'dhash':imagehash.dhash,'phash':imagehash.phash,'average_hash':imagehash.average_hash,'whash':imagehash.whash}
ns['image_extensions']=ast.literal_eval(next(n.value for n in ast.parse((S/'dataset/ImageToHash.py').read_text()).body if isinstance(n,ast.Assign) and any(isinstance(t,ast.Name) and t.id=='image_extensions' for t in n.targets)))
import time,multiprocessing
ns.update(time=time,multiprocessing=multiprocessing)
def dataset(values):
 rows=[]
 for i,v in enumerate(values):
  digits=list(v if isinstance(v,str) else format(v,'016x'))
  rows.append(dict(file=f'image-{i}',short_file=f'image-{i}',hash=''.join(digits),hash_list=digits,**{str(k):int(d,16) for k,d in enumerate(digits)}))
 return pd.DataFrame(rows)
def finder(values,metric='manhattan'):return ns['KDTreeFinder'](dataset(values),metric)
cases=[];observed={};root=Path(tempfile.mkdtemp(prefix='controls-',dir=W))
def passed(name):cases.append(name);print('PASS '+name)

f=finder(range(16));fp=fn=0;checked=0
for i in range(16):
 distances,indices=f._find(i,16,1)
 assert all(d==abs(i-int(j)) for d,j in zip(distances[0],indices[0]))
 _,selected=f.find_near_duplicates(i,16,1)
 expected={j for j in range(16) if j!=i and (i^j).bit_count()<=1};selected=set(selected)
 fp+=len(selected-expected);fn+=len(expected-selected);checked+=16
assert checked==256 and fp>0 and fn>0
observed['nibble_numeric_vs_bit_hamming']={'pairs':checked,'false_positive_directed_edges':fp,'missing_directed_edges':fn,'threshold':1,'comparison_scope':'Hamming oracle versus documented numeric nibble Manhattan metric, not visual groundtruth'}
passed('real_KDTree_all256_numeric_distances_and_Hamming_policy_difference')

f=finder([0,1,2]);keep,remove,_=f.find_all_near_duplicates(3,1)
assert keep==['image-0'] and set(remove)=={'image-1','image-2'} and abs(0-2)>1
passed('greedy_chain_removes_vertex_without_retained_neighbor_within_threshold')

f=finder(range(6));_,neighbors=f.find_near_duplicates(0,2,100);assert neighbors==[1]
observed['top_k_radius']={'radius_neighbors':5,'returned':len(neighbors),'k':2}
passed('top_k_cap_omits_other_radius_matches')

f=finder([0,0,0,0]);keep,remove,_=f.find_all_near_duplicates(2,0);assert len(set(keep+remove))==2
observed['equal_ties']={'rows':4,'represented':len(set(keep+remove)),'keep':keep,'remove':remove}
passed('equal_hash_ties_first_neighbor_anchor_omits_two_rows')

try:finder([0,1])._find(0,3,100)
except ValueError:pass
else:raise AssertionError('k>N must fail in sklearn original')
passed('nearest_neighbor_count_above_population_raises')

f=ns['cKDTreeFinder'](dataset([0,1]),'manhattan')
try:f._find(0,2,100)
except TypeError as e:assert 'n_jobs' in str(e);observed['modern_scipy_error']=str(e)
else:raise AssertionError('modern SciPy n_jobs must be rejected')
passed('unchanged_cKDTree_modern_SciPy_rejects_n_jobs')

class KeywordCompatibilityTree:
 def __init__(self,*a,**kw):self.tree=cKDTree(*a,**kw)
 def query(self,*a,**kw):kw['workers']=kw.pop('n_jobs');return self.tree.query(*a,**kw)
ns['cKDTree']=KeywordCompatibilityTree
f=ns['cKDTreeFinder'](dataset(['00','34']),'euclidean');d1,_=f._find(0,2,100);da,_=f._find_all(2,100)
assert d1[0,1]==7 and da[0,1]==5
passed('keyword_only_shim_exposes_single_vs_all_euclidean_metric_mismatch')
try:f.find_near_duplicates(0,1,100)
except (IndexError,TypeError):pass
else:raise AssertionError('k1 squeezed result shape should fail original wrapper')
passed('keyword_only_shim_k1_squeezed_shape_breaks_wrapper')

black=root/'black.png';white=root/'white.png';Image.new('RGB',(32,32),'black').save(black);Image.new('RGB',(32,32),'white').save(white)
assert ns['ImageToHash'].img_hash(str(black),hash_algo='dhash')==ns['ImageToHash'].img_hash(str(white),hash_algo='dhash')
assert Image.open(black).tobytes()!=Image.open(white).tobytes()
passed('real_dhash_equal_for_different_flat_pixels_is_not_identity')

obj=ns['ImageToHash'](str(root),hash_algo='dhash')
try:obj.build_dataset(parallel=False)
except AttributeError as e:assert 'append' in str(e);observed['modern_pandas_error']=str(e)
else:raise AssertionError('old DataFrame.append API unexpectedly available')
passed('unchanged_sequential_dataset_builder_fails_modern_pandas_append')

f=finder([0,15]);keep,remove,_=f.find_all_near_duplicates(2,0);assert keep==remove==[]
passed('unique_rows_absent_from_both_keep_and_remove_duplicate_lists')

victim=root/'owned-victim.png';Image.new('RGB',(2,2),'red').save(victim);out=root/'owned-output';out.mkdir()
original_copy=shutil.copy
def fail_copy(*a,**kw):raise IOError('owned injected backup copy failure')
shutil.copy=fail_copy
try:ns['save_results']([], [str(victim)],8,1,str(out),backup_keep=False,backup_duplicate=True,safe_deletion=False)
finally:shutil.copy=original_copy
assert not victim.exists() and list(out.rglob('*.png'))==[]
passed('explicit_backup_IO_failure_is_swallowed_then_owned_original_deleted')

print(json.dumps({'status':'passed','controls':len(cases),'cases':cases,'observations':observed,'source_pins':pins,'versions':{'python':sys.version,'numpy':np.__version__,'pandas':pd.__version__,'scipy':scipy.__version__,'sklearn':sklearn.__version__},'fixture_root':str(root),'adapters':['Original selected whole classes/functions; top-level bootstrap/imports bypassed','Two cKDTree compatibility controls rename n_jobs to workers only; actual native SciPy metric/query unchanged','One explicit shutil.copy IOError, all other helper fileIO actualownedfixtures'],'limits':['Modern dependencies differ from original2019requirements; incompatibilities reported rather than upstreamsuite pass','Synthetic hash vectors compare radius/metric/graph contracts, not calibrated visual quality','No multiprocessing/CLI/upstreamtestdataset campaign/bootstrap/fullrepository review or performance claim','No production adoption']},sort_keys=True))
