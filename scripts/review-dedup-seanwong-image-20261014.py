#!/usr/bin/env python3
from pathlib import Path
import ast,json,hashlib,contextlib,io,warnings
from concurrent.futures import ThreadPoolExecutor
import numpy as np
R=Path(__file__).resolve().parents[1];D=R/'docs/research/dedup-500';P=D/'seanwong-image-deduplicator-full-source-20261014.json';p=json.loads(P.read_text());S=Path(p['snapshot']);sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest();assert all(sha(S/k)==v for k,v in p['file_hashes'].items())
tree=ast.parse((S/'deduplicator.py').read_text());node=next(n for n in tree.body if isinstance(n,ast.ClassDef));namespace={'np':np,'tqdm':lambda value,*a,**kw:value,'ThreadPoolExecutor':ThreadPoolExecutor};exec(compile(ast.Module(body=[node],type_ignores=[]),str(S/'deduplicator.py'),'exec'),namespace);cls=namespace['ImageDeduplicator'];obj=cls.__new__(cls);obj.num_threads=2
rows=[]
def check(name,call,expected=None,error=None):
 with warnings.catch_warnings(record=True) as caught, contextlib.redirect_stdout(io.StringIO()) as out:
  warnings.simplefilter('always')
  try:
   result=call();assert error is None,(name,'expected error');assert result==expected,(name,result,expected);e=None
  except Exception as ex:
   if error is None or not isinstance(ex,error):raise
   result=None;e=type(ex).__name__
 rows.append(dict(control=name,result=result,error=e,warnings=[str(w.message) for w in caught],stdout=out.getvalue()))
check('identical_and_orthogonal_vectors',lambda:obj.deduplicate_by_similarity(np.array([[1.,0.],[1.,0.],[0.,1.]])),[0,2])
check('zero_vectors_retained_with_nan_normalization',lambda:obj.deduplicate_by_similarity(np.zeros((2,2))),[0,1])
check('integer_dtype_rejected_during_inplace_normalization',lambda:obj.deduplicate_by_similarity(np.array([[1,0],[1,0]])),error=TypeError)
x=np.array([[3.,4.],[4.,3.]])
check('caller_matrix_mutated',lambda:obj.deduplicate_by_similarity(x),[0]);assert np.allclose(x,[[.6,.8],[.8,.6]])
check('zero_batch_size_raises',lambda:obj.deduplicate_by_similarity(np.array([[1.,0.],[1.,0.]]),batch_size=0),error=ValueError)
check('negative_batch_size_silently_keeps_identical_vectors',lambda:obj.deduplicate_by_similarity(np.array([[1.,0.],[1.,0.]]),batch_size=-1),[0,1])
check('nan_threshold_silently_keeps_identical_vectors',lambda:obj.deduplicate_by_similarity(np.array([[1.,0.],[1.,0.]]),threshold=float('nan')),[0,1])
angles=np.deg2rad([0.,20.,40.]);chain=np.column_stack((np.cos(angles),np.sin(angles)));threshold=float(np.cos(np.deg2rad(25.)))
check('nontransitive_chain_order_A_B_C',lambda:obj.deduplicate_by_similarity(chain.copy(),threshold=threshold),[0,2])
check('nontransitive_chain_order_B_A_C',lambda:obj.deduplicate_by_similarity(chain[[1,0,2]].copy(),threshold=threshold),[0])
check('negative_threshold_excludes_orthogonal_vectors',lambda:obj.deduplicate_by_similarity(np.array([[1.,0.],[0.,1.]]),threshold=-1.),[0])
check('threshold_one_keeps_identical_vectors_strict_comparison',lambda:obj.deduplicate_by_similarity(np.array([[1.,0.],[1.,0.]]),threshold=1.),[0,1])
x=np.array([[1.,0.],[1.,0.]]);x.flags.writeable=False
check('readonly_input_rejected',lambda:obj.deduplicate_by_similarity(x),error=ValueError)
obj._extract_feature=lambda path: {'one':[1.,0.],'broken':None,'two':[1.,0.]}[path]
check('failed_feature_omitted_from_returned_paths',lambda:obj.deduplicate(['one','broken','two']),['one'])
obj._extract_feature=lambda path:None
check('all_failed_features_return_empty_success',lambda:obj.deduplicate(['broken']),[])
assert all(sha(S/k)==v for k,v in p['file_hashes'].items())
O=D/'seanwong-image-deduplicator-full-review-20261014.json';assert not O.exists();O.write_text(json.dumps(dict(status='complete_five_text_file_review_with14_original_method_controls',repository=p['repository'],registry_index=160,commit=p['commit'],fully_read=list(p['file_hashes']),pins={str(f):sha(f) for f in [P,Path(__file__),*[S/k for k in p['file_hashes']]]},numpy_version=np.__version__,controls=rows,decision='reference_no_production_code_adoption',findings=['Batched all-pairs cosine remains quadratic in unrelated case; all decoded features and executor results retained, no managed memory/admission/cancellation.','Greedy nontransitive threshold exclusion depends on input order. Semantic ResNet features are candidate signals, no byte/pixel/geometric identity confirmation.','Feature failures omitted from success path lists; all failures yield empty success. No source mutation/dependency/model generation guards.','Input feature matrix normalized in place; zero norms, NaN threshold and negative batch sizes unchecked. Integer/readonly inputs fail.','Source README references requirements.txt absent from complete5filetree. ActualMITtext read, copyright retains placeholder; pretrained model/dependency licenses separate.'],scope='All5stored pinned-tree text files fully read. Original class AST compiled without import statements; real NumPy and ThreadPoolExecutor, identity progress wrapper, constructor skipped. Fourteen grouping/error/input controls; feature extraction injected only for two None/path controls. Not original complete CLI/import, torchvision/ResNet weights/inference, image preprocessing, real corrupt-file handling, CUDA, platform/resource/concurrency or image-quality qualification. No upstream test files in tree.'),indent=2)+'\n');print(len(rows),'controls passed')
