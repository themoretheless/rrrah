import importlib.util,sys,unittest
from pathlib import Path
p=Path(__file__).resolve().parent/'intra095-20261024/wat/Test_ImLoad.py'
spec=importlib.util.spec_from_file_location('original_wat_image_test',p);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(module))
sys.exit(not result.wasSuccessful())
