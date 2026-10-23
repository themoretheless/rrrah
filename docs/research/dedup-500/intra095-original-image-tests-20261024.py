import os,sys,unittest
from pathlib import Path
base=Path(__file__).resolve().parent;sys.path.insert(0,str(base/'intra095-20261024'));os.chdir(base/'intra095-test-state-20261024')
suite=unittest.defaultTestLoader.loadTestsFromNames(['Tests.Test_ImLoad','Tests.Test_Pillow_Resizing'])
result=unittest.TextTestRunner(verbosity=2).run(suite)
sys.exit(not result.wasSuccessful())
