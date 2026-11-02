"""Five unchanged original pipeline test bodies; entrypoint subprocess test deferred."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import unittest

directory=Path(__file__).resolve().parent
sys.path.insert(0,str(directory/'runtime'))
source=directory.parent/'evidence/icodywu--minhash-dedup/remaining-pinned/tests/test_minhash_pipeline.py'
spec=importlib.util.spec_from_file_location('original_minhash171_pipeline_tests',source)
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
excluded='test_all_stage_entrypoints_with_two_reader_ranks'
names=[name for name in unittest.defaultTestLoader.getTestCaseNames(module.TestMinhashPipeline) if name!=excluded]
suite=unittest.TestSuite(module.TestMinhashPipeline(name) for name in names)
result=unittest.TextTestRunner(verbosity=2).run(suite)
payload={'scope':__doc__,'test_source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'original_tests_run':result.testsRun,'passed':result.testsRun-len(result.failures)-len(result.errors)-len(result.skipped),'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'executed_names':names,'excluded_not_run':[excluded],'qualification':'Original complete module imported,selected original TestCase bodies against unchanged real package/NumPy/fsspec; no stage/method adapters. Source scripts/entrypoint subprocess path not executed.'}
(directory/'original-pipeline-selected-results.json').write_text(json.dumps(payload,indent=2)+'\n');print(json.dumps(payload,indent=2))
assert result.wasSuccessful() and result.testsRun==5 and not result.skipped
