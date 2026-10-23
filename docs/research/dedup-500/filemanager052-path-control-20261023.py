import sys,types,unittest,importlib.util
from pathlib import Path
root=Path('/Users/themoretheless/Documents/Sources/rrrah/.dedup-work/filemanager052-20261023')
pkg=types.ModuleType('FileManager');pkg.__path__=[str(root)];sys.modules['FileManager']=pkg
libs=types.ModuleType('FileManager.libs');libs.__path__=[str(root/'libs')];sys.modules['FileManager.libs']=libs
spec=importlib.util.spec_from_file_location('original_path_tests',root/'tests/test_path_helper.py');tests=importlib.util.module_from_spec(spec);spec.loader.exec_module(tests)
r=unittest.TextTestRunner(verbosity=1).run(unittest.defaultTestLoader.loadTestsFromModule(tests));assert r.wasSuccessful()
from FileManager.libs.pathhelper import computer_friendly,commonpath
assert commonpath(['/tmp/A/file','/tmp/a/other'])=='/tmp/A'
assert computer_friendly('/tmp/report:final.txt')=='t:final.txt'
print('original2unittests pass; case-sensitive commonpath merges A/a; POSIX colon filename rewritten to t:final.txt')
