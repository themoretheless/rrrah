import os,sys,unittest,numpy
from pathlib import Path
base=Path(__file__).resolve().parent
sys.path.insert(0,str(base/'intra095-20261024'))
os.chdir(base/'intra095-test-state-20261024')
numpy.float=float
suite=unittest.defaultTestLoader.loadTestsFromNames(['Tests.Test_Hashing'])
result=unittest.TextTestRunner(verbosity=2).run(suite)
sys.exit(not result.wasSuccessful())
