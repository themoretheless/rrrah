"""Whole unchanged difPy module, exact declared dependencies, owned fixtures and real Pool workers."""
from pathlib import Path
from types import SimpleNamespace as NS
import contextlib,hashlib,io,json,math,os,sys,tempfile,warnings
R=Path(__file__).resolve().parents[1];W=R/'.dedup-work/dif14-20261016';S=W/'source';sys.path[:0]=[str(W/'deps'),str(S)]
import numpy as np
from PIL import Image
import difPy
from difPy.dif import _compare_imgs as Compare,_validate_param as Validate
h=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def main():
 assert np.__version__=='1.26.4' and Image.__version__=='10.2.0';observations=[]
 def record(name,**d):observations.append({'control':name,**d})
 def obj(tensors,threshold=0,rotate=False):
  o=difPy.search.__new__(difPy.search);o._search__difpy_obj=NS(_tensor_dictionary=tensors,_id_to_shape_dictionary={k:v.shape for k,v in tensors.items()});o._search__similarity=threshold;o._search__rotate=rotate;o._search__same_dim=True;return o
 a=np.zeros((10,10,3),dtype=np.uint8);b=np.full_like(a,16);actual=Compare._compute_mse(a,b,rotate=False);independent=float(((a.astype(np.int64)-b.astype(np.int64))**2).mean());assert actual==0 and independent==256;record('uint8_mse_overflow_zero_for_different_pixels',native_mse=float(actual),independent_mse=independent)
 white=np.full_like(a,255);assert Compare._compute_mse(a,white,rotate=False)==1;record('black_white_mse_wraps',native_mse=1,independent_mse=65025)
 t=np.arange(27,dtype=np.float64).reshape(3,3,3);rot=np.rot90(t);forward=Compare._compute_mse(t,rot,rotate=True);reverse=Compare._compute_mse(rot,t,rotate=True);assert forward>0 and reverse==0;record('one_of_four_rotations_missing_order_asymmetry',forward_mse=float(forward),reverse_mse=float(reverse),fixture='explicit float tensor to isolate rotation from uint8 overflow')
 x=a.copy();y=a.copy();x[0,0,:]=255;y[0,1,:]=255;o=obj({0:x,1:y});assert not o._find_matches((0,1));assert o._find_matches_batch([(0,1)])==[(0,1,0)];record('batch_equal_sums_false_exact_match',scalar_match=False,batch_match=True)
 o=obj({0:a,1:a,2:a});assert list(o._yield_comparison_group())==[[(0,1)]];record('batch_generator_omits_maximum_id',ids=[0,1,2],emitted_pairs=[[0,1]],independent_expected_pairs=3)
 o=obj({0:a,2:a,4:a});assert list(o._yield_comparison_group())==[[(0,2)]];record('sparse_ids_still_omit_maximum',ids=[0,2,4],emitted_pairs=[[0,2]])
 o=obj({0:a,1:np.full_like(a,10),2:np.full_like(a,1)},threshold=2);assert o._find_matches_batch([(0,1),(0,2)])==[(0,2,100.0)];record('batch_reports_wrong_candidate_mse',accepted_id=2,reported_mse=100,independent_mse=1)
 assert math.isnan(Validate._similarity(float('nan'))) and math.isinf(Validate._similarity(float('inf')));record('nonfinite_similarity_accepted',nan=True,infinity=True)
 o=difPy.search.__new__(difPy.search);o.lower_quality=[]
 try:o.delete(silent_del=True)
 except UnboundLocalError:record('empty_delete_uninitialized_counter',exception='UnboundLocalError')
 else:raise AssertionError('expected empty deletion error')
 with tempfile.TemporaryDirectory(prefix='dif14-owned-') as tmp:
  root=Path(tmp);images=root/'images';images.mkdir();paths=[]
  for label,color in [('a',0),('b',16)]:
   p=images/(label+'.png');Image.new('RGB',(10,10),(color,)*3).save(p);paths.append(p)
  with contextlib.redirect_stdout(io.StringIO()):
   built=difPy.build(str(images),px_size=10,show_progress=False,processes=1);found=difPy.search(built,similarity='duplicates',rotate=False,show_progress=False,processes=1)
  assert len(built._tensor_dictionary)==2 and found.stats['process']['search']['matches_found']['duplicates']==1;record('actual_full_pool_pipeline_labels_black_and_gray_duplicates',build_images=2,duplicate_count=1,distinct_decoded_images=True,worker_processes=1)
  for i,p in enumerate(paths):Image.new('RGB',(10,10),'red').save(p)
  with contextlib.redirect_stdout(io.StringIO()):
   built=difPy.build(str(images),px_size=10,show_progress=False,processes=1);found=difPy.search(built,rotate=False,show_progress=False,processes=1)
  assert len(found.lower_quality)==1;victim=Path(found.lower_quality[0]);victim.write_bytes(b'unique replacement after match');found._delete_files();assert not victim.exists();record('full_pipeline_post_match_rewrite_deleted',removed_unique_replacement=True)
  opaque=root/'opaque.png';hidden=root/'transparent.png';Image.new('RGBA',(10,10),(20,30,40,255)).save(opaque);Image.new('RGBA',(10,10),(20,30,40,0)).save(hidden);bo=difPy.build.__new__(difPy.build);bo._build__px_size=10;to=bo._generate_tensor(0,str(opaque));th=bo._generate_tensor(1,str(hidden));assert np.array_equal(to[1],th[1]);record('actual_png_alpha_discarded',different_alpha=True,tensors_equal=True)
  g1=root/'one.gif';g2=root/'two.gif';Image.new('RGB',(10,10),'red').save(g1,save_all=True,append_images=[Image.new('RGB',(10,10),'black')],duration=100);Image.new('RGB',(10,10),'red').save(g2,save_all=True,append_images=[Image.new('RGB',(10,10),'white')],duration=100);assert np.array_equal(bo._generate_tensor(0,str(g1))[1],bo._generate_tensor(1,str(g2))[1]);record('actual_gif_later_frames_ignored',changed_second_frame=True,tensors_equal=True)
  wide=root/'wide.png';square=root/'square.png';Image.new('RGB',(100,2),'red').save(wide);Image.new('RGB',(20,20),'red').save(square);assert Compare._sort_imgs_by_size([str(square),str(wide)])[0]==str(wide);record('quality_sort_uses_dimension_sum_not_pixel_area',first_pixels=200,second_pixels=400)
  target=root/'moves';target.mkdir();source=root/'victim.png';source.write_bytes(b'new');(target/source.name).write_bytes(b'unrelated existing destination');o=difPy.search.__new__(difPy.search);o.lower_quality=[str(source)]
  with contextlib.redirect_stdout(io.StringIO()):o.move_to(str(target))
  assert (target/source.name).read_bytes()==b'new' and not source.exists();record('move_overwrites_existing_destination',old_destination_lost=True)
  left=root/'left';right=root/'right';left.mkdir();right.mkdir();p1=left/'same.png';p2=right/'same.png';p1.write_bytes(b'first');p2.write_bytes(b'second');o.lower_quality=[str(p1),str(p2)]
  with contextlib.redirect_stdout(io.StringIO()):o.move_to(str(target))
  assert (target/'same.png').read_bytes()==b'second' and not p1.exists() and not p2.exists();record('move_same_basename_collides_and_loses_first',input_files=2,retained_distinct_contents=1)
 sourcepins={str(p.relative_to(S)):h(p) for p in [S/'difPy/dif.py',S/'difPy/__init__.py',S/'difPy/version.py',S/'requirements.txt']};print(json.dumps({'status':'passed_actual_native_difpy_controls','observations':observations,'source_pins':sourcepins,'dependencies':{'numpy':np.__version__,'Pillow':Image.__version__},'python':sys.version,'scope':'Whole unchanged upstream difPy module, real declaredNumPy1.26.4/Pillow10.2.0 andrealmultiprocessingPool1 for2small fullbuild/search scenarios. Other controls directlyinvokeunchanged methods withdeclaredtensor/privatefixtures, actualownedPNG/GIF/rename/delete only. No originalupstreamtests tracked, no full5001imagepipeline/scale/RSS/platform/visualquality/performance proof.'},sort_keys=True))
if __name__=='__main__':main()
