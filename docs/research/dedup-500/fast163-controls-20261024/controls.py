"""Execute pinned original pure functions and two unchanged upstream test bodies.

AST selection excludes dependency imports, CLI and plotting; no substitutes for
external algorithms are supplied. This is not a full upstream suite.
"""
import ast
import hashlib
import json
from pathlib import Path
import re
import typing
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'evidence/ayushgupta4897--fast-dedupe/02-similarity.py'
TESTS = SOURCE.with_name('05-test_similarity.py')
names = {'jaro_winkler_similarity', 'jaccard_similarity', '_create_character_ngrams', 'cosine_ngram_similarity'}
tree = ast.parse(SOURCE.read_text())
nodes = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in names]
assert len(nodes) == len(names)
scope = dict(vars(typing), re=re)
exec(compile(ast.Module(body=nodes, type_ignores=[]), str(SOURCE), 'exec'), scope)
test_tree = ast.parse(TESTS.read_text())
cls = next(n for n in test_tree.body if isinstance(n, ast.ClassDef) and n.name == 'TestSimilarityAlgorithms')
cls.body = [n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name in {'test_jaro_winkler_similarity', 'test_jaccard_similarity'}]
scope['unittest'] = unittest
exec(compile(ast.Module(body=[cls], type_ignores=[]), str(TESTS), 'exec'), scope)
result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(scope[cls.name]))
controls = [
    {'case': 'different token order and multiplicity', 'score': scope['jaccard_similarity']('alpha beta', 'beta alpha alpha'), 'expected': 100.0},
    {'case': 'unvalidated prefix weight exceeds documented score range', 'score': scope['jaro_winkler_similarity']('abcdx', 'abcdy', prefix_weight=1.0), 'expected': 140.0},
    {'case': 'default cosine case normalization is not byte identity', 'score': scope['cosine_ngram_similarity']('Document', 'document'), 'expected': 100.0},
]
for row in controls:
    assert abs(row['score'] - row['expected']) < 1e-9, row
payload = {'scope': __doc__, 'source_sha256': hashlib.sha256(SOURCE.read_bytes()).hexdigest(), 'original_test_bodies_run': result.testsRun, 'original_tests_successful': result.wasSuccessful(), 'authored_controls': controls, 'not_executed': ['full module imports', 'RapidFuzz', 'Jellyfish', 'sklearn', 'CLI', 'core dedupe pipeline', 'visualization']}
Path(__file__).with_name('results.json').write_text(json.dumps(payload, indent=2) + '\n')
assert result.wasSuccessful()
print(json.dumps(payload, indent=2))
