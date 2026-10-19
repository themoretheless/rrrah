"""Observe original collation control flow with a captured stack callback, not PyTorch."""
from pathlib import Path
from types import SimpleNamespace
from typing import List, Dict, Tuple
import ast
import hashlib
import json

R = Path(__file__).resolve().parents[1]
S = R / '.dedup-work/imagededup2-20261019/source'
W = R / '.dedup-work/imagededup2-collation-20261019'
W.mkdir(exist_ok=False)
source = S / 'imagededup/utils/data_generator.py'
node = next(n for n in ast.parse(source.read_text()).body if isinstance(n, ast.FunctionDef) and n.name == '_collate_fn')
calls = []
def captured_stack(items):
    calls.append(list(items))
    return tuple(items)
env = dict(List=List, Dict=Dict, Tuple=Tuple, torch=SimpleNamespace(tensor=object, stack=captured_stack))
exec(compile(ast.Module(body=[node], type_ignores=[]), str(source), 'exec'), env)
rows = []
for mask in range(32):
    batch = [{'image': i if mask & (1 << i) else None, 'filename': f'{i}.png'} for i in range(5)]
    images, names, bad = env['_collate_fn'](batch)
    valid = [i for i in range(5) if mask & (1 << i)]
    assert images == tuple(valid)
    assert names == [f'{i}.png' for i in valid]
    assert bad == [f'{i}.png' for i in range(5) if i not in valid]
    assert calls[-1] == valid
    rows.append({'validity_mask': mask, 'stack_input_count': len(calls[-1]), 'valid_names': names, 'bad_names': bad})
assert env['_collate_fn']([]) == ((), [], [])
assert calls[-1] == []
names = ['imagededup/utils/data_generator.py', 'tests/test_data_generator.py', 'imagededup/utils/models.py']
result = dict(status='passed_original_collate_control_flow_with_captured_stack',
    source_pins={n: hashlib.sha256((S / n).read_bytes()).hexdigest() for n in names},
    mask_controls=rows, empty_input_stack_argument=[],
    findings=['All32five-entry masks preserve exact ordering/alignment of valid image tokens and filenames; invalid names separated.',
              'All-invalid and empty batches call torch.stack with an empty list; no guard or early return in original body.',
              'Original data-generator tests cover mixed corruption but not all-invalid batches or empty directories.',
              'Read CNN batch-method excerpt: bad_im_count increments once per batch containing bad images, not once per bad image; final torch.stack(feat_arr) unconditional.',
              'ImgDataset catches image decode failure via None; basenet_preprocess exceptions are not caught by __getitem__.'],
    limits=['Captured stack returns tuples of integer tokens; no actual torch import, tensor execution, DataLoader or original upstream test run.',
            'Empty-list dispatch established; actual torch exception type/message not qualified.',
            'Whole data_generator, test_data_generator and models read; CNN only excerpt through line220, not whole-file credit.',
            'Model weights, torchvision compatibility, embedding quality and GPU/CPU runtime unqualified.',
            'No production adoption; pHash native investigation remains deferred.'])
(W / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps({'mask_controls': len(rows), 'empty_stack_dispatch': True, 'actual_torch_execution': False}))
