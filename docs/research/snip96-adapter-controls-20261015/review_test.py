"""Original selected functions, real NumPy/Pandas/owned IO, explicit unavailable backend adapters."""
from pathlib import Path
import ast,importlib.util,io,os,sys,tempfile,types,unittest
from contextlib import redirect_stdout
from unittest.mock import patch
import numpy as np
import pandas as pd
ROOT=Path(__file__).resolve().parents[3]
SRC=ROOT/'docs/research/dedup-500/evidence/ryanwebster90--snip-dedup/full-source'
spec=importlib.util.spec_from_file_location('original_cli_helper96',SRC/'snip_dedup/_cli_helper.py');helper=importlib.util.module_from_spec(spec);spec.loader.exec_module(helper)
def load(filename,names,ns,remove_decorators=False):
 parsed=ast.parse((SRC/filename).read_text());functions=[n for n in parsed.body if isinstance(n,ast.FunctionDef) and n.name in names];assert len(functions)==len(names)
 if remove_decorators:
  for n in functions:n.decorator_list=[]
 exec(compile(ast.Module(body=functions,type_ignores=[]),filename,'exec'),ns);return ns
downloads=load('snip_dedup/snip_download.py',{'snip_download'},dict(os=os,np=np,pd=pd))
mapping=load('retrieve_dup_urls_demo.py',{'abs_ind_to_feat_file','get_cum_sz','get_emb'},dict(np=np))
indexing=load('snip_dedup/snip_index.py',{'snip_index'},dict(os=os,sys=sys,np=np,_cli_helper=helper))
class Tensor:
 def __init__(self,data):self.data=data
 def float(self):return Tensor(self.data.astype(np.float32))
 def cuda(self):return self
 def cpu(self):return self
 def numpy(self):return self.data
