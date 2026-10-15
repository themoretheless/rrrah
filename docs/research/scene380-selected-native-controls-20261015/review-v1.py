"""Execute byte-original selected function bodies; no video/image/backend adapter."""
from pathlib import Path
import ast,argparse,itertools,json,hashlib,platform
import numpy as np
from collections import Counter
from functools import reduce
from re import match
R=Path(__file__).resolve().parents[3];S=R/'docs/research/dedup-500/evidence/VieVie31--scene_detection/full-source'
ns=dict(np=np,itertools=itertools,Counter=Counter,reduce=reduce,argparse=argparse,match=match)
selected=['bollinger','get_scenes_segmentation','get_LZW_dictionnary','most_common_used_element','trim_the_firsts_e','trim_the_lasts_e','cseq','mseq','merge_intervals','compress_indexes','get_indexes','longuest_common_prefix','hamming','hamming_match','int_to_bits_indexes','get_hash_of_hashes','sliding_window']
t=ast.parse((S/'src/functions.py').read_text());nodes=[n for n in t.body if isinstance(n,ast.FunctionDef) and n.name in selected];assert len(nodes)==len(selected)
exec(compile(ast.Module(body=nodes,type_ignores=[]),'upstream-functions','exec'),ns)
t=ast.parse((S/'scd').read_text());exec(compile(ast.Module(body=[n for n in t.body if isinstance(n,ast.FunctionDef)],type_ignores=[]),'upstream-cli-parsers','exec'),ns)
rows=[]
def check(name,fn):
 fn();rows.append(name);print('PASS',name)
def equal(a,b):assert a==b,(a,b)
def raises(kind,fn):
 try:fn()
 except kind:return
 raise AssertionError('expected '+kind.__name__)
check('65536_hamming_byte_pair_oracles',lambda:[equal(ns['hamming'](a,b),(a^b).bit_count()) for a in range(256) for b in range(256)])
check('sequence_hash_loses_temporal_order',lambda:equal(ns['get_hash_of_hashes']([1,2,4]),ns['get_hash_of_hashes']([4,2,1])))
check('sequence_hash_loses_repetition_count',lambda:equal(ns['get_hash_of_hashes']([1,2]),ns['get_hash_of_hashes']([1,2]*9)))
check('longest_common_prefix_overreturns_first_string',lambda:equal(ns['longuest_common_prefix']('abc','ab'),'abc'))
check('scene_segmentation_drops_uncut_full_sequence',lambda:equal(ns['get_scenes_segmentation']([0,0,0]),[]))
check('scene_segmentation_omits_trailing_scene',lambda:equal(ns['get_scenes_segmentation']([0,10,0],0),[(0,1)]))
check('sliding_zero_step_repeats_same_window_bounded_islice',lambda:equal(list(itertools.islice(ns['sliding_window']([1,2,3],2,0),3)),[[1,2]]*3))
check('bollinger_short_signal_yields_lag_rows',lambda:equal(list(ns['bollinger'](np.array([1.,2.]),5)),[False]*5))
check('empty_mseq_index_error',lambda:raises(IndexError,lambda:ns['mseq']([])))
check('empty_merge_and_compress_index_errors',lambda:[raises(IndexError,lambda:ns[f]([],3) if f=='merge_intervals' else ns[f]([])) for f in ['merge_intervals','compress_indexes']])
check('resolution_parser_valid_input_type_error',lambda:raises(TypeError,lambda:ns['resolution_parser']('8x8')))
check('positive_interval_and_search_controls',lambda:[equal(ns['merge_intervals']([0,2,10],3),[(0,5),(10,13)]),equal(ns['get_indexes']([1,2,1,2],[1,2]),[0,2]),equal(ns['hamming_match']([1,2],[1,2]),True),equal(ns['get_hash_of_hashes']([]),0)])
print(json.dumps(dict(status='passed_selected_original_body_controls',cases=len(rows),case_names=rows,python=platform.python_version(),numpy=np.__version__,scope='AST-selected original function bodies, real NumPy; no skimage/skvideo/sklearn/FFmpeg/Docker or actual video decode/quality/pipeline execution',source_sha256={str(p.relative_to(R)):hashlib.sha256(p.read_bytes()).hexdigest() for p in S.rglob('*') if p.is_file()})))
