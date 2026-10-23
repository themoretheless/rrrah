import os,sys,unittest
from pathlib import Path
base=Path(__file__).resolve().parent
sys.path[:0]=[str(base/'intra095-20261024'),str(base/'intra095-uar-dependency-20261024')]
os.chdir(base/'intra095-test-state-20261024')
suite=unittest.defaultTestLoader.loadTestsFromNames(['Tests.Test_phashArch','Tests.Test_validateTestArches'])
result=unittest.TextTestRunner(verbosity=2).run(suite)
sys.exit(not result.wasSuccessful())
