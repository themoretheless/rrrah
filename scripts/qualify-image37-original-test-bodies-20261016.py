"""All11 original unittest bodies, explicit fixture setup replacing database construction."""
from pathlib import Path
import ast,contextlib,datetime,io,json,random,runpy,unittest
R=Path(__file__).resolve().parents[1];S=R/'.dedup-work/image37-20261016/source'
with contextlib.redirect_stdout(io.StringIO()):base=runpy.run_path(str(R/'scripts/qualify-image37-selected-controls-20261016.py'))
C=base['C'];NS=base['NS'];config=base['config'];names=[]
for line in (S/'tests/py_image_dedup.yaml').read_text().splitlines():
 if '- name:' in line:names.append({'name':line.split('"')[1]})
config.PRIORITIZATION_RULES.value=names
class TestBase(unittest.TestCase):
 def setUp(self):
  self.config=NS(MAX_FILE_MODIFICATION_TIME_DELTA=NS(value=datetime.timedelta(seconds=100)))
  self.under_test=C.__new__(C);self.under_test._config=self.config;self.under_test._processed_files={}
ns={'TestBase':TestBase,'random':random,'shuffle':random.shuffle,'uniform':random.uniform,'MetadataKey':base['ns']['MetadataKey'],'Path':Path,'file_has_extension':base['file_ns']['file_has_extension']}
classes=[]
for name in ['test_select_images_to_delete.py','test_file_extension.py']:
 tree=ast.parse((S/'tests'/name).read_text());cl=[x for x in tree.body if isinstance(x,ast.ClassDef)];classes.extend(x.name for x in cl);exec(compile(ast.Module(body=cl,type_ignores=[]),name,'exec'),ns)
suite=unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromTestCase(ns[n]) for n in classes);assert suite.countTestCases()==11;random.seed(137);result=unittest.TextTestRunner(verbosity=2).run(suite);assert result.wasSuccessful() and result.testsRun==11 and not result.skipped
print(json.dumps({'tests':11,'passed':11,'scope':'Every original test method and helper body unchanged, explicit fixture-only TestBase setup with original YAML prioritization rule names/100-second delta. Selected unchanged production sorting/filter bodies; constructor/real Elasticsearch/config framework/image matcher/fullpackage not executed.'}))
