"""Original checkpoint-selection method, fake weight loader, temporary empty files."""
import ast
import json
import os
import pathlib
import sys
import tempfile
from types import SimpleNamespace

source = ast.parse(pathlib.Path(sys.argv[1]).read_text())
cls = next(n for n in source.body if isinstance(n, ast.ClassDef) and n.name == 'SpaceTimeModel')
method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == 'load_from_ckpt')
ns = {'os': os}
exec(compile(ast.Module(body=[method], type_ignores=[]), '<original-checkpoint-method>', 'exec'), ns)
with tempfile.TemporaryDirectory() as directory:
    checkpoint = pathlib.Path(directory) / 'model_000010.pth'
    checkpoint.touch()
    calls = []
    obj = SimpleNamespace(args=SimpleNamespace(ckpt_path=str(pathlib.Path(directory) / 'missing.pth'), no_reload=False), load_model=lambda *args: calls.append(args))
    step = ns['load_from_ckpt'](obj, directory)
    assert step == 10 and calls[0][0] == str(checkpoint)
    print(json.dumps({'name': 'missing-explicit-path-falls-back', 'passed': True, 'step': step}))
    calls.clear()
    obj.args.no_reload = True
    assert ns['load_from_ckpt'](obj, directory) == 0 and not calls
    print(json.dumps({'name': 'no-reload-skips-existing', 'passed': True}))
    obj.args.no_reload = False
    obj.args.ckpt_path = str(pathlib.Path(directory) / 'custom.pth')
    pathlib.Path(obj.args.ckpt_path).touch()
    try:
        ns['load_from_ckpt'](obj, directory)
    except ValueError:
        assert len(calls) == 1 and calls[0][0] == obj.args.ckpt_path
        print(json.dumps({'name': 'custom-name-fails-step-parse-after-loader', 'passed': True}))
    else:
        raise AssertionError('expected numeric filename parsing failure')
