"""Complete unchanged original partition unittest module, original package graph."""
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import unittest

directory=Path(__file__).resolve().parent
sys.path.insert(0,str(directory/'runtime'))
source=directory.parent/'evidence/icodywu--minhash-dedup/remaining-pinned/tests/test_partition.py'
spec=importlib.util.spec_from_file_location('original_minhash171_partition_tests',source)
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
result=unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(module))
payload={'scope':__doc__,'test_source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'original_tests_run':result.testsRun,'passed':result.testsRun-len(result.failures)-len(result.errors)-len(result.skipped),'failures':len(result.failures),'errors':len(result.errors),'skipped':len(result.skipped),'qualification':'Original full test module and partition/io/jsonl/runtime graph; actual temporary files and ThreadPoolExecutor, injected publication failure as upstream test specifies. No method adapters or Slurm/S3/corpus execution.'}
(directory/'original-partition-results.json').write_text(json.dumps(payload,indent=2)+'\n');print(json.dumps(payload,indent=2))
assert result.wasSuccessful() and not result.skipped