compress=load('snip_dedup/snip_compress.py',{'compute_feats_for_chunk'},dict(np=np,torch=types.SimpleNamespace(from_numpy=Tensor)),remove_decorators=True)
class Review(unittest.TestCase):
 def setUp(self):self.tmp=tempfile.TemporaryDirectory(prefix='rrrah-snip96-owned-');self.root=Path(self.tmp.name)
 def tearDown(self):self.tmp.cleanup()
 def download(self,mask,start=0,end=2):
  np.save(self.root/'is_dup_mlp_1024_128_gelu_snn_2layer_notext.npy',np.array(mask,dtype=bool));captured={};requested=[]
  def get(url):requested.append(url);return types.SimpleNamespace(content=b'owned fake metadata',status_code=200)
  def read(path):
   part=int(Path(path).stem.split('_')[-1]);return pd.DataFrame({'id':range(part*3,part*3+3)})
  def save(frame,path,*args,**kwargs):captured[Path(path).name]=list(frame.id);Path(path).write_text(frame.to_json())
  downloads['requests']=types.SimpleNamespace(get=get)
  with patch.object(pd,'read_parquet',side_effect=read),patch.object(pd.DataFrame,'to_parquet',new=save),redirect_stdout(io.StringIO()):downloads['snip_download'](str(self.root),start,end,False)
  self.assertEqual(len(requested),end-start);return captured
 def test_range_helper_valid_and_invalid(self):
  for start in range(8):
   for length in range(1,11):self.assertEqual(helper.validate_parts(f'{start}:{start+length}'),(start,start+length))
  for invalid in [1,'','1','2:1','-1:2','0:1:2','a:b']:
   with self.assertRaises(SystemExit):helper.validate_parts(invalid)
 def test_pattern_validation_does_not_validate_format_specifier(self):
  self.assertIsNone(helper.validate_part_format('{part:bad}.npy'))
  with self.assertRaises(ValueError):'{part:bad}.npy'.format(part=0)
  with self.assertRaises(ValueError):helper.validate_part_format('{part')
 def test_actual_npy_prefix_mapping_and_invalid_sentinels(self):
  files=[]
  for i,size in enumerate((2,3)):
   p=self.root/f'{i}.npy';np.save(p,np.arange(size*2).reshape(size,2));files.append(str(p))
  prefix=mapping['get_cum_sz'](files);np.testing.assert_array_equal(prefix,[0,2,5])
  for absolute,part,local in [(0,0,0),(1,0,1),(2,1,0),(4,1,2)]:
   ff,li,lci=mapping['abs_ind_to_feat_file'](absolute,prefix,files);self.assertEqual((ff,int(li),int(lci)),(files[part],part,local))
  with self.assertRaises(IndexError):mapping['abs_ind_to_feat_file'](-1,prefix,files)
  with self.assertRaises(IndexError):mapping['abs_ind_to_feat_file'](5,prefix,files)
 def test_filtered_length_misaligns_next_shard_mask(self):
  mask=[False,True,False,False,True,True];out=self.download(mask)
  self.assertEqual(out['metadata_0000.parquet'],[0,2]);self.assertEqual(out['metadata_0001.parquet'],[3,4]);self.assertTrue(mask[4])
  expected=[i for i in range(3,6) if not mask[i]];self.assertEqual(expected,[3]);self.assertNotEqual(out['metadata_0001.parquet'],expected)
 def test_nonzero_start_uses_mask_origin_zero(self):
  mask=[False,True,False,False,True,True];out=self.download(mask,start=1,end=2);self.assertEqual(out['metadata_0001.parquet'],[3,5]);self.assertTrue(mask[5])
 def test_first_row_force_keeps_a_flagged_duplicate(self):
  mask=[True,True,True];out=self.download(mask,end=1);self.assertEqual(out['metadata_0000.parquet'],[0]);self.assertTrue(mask[0])
 def test_http_error_body_overwrites_metadata_before_parse_error(self):
  np.save(self.root/'is_dup_mlp_1024_128_gelu_snn_2layer_notext.npy',np.array([False]*3));metadata=self.root/'metadata';metadata.mkdir();path=metadata/'metadata_0000.parquet';path.write_bytes(b'previous owned metadata')
  downloads['requests']=types.SimpleNamespace(get=lambda url:types.SimpleNamespace(content=b'owned HTTP500 error body',status_code=500))
  with patch.object(pd,'read_parquet',side_effect=ValueError('owned invalid parquet')),redirect_stdout(io.StringIO()):
   with self.assertRaises(ValueError):downloads['snip_download'](str(self.root),0,1,False)
  self.assertEqual(path.read_bytes(),b'owned HTTP500 error body')
 def test_empty_or_invalid_batch_size_is_not_handled(self):
  def never(_):raise AssertionError('no backend inference expected')
  for chunk,batch in [(np.empty((0,3)),256),(np.ones((2,3)),0),(np.ones((2,3)),-1)]:
   with self.assertRaises(ValueError):compress['compute_feats_for_chunk'](never,chunk,batch)
 def test_zero_shard_size_fails_before_gpu_allocation(self):
  base=self.root/'base.index';base.write_bytes(b'owned fake base index')
  indexing['faiss']=types.SimpleNamespace(StandardGpuResources=lambda:(_ for _ in ()).throw(AssertionError('no GPU allocation expected')))
  with self.assertRaises(ZeroDivisionError):indexing['snip_index'](snip_base_index_path=str(base),shard_size=0)
 def test_valid_batch_order_with_explicit_tensor_adapter(self):
  data=np.arange(15,dtype=np.float64).reshape(5,3);sizes=[]
  def net(tensor):sizes.append(len(tensor.data));return Tensor(tensor.data[:,:2]*2)
  result=compress['compute_feats_for_chunk'](net,data,2);np.testing.assert_array_equal(result,data.astype(np.float32)[:,:2]*2);self.assertEqual(sizes,[2,2,1])
if __name__=='__main__':unittest.main(verbosity=2)
