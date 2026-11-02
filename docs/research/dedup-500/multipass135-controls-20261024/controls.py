import ast,argparse,sys,json,math
from pathlib import Path
b=Path('docs/research/dedup-500/evidence/routineLife1--MultiPassDedup');t=ast.parse((b/'deep-infer.py').read_text());f=next(x for x in t.body if isinstance(x,ast.FunctionDef) and x.name=='parse_args');ns={'argparse':argparse};exec(compile(ast.Module(body=[f],type_ignores=[]),'original-infer-parse_args','exec'),ns)
def parse(extra):
 previous=sys.argv;sys.argv=['infer.py','-i','owned-input.mov','-o','owned-output.mov']+extra
 try:return ns['parse_args']()
 finally:sys.argv=previous
base=parse([]);assert base.enable_scdet is False and base.hwaccel is False
negative=parse(['-np','-1']);assert negative.n_pass==-1
nan=parse(['-fps','nan']);assert math.isnan(nan.target_fps)
positive=parse(['-np','3','-t','2','-s','-scale','0.5']);assert positive.n_pass==3 and positive.times==2 and positive.enable_scdet and positive.scale==0.5
out={'controls_passed':4,'scope':'Original argparse function AST unchanged;standard library;synthetic filenames only;no model/PyTorch/GPU/video/CLI main execution','findings':['Scene detection and hardware acceleration default false despite README describing enabled defaults.','Negative n_pass accepted;source main range(n_pass) then tail loop has no infer calls:nontermination consequence source-only,not run.','NaN target_fps accepted;source <=src_fps check does not reject it.','Explicit positive parameters parse as intended.']};Path(__file__).with_name('results.json').write_text(json.dumps(out,indent=2)+'\n');print(json.dumps(out))
