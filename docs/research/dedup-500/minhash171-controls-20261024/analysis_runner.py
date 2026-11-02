"""Run complete unchanged upstream analysis test module against original package."""
import hashlib
import importlib.metadata
import importlib.util
import json
from pathlib import Path
import sys
import unittest

directory=Path(__file__).resolve().parent
sys.path.insert(0,str(directory/'runtime'))
from minhash_dedup import minhash
assert minhash._numba is not None, minhash._numba_import_error
minhash._numba.set_num_threads(1)
source=directory.parent/'evidence/icodywu--minhash-dedup/05-test_minhash_analysis.py'
spec=importlib.util.spec_from_file_location('upstream_minhash171_analysis_tests',source)
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
suite=unittest.defaultTestLoader.loadTestsFromModule(module)
result=unittest.TextTestRunner(verbosity=2).run(suite)
compiled=list(map(str,minhash._hist_all_blocks_numba.signatures))
payload={'scope':__doc__,'test_source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'original_tests_run':result.testsRun,'passed':result.testsRun-len(result.failures)-len(result.errors)-len(result.skipped),'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'compiled_numba_signatures':compiled,'runtime_versions':{p:importlib.metadata.version(p) for p in ['numba','llvmlite','numpy','fsspec']},'qualification':'Actual original full package graph and original full unittest module; real Numba JIT, local signature files and memmaps; no method adapters,corpus-scale performance,S3 or scheduler'}
(directory/'original-analysis-results.json').write_text(json.dumps(payload,indent=2)+'\n')
print(json.dumps(payload,indent=2))
assert result.wasSuccessful() and result.testsRun==13 and not result.skipped and compiled
