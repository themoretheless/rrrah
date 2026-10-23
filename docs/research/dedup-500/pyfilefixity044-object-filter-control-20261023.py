import ast,json
from pathlib import Path
source=Path('.dedup-work/pyfilefixity044-20261023/pyFileFixity/lib/profilers/visual/pympler/muppy.py')
function=next(n for n in ast.parse(source.read_text()).body if isinstance(n,ast.FunctionDef) and n.name=='filter')
namespace={'_getsizeof':lambda obj:len(obj)}
exec(compile(ast.Module(body=[function],type_ignores=[]),'original-muppy-filter','exec'),namespace)
items=[b'a',b'1234567890']
plain=namespace['filter'](items)
ranged=namespace['filter'](items,Type=bytes,min=5,max=15)
assert plain==[] and ranged==[]
print(json.dumps({'identity_size_provider':'len (deterministic harness seam)','unfiltered_input_count':2,'unfiltered_result_count':len(plain),'expected_size_range_match_count':1,'actual_size_range_match_count':len(ranged)},indent=2))
