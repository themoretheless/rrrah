"""Compile unchanged generated C++ and run original wrapper against independent bit-count oracle."""
from pathlib import Path
from types import SimpleNamespace
import ast
import hashlib
import importlib.util
import json
import subprocess
import sys
import sysconfig
import numpy as np

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-native-search-20261019'
W.mkdir(exist_ok=False)
cpp = S / 'imagededup/handlers/search/brute_force_cython_ext.cpp'
header = S / 'imagededup/handlers/search/builtin/builtin.h'
binary = W / ('brute_force_cython_ext' + sysconfig.get_config_var('EXT_SUFFIX'))
command = ['clang++', '-shared', '-fPIC', '-O2', '-std=c++11', '-undefined', 'dynamic_lookup',
           '-I' + sysconfig.get_path('include'), '-I' + str(cpp.parent), str(cpp), '-o', str(binary)]
build = subprocess.run(command, capture_output=True, text=True)
(W / 'build.stdout.log').write_text(build.stdout)
(W / 'build.stderr.log').write_text(build.stderr)
(W / 'build-command.json').write_text(json.dumps(command, indent=2)+'\n')
assert build.returncode == 0, build.stderr[-2000:]
sys.path.insert(0, str(W))
wrapper_path = S / 'imagededup/handlers/search/brute_force_cython.py'
spec = importlib.util.spec_from_file_location('original_native_wrapper', wrapper_path)
wrapper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wrapper)
hash_path = S / 'imagededup/methods/hashing.py'
cls = next(n for n in ast.parse(hash_path.read_text()).body if isinstance(n, ast.ClassDef) and n.name == 'Hashing')
method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == 'hamming_distance')
method.decorator_list = []
env = {'np':np}
exec(compile(ast.Module(body=[method], type_ignores=[]), str(hash_path), 'exec'), env)
tests = S / 'tests/test_brute_force_cython.py'
parsed = ast.parse(tests.read_text())
parsed.body = [n for n in parsed.body if not (isinstance(n, ast.ImportFrom) and n.module.startswith('imagededup'))]
testenv = dict(BruteForceCython=wrapper.BruteForceCython,
               Hashing=SimpleNamespace(hamming_distance=env['hamming_distance']))
exec(compile(parsed, str(tests), 'exec'), testenv)
passed = sorted(n for n in testenv if n.startswith('test_'))
for name in passed:
    testenv[name]()
db = {f'файл-{i}':f'{i:016x}' for i in range(256)}
search = wrapper.BruteForceCython(db, env['hamming_distance'])
queries = 0
for q in range(256):
    for radius in range(9):
        actual = search.search(f'{q:016x}', radius)
        expected = [(name,(int(value,16)^q).bit_count()) for name,value in db.items() if (int(value,16)^q).bit_count() <= radius]
        assert actual == expected
        queries += 1
calls = []
ignored_callback = wrapper.BruteForceCython({'a':'0'}, lambda *args: calls.append(args) or 99).search('0',0)
assert ignored_callback == [('a',0)] and not calls
assert wrapper.BruteForceCython({}, lambda *args:0).search('0',0) == []
exceptions = {}
for name, db_value, query_value, tolerance in [('negative_radius','0','0',-1),
        ('negative_hash','-1','0',1),('oversized_hash','10000000000000000','0',1),
        ('oversized_query','0','10000000000000000',1)]:
    try:
        wrapper.BruteForceCython({'a':db_value}, lambda *args:0).search(query_value,tolerance)
    except Exception as e:
        exceptions[name] = type(e).__name__
    else:
        raise AssertionError(name)
result = dict(status='passed_compiled_original_generated_CPP_and_native_search',
    source_pins={str(p.relative_to(S)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [cpp,header,wrapper_path,hash_path,tests]},
    original_tests_passed=passed, independent_oracle_queries=queries,
    native_binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), integer_boundary_exceptions=exceptions,
    callback_control={'calls':len(calls),'actual':ignored_callback}, empty_database='returns empty list',
    findings=['Actual unchanged generated C++ compiles with bundled header on MacARM/Python3.12.14; native Unicode filename search matches independent fullscan.',
              'Supplied distance callback is ignored by native backend; actual callback99 still returns distance0.',
              'Unsigned conversion rejects negative radius and negative/oversized hashes; no arbitrary-width metric parity.'],
    limits=['Direct clang build with dynamic_lookup, not setup.py/package/wheel/install/platform matrix.',
            'No source regeneration or modification; no sanitizer, malformed mismatched-vector execution or memory-safety qualification.',
            'Entire files compiler-processed, but only query-related generated code manually read; not full manual source-review credit.',
            'No performance claim/adoption; deferred native pHash investigation remains untouched.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'original_tests':len(passed),'oracle_queries':queries,'boundaries':exceptions}))
