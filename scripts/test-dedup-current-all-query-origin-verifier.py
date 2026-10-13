"""Adversarial checks against false origin labels, lost failures and fabricated support."""
import copy,importlib.util,json,unittest
from pathlib import Path
spec=importlib.util.spec_from_file_location('verifier',Path(__file__).with_name('verify-dedup-current-all-query-origins-20261011.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class Verifier(unittest.TestCase):
 def fixture(self):
  original={'filename':'original.jpg','group_id':1};query={'filename':'query.jpg','group_id':2}
  row={'index':0,'original':'original.jpg','query':'query.jpg','original_group':1,'query_group':2,'label':'different_publisher_origin','elapsed_seconds':1.,'source':'/a/original.jpg','target':'/b/query.jpg','returncode':0,'status':'different_origin_supported_review','supported':True,'terminal':'native','managed_peak':100}
  pixels={'supported':True,'anchors':[10,10],'directions':[{'sites':10,'valid_sites':10,'informative_pairs':1000,'agreeing_pairs':900}]*2}
  evidence={'status':'ok','files':[0,1],'source_paths':[row['source'],row['target']],'managed_used':0,'managed_peak':100,'pairs':[{'left':0,'right':1,'evidence':{'supported':True,'terminal':'native','native_recipes':1,'pixels':pixels}}]}
  return row,original,query,evidence
 def check(self,row,original,query,evidence):module.verify_row(row,0,original,query,{'stdout':json.dumps(evidence),'stderr':''})
 def test_different_origin_support_remains_review(self):self.check(*self.fixture())
 def test_execution_failure_remains_failure(self):
  row,o,q,e=self.fixture();row.update(returncode=1,status='execution_failed');row.pop('supported');module.verify_row(row,0,o,q,{'stdout':'','stderr':'provider failed'})
  row['status']='different_origin_rejected'
  with self.assertRaises(AssertionError):module.verify_row(row,0,o,q,{'stdout':'','stderr':'provider failed'})
 def test_corrupt_schedule_labels_and_status(self):
  for key,value in [('index',1),('original','other.jpg'),('query_group',1),('label','publisher_origin_copy'),('status','different_origin_rejected'),('supported',False),('elapsed_seconds',float('nan')),('source','/a/other.jpg')]:
   with self.subTest(key=key):
    row,o,q,e=self.fixture();row[key]=value
    with self.assertRaises(AssertionError):self.check(row,o,q,e)
 def test_fabricated_or_underqualified_support(self):
  for corrupt in [lambda e:e.update(managed_used=1),lambda e:e.update(source_paths=['/wrong','/wrong']),lambda e:e['pairs'].append(copy.deepcopy(e['pairs'][0])),lambda e:e['pairs'][0]['evidence']['pixels'].update(supported=False),lambda e:e['pairs'][0]['evidence']['pixels'].update(anchors=[9,10]),lambda e:e['pairs'][0]['evidence']['pixels']['directions'][0].update(agreeing_pairs=899),lambda e:e['pairs'][0]['evidence'].update(native_recipes=7)]:
   row,o,q,e=self.fixture();corrupt(e)
   with self.assertRaises(AssertionError):self.check(row,o,q,e)
if __name__=='__main__':unittest.main()
