"""Qualify original Python brute-force search with real NumPy distance body."""
from pathlib import Path
from types import SimpleNamespace
import ast
import hashlib
import importlib.util
import json
import subprocess
import numpy as np

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-bruteforce-20261019'
H = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()


def main():
    W.mkdir(exist_ok=False)
    names = ['imagededup/handlers/search/brute_force.py', 'imagededup/methods/hashing.py', 'tests/test_brute_force.py']
    pins = {n: H(S / n) for n in names}
    for n in names:
        assert subprocess.check_output(['git', '-C', str(S), 'show', 'HEAD:' + n]) == (S / n).read_bytes()
    spec = importlib.util.spec_from_file_location('original_bruteforce', S / names[0])
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    parsed = ast.parse((S / names[1]).read_text())
    cls = next(n for n in parsed.body if isinstance(n, ast.ClassDef) and n.name == 'Hashing')
    method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == 'hamming_distance')
    method.decorator_list = []
    env = {'np': np}
    exec(compile(ast.fix_missing_locations(ast.Module(body=[method], type_ignores=[])), names[1], 'exec'), env)
    distance = env['hamming_distance']
    tests = ast.parse((S / names[2]).read_text())
    tests.body = [n for n in tests.body if not (isinstance(n, ast.ImportFrom) and n.module.startswith('imagededup'))]
    testenv = {'BruteForce': module.BruteForce, 'Hashing': SimpleNamespace(hamming_distance=distance)}
    exec(compile(tests, names[2], 'exec'), testenv)
    passed = sorted(n for n in testenv if n.startswith('test_'))
    for n in passed:
        testenv[n]()
    database = {f'file-{i}': f'{i:016x}' for i in range(256)}
    search = module.BruteForce(database, distance)
    checks = 0
    for query in range(256):
        for radius in range(9):
            actual = search.search(f'{query:016x}', radius)
            expected = [(f'file-{i}', (query ^ i).bit_count()) for i in range(256) if (query ^ i).bit_count() <= radius]
            assert actual == expected
            checks += 1
    calls = []
    instrumented = module.BruteForce(database, lambda a, b: calls.append((a, b)) or distance(a, b))
    result = instrumented.search('0000000000000000', 64)
    assert len(result) == 256 and len(calls) == 512
    assert module.BruteForce({}, distance).search('0', 0) == []
    assert search.search('0', -1) == []
    assert all(H(S / n) == pin for n, pin in pins.items())
    report = {'status': 'passed_selected_original_Python_bruteforce_with_real_numpy', 'source_pins': pins,
              'numpy_version': np.__version__, 'original_tests_passed': passed, 'oracle_queries': checks,
              'oracle_domain': 'all256eightbit patterns encoded64bit,all radii0..8,exactorderedlist compared against independent intXOR.bit_count fullscan',
              'all_match_256_candidates_distance_calls': len(calls), 'empty_and_negative_radius_controls': 'passed',
              'limits': ['Original entire BruteForce module loaded unchanged;2 original test bodies unchanged, only package imports routed explicitly.',
                         'Original Hashing.hamming_distance body extracted by AST with staticmethod decorator removed; real NumPy2.5.4, no sum adapter.',
                         'Not entire Hashing class, image decode/hash generation, Cython/native backend, package import or full upstream suite.',
                         '512calls is operation-count evidence, not measured speedup; no production code adopted.']}
    (W / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'tests': len(passed), 'oracle_queries': checks, 'all_match_distance_calls': len(calls)}))


if __name__ == '__main__':
    main()
